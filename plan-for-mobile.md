# QuillFix Mobile Plan: KMM + Rust Architecture

## Overview
Extend QuillFix to Android using Kotlin Multiplatform Mobile (KMM) with the existing Rust core, enabling native text selection integration while reusing all current business logic and ML models.

## Architecture Strategy

### High-Level Architecture
```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Android App   │    │   iOS App       │    │  Desktop App    │
│   (Kotlin)      │    │   (Swift)       │    │   (Rust)        │
└─────────┬───────┘    └─────────┬───────┘    └─────────┬───────┘
          │                      │                      │
          └──────────────────────┼──────────────────────┘
                                 │
                    ┌─────────────┴─────────────┐
                    │   KMM Shared Module       │
                    │   (Kotlin Common)         │
                    └─────────────┬─────────────┘
                                  │
                    ┌─────────────┴─────────────┐
                    │   Rust Core Library      │
                    │   (Existing Code)         │
                    └───────────────────────────┘
```

## Project Structure

### Repository Layout
```
quillfix/
├── rust-core/                    # Existing Rust code (extracted)
│   ├── src/
│   │   ├── lib.rs               # UniFFI FFI interface
│   │   ├── corrector.rs         # Existing correction logic
│   │   ├── llm/                 # Existing LLM integration
│   │   └── platform/            # Platform-specific code
│   ├── Cargo.toml
│   ├── build.rs
│   └── quillfix.udl             # UniFFI definition file
├── shared/                       # KMM shared module
│   ├── src/
│   │   ├── commonMain/kotlin/    # Shared Kotlin logic
│   │   │   ├── QuillFix.kt      # Main API interface
│   │   │   ├── models/          # Data models
│   │   │   └── utils/           # Utilities
│   │   ├── androidMain/kotlin/   # Android-specific
│   │   │   ├── QuillFixAndroid.kt
│   │   │   └── platform/        # Android platform code
│   │   └── iosMain/kotlin/       # iOS-specific (future)
│   ├── build.gradle.kts
│   └── src/androidMain/AndroidManifest.xml
├── androidApp/                   # Android application
│   ├── src/main/
│   │   ├── java/com/quillfix/
│   │   │   ├── MainActivity.kt
│   │   │   ├── TextCorrectionActivity.kt
│   │   │   └── ModelManager.kt
│   │   └── AndroidManifest.xml
│   └── build.gradle.kts
├── desktopApp/                   # Existing desktop app
│   └── src/                      # Current Rust desktop code
├── build/                        # Build scripts and CI
│   ├── build-rust.sh
│   └── generate-bindings.sh
└── docs/                         # Documentation
    ├── mobile-api.md
    └── deployment.md
```

## Phase 1: Rust Core Extraction

### 1.1 Extract Existing Rust Code
**Goal**: Isolate current Rust code into reusable library

**Tasks**:
- Move `src/` to `rust-core/src/`
- Update Cargo.toml for library configuration
- Add UniFFI dependencies
- Create FFI interface layer

**rust-core/Cargo.toml**:
```toml
[package]
name = "quillfix-core"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "staticlib"]

[dependencies]
# Existing dependencies
anyhow = "1.0.102"
serde = { version = "1.0.228", features = ["derive"] }
serde_json = "1.0.149"
tokio = { version = "1.49.0", features = ["rt-multi-thread", "macros", "sync", "time"] }
tracing = "0.1.44"
tracing-subscriber = { version = "0.3.22", features = ["env-filter", "fmt"] }

# ML dependencies (platform-specific)
[target.'cfg(target_os = "macos")'.dependencies]
candle-core = { version = "0.9.2", optional = true, features = ["metal"] }
candle-nn = { version = "0.9.2", optional = true }
candle-transformers = { version = "0.9.2", optional = true }

[target.'cfg(target_os = "android")'.dependencies]
candle-core = { version = "0.9.2", optional = true }
candle-nn = { version = "0.9.2", optional = true }
candle-transformers = { version = "0.9.2", optional = true }

[target.'cfg(not(any(target_os = "macos", target_os = "android")))'.dependencies]
candle-core = { version = "0.9.2", optional = true }
candle-nn = { version = "0.9.2", optional = true }
candle-transformers = { version = "0.9.2", optional = true }

# UniFFI for FFI
uniffi = "0.25"

[build-dependencies]
uniffi_build = "0.25"

[features]
default = []
local-llm = [
    "dep:hf-hub",
    "candle-core",
    "candle-nn",
    "candle-transformers",
    "tokenizers",
    "safetensors",
]
```

