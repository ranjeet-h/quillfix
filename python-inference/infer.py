#!/usr/bin/env python3
"""QuillFix MLX inference server — stdin/stdout JSON-line IPC.

Protocol:
  → Rust sends one JSON object per line to stdin:
      {"text": "teh quik brwon fox"}
  ← Python writes one JSON object per line to stdout:
      {"corrected": "the quick brown fox"}
    or on error:
      {"error": "some error message"}

The model is loaded once on startup and kept warm between requests.
"""

import json
import os
import sys
from pathlib import Path

SYSTEM_PROMPT = (
    "You are a precise spelling and grammar correction engine.\n"
    "Your job is to return the user's text with only the minimum necessary edits.\n"
    "Always fix: spelling, capitalization, punctuation, apostrophes, subject-verb agreement, "
    "pronouns, articles, tense, repeated words, and common homophone or word-choice mistakes "
    "such as there/their/they're, your/you're, its/it's, then/than, and to/too/two.\n"
    "Preserve the original meaning, tone, formatting, line breaks, sentence order, URLs, code, "
    "file paths, identifiers, numbers, and technical terms.\n"
    "Do not explain your changes. Do not add quotes, labels, bullets, or commentary. "
    "Do not continue unfinished thoughts. Do not paraphrase or rewrite for style unless grammar "
    "requires it. If the text is already correct, return it unchanged.\n"
    "Output ONLY the corrected text.\n\n"
    "Examples:\n"
    "User: i hav a gret idear for a new prodcut\n"
    "Assistant: I have a great idea for a new product.\n\n"
    "User: she dont know what she is doing\n"
    "Assistant: She doesn't know what she is doing.\n\n"
    "User: him and me went to the park\n"
    "Assistant: He and I went to the park.\n\n"
    "User: their going to the store\n"
    "Assistant: they're going to the store\n\n"
    "User: its been to long since we updated this\n"
    "Assistant: it's been too long since we updated this\n\n"
    "User: The results was better then expected\n"
    "Assistant: The results were better than expected.\n\n"
    "User: make shor you harden the system promt so it works corectly\n"
    "Assistant: make sure you harden the system prompt so it works correctly\n\n"
    "User: the quik brwon fox jumps ovr the lzy dog\n"
    "Assistant: the quick brown fox jumps over the lazy dog\n\n"
    "User: teh quik brwon fox\n"
    "Assistant: the quick brown fox\n\n"
    "User: Please recieve the seperate attachement by tommorow\n"
    "Assistant: Please receive the separate attachment by tomorrow.\n\n"
    "User: This is is the final version\n"
    "Assistant: This is the final version.\n\n"
    "User: The API returns JSON with status 200.\n"
    "Assistant: The API returns JSON with status 200.\n\n"
    "User: Path is /usr/local/bin and env is NODE_ENV=production\n"
    "Assistant: Path is /usr/local/bin and env is NODE_ENV=production\n\n"
    "User: Wornl words are types of hee.\n"
    "Assistant: Wrong words are typed here."
)


class TextOnlyProcessor:
    """Minimal text processor for VLM checkpoints used without image/video inputs."""

    def __init__(self, model_path: str) -> None:
        from transformers import AutoTokenizer
        from mlx_vlm.tokenizer_utils import load_tokenizer
        from mlx_vlm.utils import StoppingCriteria

        path = Path(model_path)
        self.tokenizer = AutoTokenizer.from_pretrained(model_path, use_fast=True)
        detokenizer_wrapper = load_tokenizer(path)
        self.detokenizer = detokenizer_wrapper.detokenizer
        self.tokenizer.stopping_criteria = StoppingCriteria(
            self.tokenizer.eos_token_id,
            self.tokenizer,
        )


def resolve_model_path() -> str:
    """Find the model directory - checks .app bundle or development path."""
    script_dir = os.path.dirname(os.path.abspath(__file__))
    
    # Check if we're in a .app bundle (Resources/python-inference/)
    if ".app" in script_dir:
        # Go up: python-inference -> Resources -> Contents -> MacOS -> QuillFix
        resources_path = script_dir.replace("/python-inference", "")
        if not resources_path.endswith("/Resources"):
            resources_path = script_dir
        bundle_model = os.path.join(resources_path, "model")
        if os.path.isdir(bundle_model):
            return bundle_model
    
    # Check .app bundle via QUILLFIX_EXE_PATH
    exe_path = os.environ.get("QUILLFIX_EXE_PATH", "")
    if exe_path:
        bundle_model = os.path.join(
            os.path.dirname(os.path.dirname(exe_path)),
            "Resources", "model"
        )
        if os.path.isdir(bundle_model):
            return bundle_model

    # Development fallback - infer.py is in python-inference/
    project_root = os.path.dirname(script_dir)
    dev_model = os.path.join(project_root, "resources", "model")
    if os.path.isdir(dev_model):
        return dev_model

    raise FileNotFoundError(
        f"Model not found. Run scripts/download_model.sh first. "
        f"Searched: {dev_model}"
    )


def main() -> None:
    # Suppress mlx-lm's verbose output — only our JSON goes to stdout
    stderr_log = sys.stderr

    model_path = resolve_model_path()
    stderr_log.write(f"[quillfix-infer] loading model from {model_path}\n")
    stderr_log.flush()

    from mlx_vlm.generate import generate  # noqa: E402 — import after path check
    from mlx_vlm.utils import load_model  # noqa: E402 — import after path check

    model = load_model(Path(model_path))
    processor = TextOnlyProcessor(model_path)
    stderr_log.write("[quillfix-infer] model loaded, ready for requests\n")
    stderr_log.flush()

    # Signal readiness to Rust
    sys.stdout.write('{"ready":true}\n')
    sys.stdout.flush()

    # Main request loop
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue

        try:
            request = json.loads(line)
        except json.JSONDecodeError as e:
            sys.stdout.write(json.dumps({"error": f"invalid JSON: {e}"}) + "\n")
            sys.stdout.flush()
            continue

        text = request.get("text", "")
        if not text:
            sys.stdout.write(json.dumps({"error": "empty text"}) + "\n")
            sys.stdout.flush()
            continue

        try:
            prompt = (
                f"<|im_start|>system\n{SYSTEM_PROMPT}<|im_end|>\n"
                f"<|im_start|>user\n{text}<|im_end|>\n"
                "<|im_start|>assistant\n"
                "<think>\n\n</think>\n\n"
            )
            result = generate(
                model,
                processor,
                prompt=prompt,
                max_tokens=min(len(text) * 3, 512),
                verbose=False,
            )
            corrected = result.text.strip()
            if corrected.endswith("<|im_end|>"):
                corrected = corrected[: -len("<|im_end|>")].strip()

            sys.stdout.write(json.dumps({"corrected": corrected}) + "\n")
            sys.stdout.flush()
        except Exception as e:
            sys.stdout.write(json.dumps({"error": str(e)}) + "\n")
            sys.stdout.flush()


if __name__ == "__main__":
    main()
