# DMG Size Reduction Plan

This plan implements three strategies to reduce the QuillFix DMG from ~973MB to ~150-200MB by removing the model from the bundle, optimizing Python dependencies, and using better compression.

## Overview

### Current Issues:
- ML model files (~828MB from model.safetensors)
- Complete Python virtual environment (~100+MB)
- Inefficient DMG compression (UDZO format)

### Target Result: 
Reduce DMG size by 70-80% while maintaining full functionality.

## Implementation Steps

### Option 1: Download Model on First Run

#### 1.1 Modify bundle.sh
- Remove model copying: Comment out `cp -R "$RES_DIR/model" "$APP_DIR/Contents/Resources/model"`
- Ensure Resources directory still exists for model download

#### 1.2 Update Rust Backend (src/llm/backend.rs)
- Modify `resolve_model_path()` to handle missing model directory
- Add comprehensive model download functionality with retry logic
- Implement exponential backoff retry strategy (3 attempts with delays: 1s, 2s, 4s)
- Add network connectivity checks before download attempts
- Integrate with existing download_model.sh logic
- Add progress reporting and detailed error handling
- Validate downloaded model integrity with checksum verification

#### 1.3 Enhance Python Inference (python-inference/infer.py)
- Update `resolve_model_path()` to trigger download if model missing
- Add retry logic for model download failures
- Implement proper error messages for different failure scenarios
- Handle disk space checks before download
- Add timeout handling for slow networks

#### 1.4 Update User Experience
- Add comprehensive download UI with progress indicators
- Implement real-time progress updates (percentage, speed, ETA)
- Add retry buttons and manual retry options
- Handle different error states with specific messages:
  - Network failures (offline, timeout, slow connection)
  - Disk space issues (insufficient storage, permissions)
  - Server errors (HTTP errors, rate limiting)
  - Corrupted downloads (checksum failures)
- Add pause/resume functionality for large downloads
- Implement background download with notification support

### Option 3: Optimize Python Dependencies

#### 3.1 Create Minimal Virtual Environment
- Modify python-inference setup to use `--no-cache-dir` during pip install
- Remove unnecessary packages from requirements
- Clean up pip cache after installation

#### 3.2 Bundle Script Updates
- Add `pip cache purge` after dependency installation
- Remove development/testing packages from bundle
- Optimize Python environment size

#### 3.3 Dependency Audit
- Review current packages in python-inference/lib/python3.13/site-packages/
- Remove unused packages and .dist-info directories
- Keep only essential MLX and inference dependencies

### Option 4: Better DMG Compression

#### 4.1 Update create_dmg.sh
- Change format from UDZO to UDBZ (better compression)
- Add compression level optimization: `-imagekey zlib-level=9`
- Test compression quality vs time tradeoff

#### 4.2 Compression Testing
- Compare file sizes between UDZO and UDBZ formats
- Ensure DMG still opens correctly on target macOS versions
- Verify no functionality loss from compression
## Implementation Order

1. **Phase 1:** Update bundle.sh and create_dmg.sh (quick wins)
2. **Phase 2:** Implement model download logic in Rust/Python
3. **Phase 3:** Optimize Python dependencies
4. **Phase 4:** Test and validate complete solution

## Files to Modify

### Core Scripts
- `scripts/bundle.sh` - Remove model copying
- `scripts/create_dmg.sh` - Better compression
- `scripts/download_model.sh` - May need updates for programmatic use

### Rust Code
- `src/llm/backend.rs` - Add model download logic
- `src/main.rs` - Add first-run setup handling
- `src/menu_bar.rs` - Add download progress UI

### Python Code  
- `python-inference/infer.py` - Update model path resolution

### New Files
- `src/model_downloader.rs` - Dedicated model download module with retry logic
- `src/download_ui.rs` - Download progress UI components
- `scripts/setup_minimal_python.sh` - Optimized Python environment setup
- `src/network_utils.rs` - Network connectivity and retry utilities
## Detailed Download & Retry Implementation

### Retry Strategy
- **Exponential Backoff:** 3 attempts with delays (1s, 2s, 4s)
- **Network Validation:** Check connectivity before each attempt
- **Partial Resume:** Support for resuming interrupted downloads
- **Checksum Verification:** Validate model integrity after download

### Error Handling Scenarios

#### Network Issues:
- No internet connection
- Slow/timeout connections
- DNS resolution failures
- HTTP 5xx server errors

#### Storage Issues:
- Insufficient disk space (check 2x model size requirement)
- Permission denied errors
- Corrupted filesystem

#### Download Issues:
- Partial downloads (resume capability)
- Corrupted files (checksum mismatch)
- Rate limiting from HuggingFace

### UI Components
- **Progress Bar:** Real-time percentage, speed, ETA
- **Status Messages:** Clear error descriptions and solutions
- **Retry Options:** Automatic retry with manual override
- **Background Mode:** Continue download in background with notifications
- **Cancel/Pause:** User control over download process

### Download States
1. **Initializing:** Checking network, disk space
2. **Downloading:** Progress updates, speed monitoring
3. **Verifying:** Checksum validation
4. **Completed:** Model ready for use
5. **Failed:** Error display with retry options
6. **Paused:** User-initiated pause with resume option
## Testing Strategy

1. **Size Validation:** Measure DMG size before/after changes
2. **Functionality Testing:** Ensure model download and inference work correctly
3. **First-Run Testing:** Verify smooth first-time user experience
4. **Network Testing:** Test all download failure scenarios:
   - Network disconnection during download
   - Slow network conditions
   - Server errors and rate limiting
   - DNS resolution failures
5. **Storage Testing:** 
   - Insufficient disk space scenarios
   - Permission denied errors
   - Corrupted download recovery
6. **Retry Logic Testing:**
   - Automatic retry behavior
   - Manual retry functionality
   - Resume interrupted downloads
7. **UI Testing:**
   - Progress indicator accuracy
   - Error message clarity
   - Background download notifications
8. **Compression Testing:** Validate DMG opens correctly with new format

## Expected Results

- **Before:** ~973MB DMG
- **After:** ~150-200MB DMG (70-80% reduction)
- **User Experience:** One-time model download on first launch
- **Functionality:** No loss of features or performance

## Risk Mitigation

- **Download Failures:** Comprehensive retry logic with exponential backoff
- **Network Issues:** Pre-download connectivity checks and graceful fallbacks
- **Storage Issues:** Disk space validation and permission checks
- **Model Integrity:** Checksum verification and corruption recovery
- **User Experience:** Clear error messages with actionable solutions
- **Performance:** Background downloads with minimal UI blocking
- **Reliability:** Resume capability for interrupted downloads
- **Compatibility:** Test DMG compression across macOS versions