### 1.2 Create UniFFI Interface
**rust-core/src/lib.rs**:
```rust
use uniffi::export;
use std::sync::Arc;

// Re-export existing functionality
pub use crate::corrector::QuillFixCorrector;
pub use crate::llm::prompt::CorrectionResult;

#[export]
impl QuillFixCore {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            corrector: QuillFixCorrector::new(),
        })
    }
    
    #[export]
    pub fn correct_text(&self, text: String) -> Result<String, String> {
        match self.corrector.correct(&text) {
            Ok(CorrectionResult::Changed(corrected)) => Ok(corrected),
            Ok(CorrectionResult::Unchanged) => Ok(text),
            Ok(CorrectionResult::Error(msg)) => Err(msg),
            Err(e) => Err(e.to_string()),
        }
    }
    
    #[export]
    pub fn is_model_loaded(&self) -> bool {
        self.corrector.is_loaded()
    }
    
    #[export]
    pub async fn load_model(&self) -> Result<(), String> {
        self.corrector.ensure_loaded()
            .map_err(|e| e.to_string())
    }
}

// Generate UniFFI scaffolding
uniffi::build_scaffolding!("./src/quillfix.udl").unwrap();
```

**rust-core/src/quillfix.udl**:
```udl
namespace quillfix {
    interface QuillFixCore {
        [Constructor]
        QuillFixCore();
        
        string correct_text(string text);
        boolean is_model_loaded();
        [Async]
        void load_model();
    }
}
```

## Phase 2: KMM Shared Module

### 2.1 Create KMM Project Structure
**shared/build.gradle.kts**:
```kotlin
plugins {
    kotlin("multiplatform")
    id("com.android.library")
}

kotlin {
    androidTarget {
        compilations.all {
            kotlinOptions {
                jvmTarget = "17"
            }
        }
    }
    
    // Add future iOS target
    // iosArm64()
    // iosSimulatorArm64()
    
    sourceSets {
        commonMain.dependencies {
            implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.7.3")
        }
        
        androidMain.dependencies {
            implementation("androidx.annotation:annotation:1.6.0")
        }
    }
}

// Rust integration
tasks.register<Exec>("buildRustAndroid") {
    workingDir("../rust-core")
    commandLine("cargo", "build", "--target", "aarch64-linux-android")
}

tasks.register<Exec>("buildRustAndroidX86") {
    workingDir("../rust-core")
    commandLine("cargo", "build", "--target", "x86_64-linux-android")
}

tasks.named("preBuild") {
    dependsOn("buildRustAndroid", "buildRustAndroidX86")
}
```

### 2.2 Shared Kotlin Interface
**shared/src/commonMain/kotlin/QuillFix.kt**:
```kotlin
import kotlinx.coroutines.flow.Flow

// Generated by UniFFI - this will be auto-generated
expect class QuillFixCore {
    suspend fun correctText(text: String): Result<String, String>
    fun isModelLoaded(): Boolean
    suspend fun loadModel(): Result<Unit, String>
}

// Shared business logic
class QuillFixRepository {
    private val core = QuillFixCore()
    
    suspend fun correctText(text: String): QuillFixResult {
        return try {
            if (!core.isModelLoaded()) {
                core.loadModel()
            }
            
            when (val result = core.correctText(text)) {
                is Result.Ok -> QuillFixResult.Success(result.value)
                is Result.Err -> QuillFixResult.Error(result.value)
            }
        } catch (e: Exception) {
            QuillFixResult.Error(e.message ?: "Unknown error")
        }
    }
}

sealed class QuillFixResult {
    data class Success(val correctedText: String) : QuillFixResult()
    data class Error(val message: String) : QuillFixResult()
}
```

### 2.3 Android Platform Implementation
**shared/src/androidMain/kotlin/QuillFixAndroid.kt**:
```kotlin
// Actual implementation for Android
actual class QuillFixCore {
    private val nativePtr: Long
    
    init {
        System.loadLibrary("quillfix_core")
        nativePtr = nativeInit()
    }
    
    actual suspend fun correctText(text: String): Result<String, String> {
        return try {
            val result = nativeCorrectText(nativePtr, text)
            if (result != null) {
                Result.Ok(result)
            } else {
                Result.Err("Correction failed")
            }
        } catch (e: Exception) {
            Result.Err(e.message ?: "Native error")
        }
    }
    
    actual fun isModelLoaded(): Boolean {
        return nativeIsModelLoaded(nativePtr)
    }
    
    actual suspend fun loadModel(): Result<Unit, String> {
        return try {
            nativeLoadModel(nativePtr)
            Result.Ok(Unit)
        } catch (e: Exception) {
            Result.Err(e.message ?: "Model loading failed")
        }
    }
    
    private external fun nativeInit(): Long
    private external fun nativeCorrectText(ptr: Long, text: String): String?
    private external fun nativeIsModelLoaded(ptr: Long): Boolean
    private external fun nativeLoadModel(ptr: Long)
}
```

