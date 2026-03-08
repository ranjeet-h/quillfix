#![allow(dead_code)]

#[cfg(feature = "local-llm")]
use anyhow::Context;
use anyhow::{Result, anyhow};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[cfg(feature = "local-llm")]
use candle_core::{DType, Device, Tensor};
#[cfg(feature = "local-llm")]
use candle_nn::VarBuilder;
#[cfg(feature = "local-llm")]
use candle_transformers::generation::{LogitsProcessor, Sampling};
#[cfg(feature = "local-llm")]
use candle_transformers::models::qwen2::{Config as Qwen2Config, ModelForCausalLM};
#[cfg(feature = "local-llm")]
use tokenizers::Tokenizer;

/// Inner state only compiled when the `local-llm` feature is active.
#[cfg(feature = "local-llm")]
struct Qwen2Inner {
    model: ModelForCausalLM,
    tokenizer: Tokenizer,
    device: Device,
    eos_token_id: u32,
}

/// A running Python subprocess for MLX inference.
struct PythonProcess {
    child: std::process::Child,
    stdin: std::io::BufWriter<std::process::ChildStdin>,
    stdout: std::io::BufReader<std::process::ChildStdout>,
}

/// Shared state across clone()d handles (the `Clone` on `LlmBackend`
/// is used only in tests; in production a single instance lives in CORRECTOR).
struct BackendState {
    loaded: bool,
    model_path: Option<PathBuf>,
    python: Option<PythonProcess>,
    #[cfg(feature = "local-llm")]
    inner: Option<Box<Qwen2Inner>>,
}

impl BackendState {
    const fn new() -> Self {
        Self {
            loaded: false,
            model_path: None,
            python: None,
            #[cfg(feature = "local-llm")]
            inner: None,
        }
    }
}

#[derive(Clone)]
pub struct LlmBackend {
    state: Arc<Mutex<BackendState>>,
}

impl Default for LlmBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl LlmBackend {
    const DEFAULT_READY_TIMEOUT_MS: u64 = 60_000;
    const DEFAULT_REQUEST_TIMEOUT_MS: u64 = 12_000;

    #[must_use]
    pub fn new() -> Self {
        Self { state: Arc::new(Mutex::new(BackendState::new())) }
    }

