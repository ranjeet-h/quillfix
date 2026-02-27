# QuillFix - Project Summary

## Overview

QuillFix is a macOS menu bar application that provides one-click grammar and spelling correction for selected text using local machine learning models. The application integrates seamlessly with macOS accessibility features and offers both local LLM inference and cloud-based correction capabilities.

## Architecture

### Core Components

#### 1. Main Application (`src/main.rs`)
- **Entry Point**: Application initialization and lifecycle management
- **Permission Management**: Accessibility permissions check and onboarding
- **Background Pre-warming**: LLM backend initialization on background thread
- **Menu Bar Integration**: Setup of macOS menu bar interface
- **NSServices Registration**: System-wide service integration

#### 2. Text Correction Engine (`src/corrector.rs`, `src/llm/`)
- **Global Corrector Instance**: Thread-safe singleton pattern using `Arc<Mutex<Corrector>>`
- **Multi-Backend Support**: 
  - Python MLX subprocess (primary)
  - Rust Candle implementation (local-llm feature)
  - Deterministic stub (testing/fallback)
- **Backend Abstraction**: Unified interface for different inference backends

#### 3. LLM Backend (`src/llm/backend.rs`)
- **Python MLX Integration**: Subprocess communication via JSON IPC
- **Candle Implementation**: Native Rust ML inference with Metal acceleration
- **Model Management**: Loading, caching, and lifecycle management
- **Fallback Strategy**: Graceful degradation between backends

#### 4. Menu Bar Interface (`src/menu_bar.rs`)
- **Native macOS Integration**: Objective-C bindings via objc2
- **System Status Item**: Menu bar icon and dropdown menu
- **User Preferences**: Toggle enable/disable, launch at login
- **Accessibility Settings**: Direct link to system preferences

#### 5. Python Inference Server (`python-inference/infer.py`)
- **MLX Model Loading**: Apple MLX framework for efficient inference
- **JSON IPC Protocol**: Stdin/stdout communication with Rust backend
- **Prompt Engineering**: System prompts for grammar/spelling correction
- **Error Handling**: Robust error reporting and recovery

## Technology Stack

### Rust Core
- **Edition**: Rust 2024 Edition (rustc 1.93.1+)
- **Async Runtime**: Tokio for concurrent operations
- **Logging**: tracing with macOS OSLog integration
- **Error Handling**: anyhow for ergonomic error management
- **Serialization**: serde/serde_json for data interchange

### macOS Integration
- **Frameworks**: AppKit, CoreGraphics, CoreFoundation, ApplicationServices, Accessibility
- **Objective-C Bindings**: objc2 crate for safe Objective-C interop
- **System Services**: NSServices for system-wide text correction
- **Menu Bar**: NSStatusBar and NSMenu for native UI

### Machine Learning
- **Primary Backend**: Python MLX with mlx-lm
- **Native Backend**: Candle with Metal acceleration (optional)
- **Model**: Qwen2.5-1.5B-Instruct-4bit (Hugging Face)
- **Tokenization**: Hugging Face tokenizers

### Development Tools
- **Build System**: Cargo with custom build.rs for framework linking
- **Code Quality**: rustfmt, clippy with strict linting rules
- **Testing**: Unit and integration tests with deterministic stub
- **Packaging**: Custom scripts for app bundling and DMG creation

## Project Structure