## Phase 3: Android Application

### 3.1 Text Selection Integration
**androidApp/src/main/java/com/quillfix/TextCorrectionActivity.kt**:
```kotlin
class TextCorrectionActivity : Activity() {
    private lateinit var repository: QuillFixRepository
    private lateinit var progressBar: ProgressBar
    
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        
        // Initialize repository
        repository = QuillFixRepository()
        
        // Setup UI
        setupUI()
        
        // Process text selection
        processTextSelection()
    }
    
    private fun setupUI() {
        // Simple loading UI
        progressBar = ProgressBar(this).apply {
            isIndeterminate = true
        }
        
        setContentView(progressBar)
    }
    
    private fun processTextSelection() {
        val text = intent.getCharSequenceExtra(Intent.EXTRA_PROCESS_TEXT)?.toString()
        val readOnly = intent.getBooleanExtra(Intent.EXTRA_PROCESS_TEXT_READONLY, false)
        
        if (text != null && !readOnly && text.isNotBlank()) {
            GlobalScope.launch(Dispatchers.Main) {
                try {
                    progressBar.visibility = View.VISIBLE
                    
                    val result = repository.correctText(text)
                    when (result) {
                        is QuillFixResult.Success -> {
                            returnCorrectedText(result.correctedText)
                        }
                        is QuillFixResult.Error -> {
                            showToast("Correction failed: ${result.message}")
                            returnCorrectedText(text) // Fallback to original
                        }
                    }
                } catch (e: Exception) {
                    showToast("Error: ${e.message}")
                    returnCorrectedText(text)
                } finally {
                    finish()
                }
            }
        } else {
            finish()
        }
    }
    
    private fun returnCorrectedText(corrected: String) {
        val result = Intent()
        result.putExtra(Intent.EXTRA_PROCESS_TEXT, corrected)
        setResult(RESULT_OK, result)
    }
    
    private fun showToast(message: String) {
        Toast.makeText(this, message, Toast.LENGTH_SHORT).show()
    }
}
```

### 3.2 Android Manifest
**androidApp/src/main/AndroidManifest.xml**:
```xml
<manifest xmlns:android="http://schemas.android.com/apk/res/android">
    
    <!-- Text selection integration -->
    <activity
        android:name=".TextCorrectionActivity"
        android:label="Correct with QuillFix"
        android:theme="@android:style/Theme.Translucent.NoTitleBar"
        android:exported="true">
        <intent-filter>
            <action android:name="android.intent.action.PROCESS_TEXT" />
            <category android:name="android.intent.category.DEFAULT" />
            <data android:mimeType="text/plain" />
        </intent-filter>
    </activity>
    
    <!-- Main activity for settings -->
    <activity
        android:name=".MainActivity"
        android:exported="true">
        <intent-filter>
            <action android:name="android.intent.action.MAIN" />
            <category android:name="android.intent.category.LAUNCHER" />
        </intent-filter>
    </activity>
    
    <!-- Permissions -->
    <uses-permission android:name="android.permission.INTERNET" />
    <uses-permission android:name="android.permission.WRITE_EXTERNAL_STORAGE" />
    
</manifest>
```

### 3.3 Model Management
**androidApp/src/main/java/com/quillfix/ModelManager.kt**:
```kotlin
class ModelManager(private val context: Context) {
    private val modelDir = File(context.getExternalFilesDir(null), "models")
    private val qwenModelFile = File(modelDir, "qwen2.5-0.5b-instruct-q8_0.gguf")
    
    suspend fun ensureModelDownloaded(): Boolean {
        if (qwenModelFile.exists()) return true
        
        return downloadModel()
    }
    
    private suspend fun downloadModel(): Boolean {
        return withContext(Dispatchers.IO) {
            try {
                val modelUrl = "https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF/resolve/main/qwen2.5-0.5b-instruct-q8_0.gguf"
                
                // Download with progress
                val url = URL(modelUrl)
                url.openStream().use { input ->
                    FileOutputStream(qwenModelFile).use { output ->
                        input.copyTo(output)
                    }
                }
                
                true
            } catch (e: Exception) {
                Log.e("ModelManager", "Download failed", e)
                false
            }
        }
    }
    
    fun getModelPath(): String? {
        return if (qwenModelFile.exists()) {
            qwenModelFile.absolutePath
        } else null
    }
    
    fun getModelSize(): Long {
        return if (qwenModelFile.exists()) {
            qwenModelFile.length()
        } else 0
    }
}
```