    fn env_timeout_ms(key: &str, default_ms: u64) -> Duration {
        let ms = std::env::var(key)
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|ms| *ms > 0)
            .unwrap_or(default_ms);
        Duration::from_millis(ms)
    }

    fn reset_python_process(state: &mut BackendState) {
        if let Some(mut proc) = state.python.take() {
            let _ = proc.child.kill();
            let _ = proc.child.wait();
        }
    }

    #[cfg(unix)]
    fn wait_stdout_with_timeout(
        stdout: &std::process::ChildStdout,
        timeout: Duration,
    ) -> Result<()> {
        use std::os::fd::AsRawFd;

        let timeout_ms_u128 = timeout.as_millis().min(i32::MAX as u128);
        let timeout_ms = i32::try_from(timeout_ms_u128).unwrap_or(i32::MAX);
        let mut poll_fd = libc::pollfd { fd: stdout.as_raw_fd(), events: libc::POLLIN, revents: 0 };
        // SAFETY: `poll_fd` points to valid memory and has a single entry.
        let rc = unsafe { libc::poll(&raw mut poll_fd, 1, timeout_ms) };
        if rc == 0 {
            return Err(anyhow!("timed out waiting for Python response"));
        }
        if rc < 0 {
            return Err(anyhow!(
                "failed waiting for Python response: {}",
                std::io::Error::last_os_error()
            ));
        }
        if (poll_fd.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL)) != 0 {
            return Err(anyhow!("Python subprocess stream closed"));
        }
        Ok(())
    }

    #[cfg(not(unix))]
    fn wait_stdout_with_timeout(
        _stdout: &std::process::ChildStdout,
        _timeout: Duration,
    ) -> Result<()> {
        Ok(())
    }

    fn read_line_with_timeout(
        reader: &mut BufReader<std::process::ChildStdout>,
        timeout: Duration,
    ) -> Result<String> {
        Self::wait_stdout_with_timeout(reader.get_ref(), timeout)?;
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .map_err(|e| anyhow!("failed reading from Python stdout: {e}"))?;
        if line.trim().is_empty() {
            return Err(anyhow!("empty response from Python subprocess"));
        }
        Ok(line)
    }

    fn elapsed_ms(start: Instant) -> u64 {
        u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    /// Resolve the model directory: prefer the `.app` bundle path, fall back to cwd.
    fn resolve_model_path() -> PathBuf {
        // Running inside QuillFix.app:
        //   …/QuillFix.app/Contents/MacOS/quillfix  (exe)
        //   …/QuillFix.app/Contents/Resources/model (weights)
        if let Ok(exe) = std::env::current_exe() {
            let bundle_resources = exe
                .parent() // MacOS/
                .and_then(|p| p.parent()) // Contents/
                .map(|p| p.join("Resources").join("model"));
            if let Some(path) = bundle_resources
                && path.exists()
            {
                return path;
            }
        }
        // Development / test fallback
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join("resources/model")
    }

    /// Resolve the `python-inference/` venv directory.
    /// Checks `.app` bundle first, then project root relative to cwd.
    fn resolve_python_venv() -> Option<PathBuf> {
        // Inside .app bundle
        if let Ok(exe) = std::env::current_exe() {
            let bundle_py = exe
                .parent()
                .and_then(|p| p.parent())
                .map(|p| p.join("Resources").join("python-inference"));
            if let Some(path) = bundle_py
                && path.join("bin").join("python3").exists()
            {
                return Some(path);
            }
        }
        // Development fallback
        let dev =
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join("python-inference");
        if dev.join("bin").join("python3").exists() {
            return Some(dev);
        }
        None
    }

    /// Spawn the Python MLX inference subprocess and wait for its ready signal.
    fn spawn_python(venv: &Path) -> Result<PythonProcess> {
        let start = Instant::now();
        let python = venv.join("bin").join("python3");
        let script = venv.join("infer.py");
        if !script.exists() {
            return Err(anyhow!("infer.py not found in {}", venv.display()));
        }

        let mut command = Command::new(&python);
        command.arg(&script).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit()); // model loading logs go to stderr
        if let Ok(exe_path) = std::env::current_exe() {
            command.env("QUILLFIX_EXE_PATH", exe_path);
        }

        let mut child =
            command.spawn().map_err(|e| anyhow!("failed to spawn Python subprocess: {e}"))?;

        let child_stdin = child.stdin.take().ok_or_else(|| anyhow!("no stdin pipe"))?;
        let child_stdout = child.stdout.take().ok_or_else(|| anyhow!("no stdout pipe"))?;

        let mut stdout_reader = BufReader::new(child_stdout);

        // Wait for {"ready":true} signal (with timeout)
        let ready_timeout =
            Self::env_timeout_ms("QUILLFIX_READY_TIMEOUT_MS", Self::DEFAULT_READY_TIMEOUT_MS);
        let ready_line = match Self::read_line_with_timeout(&mut stdout_reader, ready_timeout) {
            Ok(line) => line,
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(anyhow!(
                    "Python subprocess did not become ready in {} ms: {e}",
                    ready_timeout.as_millis()
                ));
            }
        };

        let ready: serde_json::Value = serde_json::from_str(ready_line.trim())
            .map_err(|e| anyhow!("invalid ready signal: {e} (got: {ready_line})"))?;
        if ready.get("ready").and_then(serde_json::Value::as_bool) != Some(true) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(anyhow!("unexpected ready signal: {ready_line}"));
        }

        tracing::info!(
            phase = "llm",
            startup_ms = Self::elapsed_ms(start),
            "Python MLX subprocess ready"
        );

        Ok(PythonProcess { child, stdin: BufWriter::new(child_stdin), stdout: stdout_reader })
    }

    /// Load the model on first call; no-op on subsequent calls.
    ///
    /// Tries Python MLX subprocess first, then candle (`local-llm` feature),
    /// then falls back to the deterministic stub.
    ///
    /// # Errors
    /// Returns an error if weights are missing or the model cannot be built.
    pub fn ensure_loaded(&self) -> Result<()> {
        let started = Instant::now();
        let already = {
            let s = self.state.lock().map_err(|_| anyhow!("state lock poisoned"))?;
            s.loaded
        };
        if already {
            return Ok(());
        }

        let path = Self::resolve_model_path();

        // Try Python MLX subprocess first (works with MLX 4-bit weights).
        // Disabled when QUILLFIX_STUB=1 (set by `make test` for deterministic tests).
        let use_python = std::env::var("QUILLFIX_STUB").is_err();
        if use_python && let Some(venv) = Self::resolve_python_venv() {
            match Self::spawn_python(&venv) {
                Ok(proc) => {
                    let mut s = self.state.lock().map_err(|_| anyhow!("state lock poisoned"))?;
                    s.loaded = true;
                    s.model_path = Some(path);
                    s.python = Some(proc);
                    drop(s);
                    tracing::info!(
                        phase = "llm",
                        backend = "python",
                        load_ms = Self::elapsed_ms(started),
                        "backend ready"
                    );
                    return Ok(());
                }
                Err(e) => {
                    tracing::warn!(phase = "llm", ?e, "Python MLX subprocess failed; falling back");
                }
            }
        }

        #[cfg(feature = "local-llm")]
        self.load_real(&path)?;

        let mut s = self.state.lock().map_err(|_| anyhow!("state lock poisoned"))?;
        s.loaded = true;
        s.model_path = Some(path);
        drop(s);
        tracing::info!(
            phase = "llm",
            backend = if cfg!(feature = "local-llm") { "candle" } else { "stub" },
            load_ms = Self::elapsed_ms(started),
            "backend ready"
        );
        Ok(())
    }

    // ------------------------------------------------------------------
    // Real model loading (local-llm feature)
    // ------------------------------------------------------------------

    #[cfg(feature = "local-llm")]
    fn load_real(&self, path: &PathBuf) -> Result<()> {
        use std::fs;

        let config_path = path.join("config.json");
        let config_str = fs::read_to_string(&config_path)
            .with_context(|| format!("reading config from {}", config_path.display()))?;
        let mut config_json: serde_json::Value = serde_json::from_str(&config_str)
            .with_context(|| "parsing config.json as generic json")?;

        // Qwen3.5 puts language model properties inside text_config, but Candle's Qwen2Config expects them at the root.
        let text_config_clone = config_json.get("text_config").cloned();
        if let Some(serde_json::Value::Object(text_config)) = text_config_clone {
            let config_obj = config_json.as_object_mut().unwrap();
            for (key, value) in text_config {
                if !config_obj.contains_key(&key) {
                    config_obj.insert(key, value);
                }
            }
        }

        // Qwen2Config in candle explicitly looks for `sliding_window` but Qwen3.5 may not provide it if it's default
        if let Some(config_obj) = config_json.as_object_mut() {
            if !config_obj.contains_key("sliding_window") {
                config_obj.insert("sliding_window".to_string(), serde_json::json!(32768));
            }
            if !config_obj.contains_key("max_window_layers") {
                config_obj.insert("max_window_layers".to_string(), serde_json::json!(21));
            }
            if !config_obj.contains_key("hidden_size") {
                // Extracted from typical Qwen 0.5/0.8B configs if missing, but it should be there.
                config_obj.insert("hidden_size".to_string(), serde_json::json!(1536));
            }
            // Qwen3.5 nests rope_theta under rope_parameters
            if !config_obj.contains_key("rope_theta") {
                let rope_theta = config_obj
                    .get("rope_parameters")
                    .and_then(|rp| rp.get("rope_theta"))
                    .cloned()
                    .unwrap_or(serde_json::json!(10000.0));
                config_obj.insert("rope_theta".to_string(), rope_theta);
            }
        }

        let config: Qwen2Config =
            serde_json::from_value(config_json).with_context(|| "parsing modified config.json")?;

        let tokenizer_path = path.join("tokenizer.json");
        let tokenizer =
            Tokenizer::from_file(&tokenizer_path).map_err(|e| anyhow!("loading tokenizer: {e}"))?;

        // Prefer Metal for Apple Silicon; fall back to CPU
        let device = Device::new_metal(0).unwrap_or(Device::Cpu);

        // Collect safetensors shards
        let mut shard_paths: Vec<PathBuf> = fs::read_dir(path)
            .with_context(|| format!("reading model dir {}", path.display()))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("safetensors"))
            .collect();
        if shard_paths.is_empty() {
            return Err(anyhow!("no .safetensors files found in {}", path.display()));
        }
        shard_paths.sort();

        let dtype = DType::F16;
        // SAFETY: we own the model directory and the files are not mutated.
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&shard_paths, dtype, &device)
                .with_context(|| "loading safetensors weights")?
        };

        let mut model =
            ModelForCausalLM::new(&config, vb).with_context(|| "building Qwen3.5 model")?;

        // Pre-warm: compile Metal/ANE pipeline with a single-token pass
        let dummy = Tensor::zeros((1, 1), DType::U32, &device)
            .with_context(|| "creating pre-warm tensor")?;
        let _ = model.forward(&dummy, 0);
        model.clear_kv_cache();

        let eos_token_id = tokenizer
            .token_to_id("<|im_end|>")
            .or_else(|| tokenizer.token_to_id("</s>"))
            .unwrap_or(151_645_u32); // Qwen3.5 default EOS

        let mut s = self.state.lock().map_err(|_| anyhow!("state lock poisoned"))?;
        s.inner = Some(Box::new(Qwen2Inner { model, tokenizer, device, eos_token_id }));
        Ok(())
    }

    // ------------------------------------------------------------------
    // Inference
    // ------------------------------------------------------------------

    /// Send a request to the Python subprocess and read the response.
    fn infer_python(proc: &mut PythonProcess, text: &str) -> Result<String> {
        let request_timeout =
            Self::env_timeout_ms("QUILLFIX_REQUEST_TIMEOUT_MS", Self::DEFAULT_REQUEST_TIMEOUT_MS);
        let request = serde_json::json!({"text": text});
        writeln!(proc.stdin, "{request}")
            .map_err(|e| anyhow!("failed writing to Python stdin: {e}"))?;
        proc.stdin.flush().map_err(|e| anyhow!("failed flushing Python stdin: {e}"))?;

        let response_line = Self::read_line_with_timeout(&mut proc.stdout, request_timeout)?;

        let resp: serde_json::Value = serde_json::from_str(response_line.trim())
            .map_err(|e| anyhow!("invalid JSON from Python: {e} (got: {response_line})"))?;

        if let Some(err) = resp.get("error").and_then(|v| v.as_str()) {
            return Err(anyhow!("Python inference error: {err}"));
        }

        resp.get("corrected")
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string)
            .ok_or_else(|| anyhow!("missing 'corrected' in Python response: {response_line}"))
    }

    /// Run inference.
    ///
    /// Priority:
    /// 1. Python MLX subprocess (if spawned)
    /// 2. candle (`local-llm` feature)
    /// 3. Deterministic stub (CI / no model)
    ///
    /// # Errors
    /// Returns an error if inference fails.
    pub fn infer(&self, prompt: &str) -> Result<String> {
        let started = Instant::now();
        // Extract user text from the ChatML prompt for the Python IPC
        let user_text = Self::extract_user_text(prompt);

        let python_result = {
            let mut s = self.state.lock().map_err(|_| anyhow!("state lock poisoned"))?;
            if let Some(ref mut proc) = s.python {
                if let Ok(Some(status)) = proc.child.try_wait() {
                    tracing::warn!(phase = "llm", ?status, "python subprocess exited unexpectedly");
                    Self::reset_python_process(&mut s);
                    None
                } else {
                    Some(Self::infer_python(proc, &user_text))
                }
            } else {
                None
            }
        };

        if let Some(result) = python_result {
            match result {
                Ok(corrected) => {
                    tracing::info!(
                        phase = "llm",
                        backend = "python",
                        text_len = user_text.len(),
                        latency_ms = Self::elapsed_ms(started),
                        "inference ok"
                    );
                    return Ok(corrected);
                }
                Err(first_err) => {
                    tracing::warn!(
                        phase = "llm",
                        ?first_err,
                        "python inference failed; restarting"
                    );
                    {
                        let mut s =
                            self.state.lock().map_err(|_| anyhow!("state lock poisoned"))?;
                        Self::reset_python_process(&mut s);
                        s.loaded = false;
                    }
                    self.ensure_loaded()?;
                    let retry = {
                        let mut s =
                            self.state.lock().map_err(|_| anyhow!("state lock poisoned"))?;
                        s.python.as_mut().map_or_else(
                            || Err(anyhow!("python backend unavailable after restart")),
                            |proc| Self::infer_python(proc, &user_text),
                        )
                    };
                    match retry {
                        Ok(corrected) => {
                            tracing::info!(
                                phase = "llm",
                                backend = "python-retry",
                                text_len = user_text.len(),
                                latency_ms = Self::elapsed_ms(started),
                                "inference ok"
                            );
                            return Ok(corrected);
                        }
                        Err(retry_err) => {
                            tracing::warn!(
                                phase = "llm",
                                ?retry_err,
                                "python retry failed; falling back to alternate backend"
                            );
                            let mut s =
                                self.state.lock().map_err(|_| anyhow!("state lock poisoned"))?;
                            Self::reset_python_process(&mut s);
                            s.python = None;
                        }
                    }
                }
            }
        }

        #[cfg(feature = "local-llm")]
        {
            let result = self.infer_real(prompt);
            if result.is_ok() {
                tracing::info!(
                    phase = "llm",
                    backend = "candle",
                    text_len = user_text.len(),
                    latency_ms = Self::elapsed_ms(started),
                    "inference ok"
                );
            }
            return result;
        }

        #[cfg(not(feature = "local-llm"))]
        {
            let corrected = Self::infer_stub(prompt);
            tracing::info!(
                phase = "llm",
                backend = "stub",
                text_len = user_text.len(),
                latency_ms = Self::elapsed_ms(started),
                "inference ok"
            );
            Ok(corrected)
        }
    }

    #[cfg(not(feature = "local-llm"))]
    fn infer_stub(prompt: &str) -> String {
        let user_text = Self::extract_user_text(prompt);
        let lower = user_text.to_lowercase();

        // Spelling corrections
        if lower.contains("teh quik brwon fox") {
            return "the quick brown fox".to_string();
        }
        if lower.contains("i hav a gret idear") {
            return "I have a great idea".to_string();
        }
        if lower.contains("speling mistaeks") {
            return "spelling mistakes".to_string();
        }
        if lower.contains("ths is wrng") {
            return "This is wrong".to_string();
        }
        if lower.contains("recieve") && lower.contains("seperate") {
            return user_text.replace("recieve", "receive").replace("seperate", "separate");
        }
        if lower.contains("definately") {
            return user_text.replace("definately", "definitely");
        }
        if lower.contains("occured") {
            return user_text.replace("occured", "occurred");
        }

        // Grammar corrections
        if lower.contains("she dont") {
            return user_text.replace("dont", "doesn't");
        }
        if lower.contains("him and me went") {
            return user_text.replace("him and me went", "he and I went");
        }
        if lower.contains("their going to") {
            return user_text.replace("their going to", "they're going to");
        }

        // Punctuation corrections
        if lower.contains("hello world how are you") && !lower.contains("hello, world") {
            return "Hello, world! How are you?".to_string();
        }

        // Return the user-turn text unchanged (simulates "no correction needed")
        user_text
    }

    fn extract_user_text(prompt: &str) -> String {
        if let Some(user_text) = prompt.split("<|im_start|>user\n").nth(1)
            && let Some(text) = user_text.split("<|im_end|>").next()
        {
            return text.trim().to_string();
        }
        prompt.to_string()
    }

    #[cfg(feature = "local-llm")]
    fn infer_real(&self, prompt: &str) -> Result<String> {
        let mut s = self.state.lock().map_err(|_| anyhow!("state lock poisoned"))?;
        let inner = s
            .inner
            .as_mut()
            .ok_or_else(|| anyhow!("model not loaded; call ensure_loaded() first"))?;

        let max_new_tokens = (prompt.len() + 50).min(512);

        // Encode
        let encoding =
            inner.tokenizer.encode(prompt, true).map_err(|e| anyhow!("tokenization error: {e}"))?;
        let mut tokens: Vec<u32> = encoding.get_ids().to_vec();
        let prompt_len = tokens.len();

        let mut logits_processor = LogitsProcessor::from_sampling(42, Sampling::ArgMax);
        let mut generated_ids: Vec<u32> = Vec::new();

        for i in 0..max_new_tokens {
            let start = if i == 0 { 0 } else { tokens.len() - 1 };
            let input = Tensor::new(tokens[start..].to_vec(), &inner.device)?.unsqueeze(0)?;
            let seqlen_offset = if i == 0 { 0 } else { prompt_len + i - 1 };
            let logits = inner.model.forward(&input, seqlen_offset)?;
            let logits = logits.squeeze(0)?.squeeze(0)?.to_dtype(DType::F32)?;
            let next_token = logits_processor.sample(&logits)?;
            tokens.push(next_token);
            generated_ids.push(next_token);
            if next_token == inner.eos_token_id {
                break;
            }
        }

        let raw = inner
            .tokenizer
            .decode(&generated_ids, true)
            .map_err(|e| anyhow!("decoding error: {e}"))?;

        Ok(raw.trim_end_matches("<|im_end|>").trim_end_matches('\n').trim().to_string())
    }

    #[must_use]
    pub fn is_loaded(&self) -> bool {
        self.state.lock().map(|s| s.loaded).unwrap_or(false)
    }
}