```
quillfix/
├── src/                    # Rust source code
│   ├── main.rs            # Application entry point
│   ├── lib.rs             # Library interface
│   ├── corrector.rs       # Global corrector instance
│   ├── menu_bar.rs        # macOS menu bar interface
│   └── llm/               # LLM backend implementation
│       ├── mod.rs         # Backend interface
│       ├── backend.rs     # Multi-backend implementation
│       └── prompt.rs       # Prompt engineering
├── python-inference/       # Python MLX inference server
│   ├── infer.py           # Main inference script
│   ├── requirements.txt   # Python dependencies
│   └── pyvenv.cfg         # Virtual environment config
├── resources/             # Application resources
│   ├── Info.plist         # macOS app metadata
│   ├── entitlements.plist # App entitlements
│   └── model/             # ML model directory (gitignored)
├── scripts/               # Build and utility scripts
│   ├── bundle.sh          # App bundling and signing
│   ├── create_dmg.sh      # DMG installer creation
│   ├── download_model.sh  # Model download script
│   └── run_integration_tests.sh # Test runner
├── tests/                 # Test suites
│   ├── integration_*.rs   # Integration tests
│   └── unit_*.rs         # Unit tests
├── docs/                  # Documentation (currently empty)
├── Cargo.toml            # Rust project configuration
├── Makefile              # Build automation
├── build.rs              # Build script for framework linking
└── README.md             # Project documentation
```

## Key Features

### 1. System Integration
- **Menu Bar Application**: Native macOS menu bar interface
- **NSServices**: "Correct with QuillFix" in system services menu
- **Accessibility API**: Text selection and correction
- **Launch at Login**: Optional auto-start functionality

### 2. Multi-Backend Architecture
- **Python MLX**: Primary backend with Apple Silicon optimization
- **Rust Candle**: Native Rust implementation with Metal support
- **Fallback Stub**: Deterministic corrections for testing/offline use

### 3. Performance Optimizations
- **Background Pre-warming**: Model loading on startup
- **Process Isolation**: Separate Python process for ML inference
- **Caching**: Persistent model and tokenizer caching
- **Metal Acceleration**: GPU acceleration for Apple Silicon

### 4. User Experience
- **One-Click Correction**: Simple text selection and correction
- **Real-time Feedback**: Immediate correction results
- **Privacy-First**: Local processing ensures text privacy
- **System Integration**: Seamless macOS experience

## Build and Deployment

### Development Build
```bash
cargo check                    # Basic compilation check
cargo test                     # Run tests (with stub backend)
make check-all                # Full quality gate
```

### Production Build
```bash
make dmg                       # Build signed app and DMG
cargo build --release --features local-llm  # Full ML build
```

### Dependencies
- **Rust Toolchain**: Stable with Edition 2024 support
- **macOS**: Version 14+ required
- **Xcode**: Full installation for MLX/Metal compilation
- **Python**: For MLX inference server
- **Hugging Face CLI**: For model downloads

## Security and Privacy

### Local Processing
- **No Cloud Dependencies**: All processing happens locally
- **Text Privacy**: No text sent to external services
- **Sandboxing**: macOS app sandboxing support

### Permissions
- **Accessibility**: Required for text selection access
- **File System**: Model directory access
- **Network**: Optional for model downloads only

## Testing Strategy

### Unit Tests
- **Component Testing**: Individual module testing
- **Mock Backends**: Deterministic stub for reliable testing
- **Edge Cases**: Error handling and boundary conditions

### Integration Tests
- **End-to-End**: Full correction workflow testing
- **System Integration**: macOS accessibility testing
- **Backend Compatibility**: Multiple backend validation

### Quality Gates
- **Linting**: Strict clippy rules with pedantic checks
- **Formatting**: Consistent code style with rustfmt
- **Security**: Dependency vulnerability scanning

## Future Enhancements

### Planned Features
- **Additional Models**: Support for different correction models
- **Custom Prompts**: User-configurable correction styles
- **Batch Processing**: Multiple text correction
- **Statistics**: Usage analytics and correction history

### Technical Improvements
- **Performance**: Further optimization of inference speed
- **Memory**: Reduced memory footprint
- **Compatibility**: Broader macOS version support
- **Internationalization**: Multi-language support

## License and Distribution

- **License**: MIT License
- **Distribution**: DMG installer with code signing
- **Source**: Open source with contribution guidelines
- **Support**: GitHub issues and community support

---

*This summary provides a comprehensive overview of the QuillFix project architecture, technologies, and implementation details for developers and stakeholders.*