## Phase 4: On-Device ML Integration

### 4.1 Qwen 2.5 Android Deployment
**rust-core/src/android.rs**:
```rust
#[cfg(target_os = "android")]
pub mod android {
    use super::*;
    use std::path::PathBuf;
    
    pub struct AndroidModelLoader {
        model_path: Option<PathBuf>,
    }
    
    impl AndroidModelLoader {
        pub fn new() -> Self {
            Self { model_path: None }
        }
        
        pub fn set_model_path<P: AsRef<Path>>(&mut self, path: P) {
            self.model_path = Some(path.as_ref().to_path_buf());
        }
        
        pub async fn load_model(&self) -> Result<(), Box<dyn std::error::Error>> {
            if let Some(path) = &self.model_path {
                // Load Qwen 2.5 GGUF model using llama.cpp
                let llama = llama_cpp::Llama::new()
                    .with_model_path(path)
                    .with_context_size(2048)
                    .with_n_threads(4)
                    .build()?;
                
                // Store in global state for corrections
                *GLOBAL_LLAMA.lock().unwrap() = Some(llama);
                Ok(())
            } else {
                Err("Model path not set".into())
            }
        }
    }
    
    static GLOBAL_LLAMA: std::sync::Mutex<Option<llama_cpp::Llama>> = 
        std::sync::Mutex::new(None);
    
    pub fn correct_text_android(text: &str) -> Result<String, Box<dyn std::error::Error>> {
        let llama = GLOBAL_LLAMA.lock().unwrap();
        if let Some(llama) = llama.as_ref() {
            let prompt = format!("Correct this text for grammar and spelling: {}", text);
            let corrected = llama.complete(prompt, text.len() + 100)?;
            Ok(corrected)
        } else {
            Err("Model not loaded".into())
        }
    }
}
```

### 4.2 Performance Optimization
**rust-core/src/performance.rs**:
```rust
pub struct PerformanceConfig {
    pub max_threads: usize,
    pub context_size: usize,
    pub batch_size: usize,
    pub gpu_layers: i32,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            max_threads: num_cpus::get(),
            context_size: 2048,
            batch_size: 512,
            gpu_layers: 0, // CPU-only for Android initially
        }
    }
}

impl PerformanceConfig {
    pub fn for_device() -> Self {
        #[cfg(target_os = "android")]
        {
            // Android-specific optimizations
            Self {
                max_threads: std::cmp::min(num_cpus::get(), 4), // Limit threads on mobile
                context_size: 1024, // Smaller context for memory
                batch_size: 256,
                gpu_layers: 0,
            }
        }
        
        #[cfg(not(target_os = "android"))]
        Self::default()
    }
}
```

## Phase 5: Build and Deployment

### 5.1 Cross-Platform Build Script
**build/build-rust.sh**:
```bash
#!/bin/bash

set -e

echo "Building Rust core for all targets..."

# Android targets
cargo build --target aarch64-linux-android --release
cargo build --target x86_64-linux-android --release

# Desktop targets
cargo build --target x86_64-apple-darwin --release
cargo build --target x86_64-pc-windows-msvc --release
cargo build --target x86_64-unknown-linux-gnu --release

echo "Generating UniFFI bindings..."
cd rust-core
uniffi-bindgen generate --language kotlin --out-dir ../shared/src/androidMain/kotlin/ src/quillfix.udl

echo "Build complete!"
```

### 5.2 Android Gradle Integration
**androidApp/build.gradle.kts**:
```kotlin
plugins {
    id("com.android.application")
    kotlin("android")
}

android {
    namespace = "com.quillfix"
    compileSdk = 34
    
    defaultConfig {
        applicationId = "com.quillfix"
        minSdk = 26
        targetSdk = 34
        versionCode = 1
        versionName = "1.0"
    }
    
    buildTypes {
        release {
            isMinifyEnabled = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"))
        }
    }
    
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    
    kotlinOptions {
        jvmTarget = "17"
    }
    
    ndkVersion = "25.1.8937393"
}

dependencies {
    implementation(project(":shared"))
    implementation("androidx.core:core-ktx:1.12.0")
    implementation("androidx.appcompat:appcompat:1.6.1")
    implementation("com.google.android.material:material:1.10.0")
}

// Copy Rust libraries
tasks.register<Copy>("copyRustLibs") {
    from("../rust-core/target/aarch64-linux-android/release/libquillfix_core.so")
    into("src/main/jniLibs/arm64-v8a/")
    rename("libquillfix_core.so", "libquillfix_core.so")
}

tasks.named("preBuild") {
    dependsOn("copyRustLibs")
}
```

## Phase 6: Testing Strategy

### 6.1 Unit Tests
**shared/src/commonTest/kotlin/QuillFixTest.kt**:
```kotlin
class QuillFixTest {
    @Test
    fun testTextCorrection() {
        val repository = QuillFixRepository()
        
        runTest {
            val result = repository.correctText("hello worlld")
            assertTrue(result is QuillFixResult.Success)
            assertEquals("hello world", (result as QuillFixResult.Success).correctedText)
        }
    }
    
    @Test
    fun testModelLoading() {
        val core = QuillFixCore()
        
        runTest {
            assertFalse(core.isModelLoaded())
            val loadResult = core.loadModel()
            assertTrue(loadResult is Result.Ok)
            assertTrue(core.isModelLoaded())
        }
    }
}
```

### 6.2 Integration Tests
**androidApp/src/test/java/com/quillfix/TextCorrectionTest.kt**:
```kotlin
@RunWith(AndroidJUnit4::class)
class TextCorrectionTest {
    
    @Test
    fun testTextSelectionIntent() {
        val intent = Intent().apply {
            action = Intent.ACTION_PROCESS_TEXT
            putExtra(Intent.EXTRA_PROCESS_TEXT, "test text")
            putExtra(Intent.EXTRA_PROCESS_TEXT_READONLY, false)
        }
        
        // Test activity can handle the intent
        val activity = ActivityScenario.launch<TextCorrectionActivity>(intent)
        
        // Verify result
        activity.onActivity { activity ->
            // Check that activity processes text correctly
        }
    }
}
```

## Phase 7: Performance Optimization

### 7.1 Model Optimization
- **Quantization**: Use Q4_K_M for best speed/size ratio
- **Pruning**: Remove unnecessary model weights
- **Caching**: Keep model in memory for frequent use
- **Batching**: Process multiple corrections together

### 7.2 Battery Optimization
```kotlin
class BatteryManager {
    fun shouldUseLowPowerMode(): Boolean {
        val batteryLevel = getBatteryLevel()
        val isCharging = isDeviceCharging()
        val thermalState = getThermalState()
        
        return batteryLevel < 20 && !isCharging || thermalState > THERMAL_STATE_MODERATE
    }
    
    fun getOptimalThreads(): Int {
        return if (shouldUseLowPowerMode()) {
            2 // Reduce threads for battery saving
        } else {
            minOf(4, numCpuCores())
        }
    }
}
```

## Phase 8: Deployment

### 8.1 App Store Preparation
- **Play Store**: Prepare APK/AAB with proper signing
- **Model Assets**: Include small default model or download on first use
- **Permissions**: Request only necessary permissions
- **Privacy Policy**: Document on-device processing

### 8.2 Distribution Strategy
- **Google Play Store**: Primary distribution
- **F-Droid**: Open-source alternative
- **Direct APK**: For testing/sideload

## Timeline

### Week 1-2: Rust Core Extraction
- Extract existing code to `rust-core/`
- Add UniFFI bindings
- Test with existing desktop app

### Week 3-4: KMM Integration
- Create shared module
- Implement Android bindings
- Build basic Android app

### Week 5-6: Android Features
- Text selection integration
- Model management
- UI implementation

### Week 7-8: Testing & Optimization
- Unit and integration tests
- Performance optimization
- Battery usage optimization

### Week 9-10: Deployment
- App store preparation
- Documentation
- Release

## Success Metrics

### Performance Targets
- **Model loading**: < 3 seconds
- **Text correction**: < 2 seconds for typical text
- **Memory usage**: < 500MB total
- **Battery impact**: < 5% per hour of use

### User Experience Targets
- **First launch**: Model download + setup < 30 seconds
- **Subsequent use**: Instant correction
- **Error rate**: < 1% correction failures
- **User satisfaction**: > 4.5/5 rating

## Future Enhancements

### iOS Support
- Add iOS target to KMM
- Implement iOS text selection
- App Store deployment

### Advanced Features
- Multiple language support
- Custom correction styles
- Learning from user corrections
- Voice input correction

### Cloud Integration
- Hybrid on-device/cloud processing
- Model updates
- User preferences sync

This plan provides a comprehensive roadmap for extending QuillFix to Android using KMM + Rust, leveraging existing code while providing native mobile experience.
