# QuillFix — Implementation Plan

> **App**: QuillFix
> **Stack**: Rust 2024 Edition · macOS 14+ (Sonoma) · mlx-rs · objc2 · CGEventTap · AXAPI
> **Model**: `mlx-community/Qwen2.5-0.5B-Instruct-4bit` (~300 MB) — bundled inside `.app`
> **Testing**: `cargo test` (unit) + integration test binaries (macOS APIs) — no dummy tests; a phase is complete only when its tests pass
> **Docs**: `docs/phase-N/HOWTO.md` per phase

---

## Completion Key

| Symbol | Meaning                      |
| ------ | ---------------------------- |
| `[ ]`  | Not started                  |
| `[~]`  | In progress                  |
| `[x]`  | Done — all test(s) passing   |

---

## Final Folder Structure

```
quillfix/
├── Cargo.toml                          ← workspace manifest
├── Cargo.lock
├── build.rs                            ← framework linking
├── src/
│   ├── main.rs                         ← entry point; runs NSApplication on main thread
│   ├── permissions.rs                  ← AX + Input Monitoring permission checks
│   ├── menu_bar.rs                     ← NSStatusItem, toggle, UserDefaults persistence
│   ├── event_monitor.rs                ← CGEventTap + AXObserver wiring
│   ├── debounce.rs                     ← timer + selection hash deduplication
│   ├── ax_query.rs                     ← AXUIElement text + bounds extraction
│   ├── popup.rs                        ← NSPanel, animations, auto-hide logic
│   ├── replacement.rs                  ← AX setValue primary + clipboard fallback
│   └── llm/
│       ├── mod.rs                      ← public API: Corrector::correct(text)
│       ├── backend.rs                  ← mlx-rs model load + inference
│       └── prompt.rs                   ← prompt builder + post-processor
├── tests/
│   ├── unit_permissions.rs
│   ├── unit_debounce.rs
│   ├── unit_popup.rs
│   ├── unit_prompt.rs
│   ├── unit_replacement.rs
│   ├── integration_bundle.rs
│   ├── integration_menu_bar.rs
│   ├── integration_event_monitor.rs
│   ├── integration_popup.rs
│   ├── integration_llm.rs
│   └── integration_e2e.rs
├── resources/
│   ├── Info.plist
│   ├── entitlements.plist
│   └── model/                          ← Qwen2.5-0.5B-Instruct-4bit weights (gitignored)
├── scripts/
│   ├── bundle.sh                       ← assembles QuillFix.app + codesigns
│   ├── download_model.sh               ← one-time model download via huggingface-cli
│   └── run_integration_tests.sh        ← runs tests by phase or all
└── docs/
    ├── phase-1/HOWTO.md
    ├── phase-2/HOWTO.md
    ├── phase-3/HOWTO.md
    ├── phase-4/HOWTO.md
    ├── phase-5/HOWTO.md
    └── phase-6/HOWTO.md
```

---

## Phase 1 — Project Setup and Foundations `[ ]`

**Goal**: A compiling Rust project with a valid `.app` bundle skeleton, Accessibility permission detection, and bundled model weights present at the expected path.

### 1.1 Cargo workspace and dependencies `[ ]`

- [ ] Run `cargo init --name quillfix` in the repo root
- [ ] Set `edition = "2024"` in `[package]`
- [ ] Add the following to `Cargo.toml`:

```toml
[dependencies]
mlx-rs           = "0.21"
mlx-models       = "0.21"
objc2             = "0.5"
objc2-app-kit     = { version = "0.2", features = [
                       "NSStatusItem", "NSPanel", "NSMenu",
                       "NSMenuItem", "NSImageView", "NSApplication"] }
objc2-foundation  = "0.2"
core-graphics     = { version = "0.23", features = ["event"] }
core-foundation   = "0.9"
accessibility     = "0.1"
tokio             = { version = "1", features = ["full"] }
anyhow            = "1"
log               = "0.4"
oslog             = "0.2"
dirs              = "5"

[dev-dependencies]
assert_cmd = "2"
tempfile   = "3"
```

- [ ] Verify `cargo check` exits 0 with zero errors

### 1.2 build.rs — framework linking `[ ]`

- [ ] Create `build.rs` in the repo root
- [ ] Emit the following link directives:

```rust
fn main() {
    println!("cargo:rustc-link-lib=framework=AppKit");
    println!("cargo:rustc-link-lib=framework=CoreGraphics");
    println!("cargo:rustc-link-lib=framework=CoreFoundation");
    println!("cargo:rustc-link-lib=framework=ApplicationServices");
    println!("cargo:rustc-link-lib=framework=Accessibility");
}
```

- [ ] Verify `cargo build` links without any `ld: framework not found` errors

### 1.3 .app bundle resources `[ ]`

- [ ] Create `resources/Info.plist` with the following keys:
  - `CFBundleName` → `QuillFix`
  - `CFBundleIdentifier` → `com.quillfix.app`
  - `CFBundleExecutable` → `quillfix`
  - `CFBundleVersion` → `0.1.0`
  - `LSUIElement` → `true` (hides from Dock and App Switcher)
  - `NSAccessibilityUsageDescription` → `QuillFix needs Accessibility access to read and correct selected text.`
  - `NSAppleEventsUsageDescription` → `QuillFix uses Apple Events for text replacement fallback.`
  - `CFBundleIconFile` → `AppIcon`
- [ ] Create `resources/entitlements.plist` with:
  - `com.apple.security.automation.apple-events` → `true`
  - `com.apple.security.device.input-monitoring` → `true`

### 1.4 Bundle script `[ ]`

- [ ] Create `scripts/bundle.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail
BINARY=target/release/quillfix
APP=QuillFix.app
cargo build --release
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS"
mkdir -p "$APP/Contents/Resources"
cp "$BINARY" "$APP/Contents/MacOS/quillfix"
cp resources/Info.plist "$APP/Contents/"
cp -r resources/model "$APP/Contents/Resources/"
codesign --deep --force --sign - \
  --entitlements resources/entitlements.plist "$APP"
echo "Bundle ready: $APP"
```

- [ ] `chmod +x scripts/bundle.sh`
- [ ] Run `scripts/bundle.sh` and confirm `QuillFix.app` is created without errors

### 1.5 Model acquisition `[ ]`

- [ ] Create `scripts/download_model.sh`:

```bash
#!/usr/bin/env bash
# Requires: pip install huggingface_hub
set -euo pipefail
DEST=resources/model
mkdir -p "$DEST"
huggingface-cli download mlx-community/Qwen2.5-0.5B-Instruct-4bit \
  --local-dir "$DEST"
echo "Model downloaded to $DEST"
```

- [ ] `chmod +x scripts/download_model.sh`
- [ ] Run `scripts/download_model.sh` and verify `resources/model/` contains `*.safetensors`, `config.json`, and `tokenizer.json`
- [ ] Add `resources/model/` to `.gitignore`

### 1.6 Permission checker `[ ]`

- [ ] Create `src/permissions.rs` with the following public API:

```rust
/// Represents the macOS Accessibility permission state.
#[derive(Debug, Clone, PartialEq)]
pub enum PermissionState {
    Granted,
    Denied,
    NotDetermined,
}

/// Query Accessibility permission without prompting the user.
pub fn accessibility_state() -> PermissionState { ... }

/// Open System Settings → Privacy & Security → Accessibility.
pub fn open_accessibility_settings() { ... }
```

- [ ] Implement `accessibility_state()` using `AXIsProcessTrustedWithOptions` via `core-foundation` FFI
- [ ] Implement `open_accessibility_settings()` using `NSWorkspace::open(url)` with the deep-link URL `x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility`
- [ ] In `main.rs`, call `accessibility_state()` on launch; if `Denied`, call `open_accessibility_settings()` and log the event

### 1.7 Logging setup `[ ]`

- [ ] In `main.rs`, initialize the `oslog` logger with subsystem `com.quillfix.app` before any other code runs
- [ ] Verify all `log::info!` and `log::error!` calls route to Console.app (not stdout) in release builds

### 1.8 Docs `[ ]`

- [ ] Create `docs/phase-1/HOWTO.md` with the following sections:
  - **Prerequisites** — Rust 2024 toolchain, macOS 14+, Xcode CLT, `pip install huggingface_hub`
  - **Download model** — `bash scripts/download_model.sh`
  - **Build** — `cargo build` (debug) or `bash scripts/bundle.sh` (release `.app`)
  - **Run** — `open QuillFix.app`; check Console.app for permission log output
  - **Run tests** — `cargo test --test integration_bundle && cargo test unit_permissions`
  - **Expected result** — all tests green; `QuillFix.app` passes `codesign --verify`

### Phase 1 Tests `[ ]`

> A phase is marked `[x]` only when every test below passes.

- [ ] **`tests/unit_permissions.rs`** — `test_permission_state_enum_variants`
  - Assert `PermissionState::Granted != PermissionState::Denied`
  - Assert `PermissionState::NotDetermined` is a distinct third variant
  - Assert `accessibility_state()` returns a valid variant without panicking
  - _(Does not assert a specific value — the result depends on the machine's permission state)_

- [ ] **`tests/integration_bundle.rs`** — `test_app_bundle_structure`
  - Assert `QuillFix.app/Contents/MacOS/quillfix` binary exists
  - Assert `QuillFix.app/Contents/Info.plist` exists and is valid XML plist
  - Assert `LSUIElement` key parses to `true`
  - Assert `CFBundleIdentifier` == `"com.quillfix.app"`
  - Assert `NSAccessibilityUsageDescription` key is present and non-empty
  - Assert `QuillFix.app/Contents/Resources/model/` contains at least one `.safetensors` file

- [ ] **`tests/integration_bundle.rs`** — `test_codesign_verify`
  - Run `codesign --verify --deep QuillFix.app` via `std::process::Command`
  - Assert exit code == 0

---

## Phase 2 — Menu Bar UI `[ ]`

**Goal**: QuillFix appears in the macOS menu bar with a "sparkles" SF Symbol icon, a single enable/disable toggle, and state persisted across restarts via `NSUserDefaults`.

### 2.1 NSStatusItem and icon `[ ]`

- [ ] Create `src/menu_bar.rs`
- [ ] Implement `menu_bar::setup()`:
  - Call `NSStatusBar::system().statusItem(withLength: NSVariableStatusItemLength)`
  - Set the button image to SF Symbol `"sparkles"` via `NSImage::imageWithSystemSymbolName_accessibilityDescription`
  - Call `setTemplate(true)` on the image so it adapts to light/dark menu bar
  - Assign a `NSMenu` containing one item: `"Enable QuillFix"`
- [ ] Call `menu_bar::setup()` from `main.rs` on the main thread before `NSApplication::run()`

### 2.2 Toggle state `[ ]`

- [ ] Add a global `AtomicBool` named `IS_ENABLED`
- [ ] When the menu item is clicked:
  - Flip `IS_ENABLED`
  - Update menu item title to `"Disable QuillFix"` when enabled, `"Enable QuillFix"` when disabled
  - Set menu item state (`NSOnState` / `NSOffState`) for the native checkmark
  - Call `event_monitor::start()` stub when enabled; `event_monitor::stop()` stub when disabled

### 2.3 UserDefaults persistence `[ ]`

- [ ] In `menu_bar.rs`, define:

```rust
const DEFAULTS_KEY: &str = "quillfix.enabled";

pub fn load_enabled_state() -> bool { ... }   // reads NSUserDefaults
pub fn save_enabled_state(enabled: bool) { ... } // writes NSUserDefaults
```

- [ ] On app launch, call `load_enabled_state()` and apply the result to `IS_ENABLED` and the menu item
- [ ] On every toggle, call `save_enabled_state(new_state)`

### 2.4 Launch at Login (optional) `[ ]`

- [ ] Add a second menu item `"Launch at Login"` with a checkmark state
- [ ] Toggling it writes (or removes) `~/Library/LaunchAgents/com.quillfix.app.plist`:

```xml
<key>Label</key>        <string>com.quillfix.app</string>
<key>Program</key>      <string>/Applications/QuillFix.app/Contents/MacOS/quillfix</string>
<key>RunAtLoad</key>    <true/>
```

- [ ] Persist preference to `NSUserDefaults` key `"quillfix.launchAtLogin"`

### 2.5 Docs `[ ]`

- [ ] Create `docs/phase-2/HOWTO.md` with:
  - **Build + Run** — `bash scripts/bundle.sh && open QuillFix.app`
  - **Manual test** — Click menu bar icon → `"Enable QuillFix"` → quit → reopen → verify state persisted
  - **Run tests** — `cargo test unit_menu_bar && cargo test --test integration_menu_bar`
  - **Expected result** — Sparkles icon appears; toggle works; state survives restart

### Phase 2 Tests `[ ]`

- [ ] **`tests/unit_menu_bar.rs`** — `test_defaults_key_constant`
  - Assert `DEFAULTS_KEY == "quillfix.enabled"`

- [ ] **`tests/unit_menu_bar.rs`** — `test_load_save_enabled_state_roundtrip`
  - Call `save_enabled_state(true)` → `load_enabled_state()` → assert `true`
  - Call `save_enabled_state(false)` → `load_enabled_state()` → assert `false`

- [ ] **`tests/integration_menu_bar.rs`** — `test_status_item_exists_after_setup`
  - Spawn the QuillFix process; wait 2 s
  - Assert `NSStatusBar::system().statusItems()` contains an item whose button image name contains `"sparkles"`

- [ ] **`tests/integration_menu_bar.rs`** — `test_toggle_persists_after_restart`
  - Enable via AppleScript (`menu bar item "QuillFix" → click "Enable QuillFix"`)
  - Kill the process; relaunch
  - Read `NSUserDefaults` key `"quillfix.enabled"` → assert `true`
  - Disable; kill; relaunch → assert `false`

---

## Phase 3 — Global Event Monitoring for Text Selection `[ ]`

**Goal**: Detect when the user selects text in any app, debounce the event, and extract the selected text and screen bounds via Accessibility APIs.

### 3.1 CGEventTap `[ ]`

- [ ] Create `src/event_monitor.rs`
- [ ] Implement `event_monitor::start()`:
  - Create a `CGEventTap` at `kCGSessionEventTap` tapping `kCGEventLeftMouseUp` and `kCGEventKeyUp`
  - In the callback, post a `SelectionCandidate` to an internal `mpsc::Sender`
  - Add the tap to the current `CFRunLoop` and enable it
- [ ] Implement `event_monitor::stop()`:
  - Disable the `CGEventTap`
  - Remove it from the run loop
- [ ] If tap creation fails (no Input Monitoring permission), log a warning and fall back to pure AXObserver

### 3.2 AXObserver `[ ]`

- [ ] Implement `register_ax_observer(pid: pid_t)` in `event_monitor.rs`:
  - Create an `AXObserver` for the given PID
  - Add notifications: `kAXSelectedTextChangedNotification` and `kAXFocusedUIElementChangedNotification`
  - On `FocusedUIElementChanged`: call `register_ax_observer(new_pid)` for the new foreground app
  - On `SelectedTextChanged`: post a `SelectionCandidate` to the same internal channel

### 3.3 Debouncer `[ ]`

- [ ] Create `src/debounce.rs`:

```rust
pub struct Debouncer {
    delay_ms: u64,
    last_hash: AtomicU64,   // FNV-1a hash of the last seen text
    timer_handle: Option<JoinHandle<()>>,
}

impl Debouncer {
    pub fn new(delay_ms: u64) -> Self { ... }

    /// Returns Some(text) only once the selection has been stable for `delay_ms`.
    /// Returns None for rapid changes or repeated identical text.
    pub fn feed(&self, text: &str) -> Option<String> { ... }
}
```

- [ ] Use a constant `DEBOUNCE_MS: u64 = 300`
- [ ] Hash text with FNV-1a; if hash is unchanged, suppress the event
- [ ] On hash change, cancel any pending timer and start a new one

### 3.4 AX text and bounds extraction `[ ]`

- [ ] Create `src/ax_query.rs`:

```rust
pub struct SelectionInfo {
    pub text: String,
    pub bounds: CGRect,      // AppKit screen coordinates (origin bottom-left)
    pub element_role: String,
}

pub fn query_selection(element: &AXUIElement) -> Option<SelectionInfo> { ... }
```

- [ ] Read `AXSelectedText` for the text content
- [ ] Read `AXBoundsForRange` for the selected range to get a `CGRect`
- [ ] Convert the Quartz flipped coordinate rect to AppKit screen coordinates
- [ ] Read `AXRole` — pass only `"AXTextArea"` and `"AXTextField"`
- [ ] Read `AXSecureTextField` — skip if `true` (password field)
- [ ] Reject text shorter than 4 characters or longer than 1500 characters

### 3.5 Callback wiring `[ ]`

- [ ] In the event callback: call `debouncer.feed(raw_text)` → if `Some(stable_text)` returned, call `query_selection()` and send the result on the popup channel
- [ ] Ensure all `AXUIElement` calls are dispatched to the main thread via `dispatch_async(dispatch_get_main_queue(), ...)`

### 3.6 Docs `[ ]`

- [ ] Create `docs/phase-3/HOWTO.md` with:
  - **Prerequisites** — Accessibility permission must be granted to `QuillFix` in System Settings → Privacy → Accessibility
  - **Build + Run** — `bash scripts/bundle.sh && open QuillFix.app` → enable via menu
  - **Manual test** — Open TextEdit, type `hello world`, select it → observe `Selection detected: hello world` in Console.app
  - **Run tests** — `cargo test unit_debounce && bash scripts/run_integration_tests.sh phase3`
  - **Expected result** — Selection of ≥4 chars fires within 350 ms; password fields and short selections are silently skipped

### Phase 3 Tests `[ ]`

- [ ] **`tests/unit_debounce.rs`** — `test_debounce_suppresses_rapid_changes`
  - Feed the debouncer 5 different strings in < 50 ms intervals
  - Assert only the last string produces `Some(...)` after 300 ms
  - Assert all earlier feeds return `None`

- [ ] **`tests/unit_debounce.rs`** — `test_debounce_same_text_deduplicated`
  - Feed `"hello"`, wait 400 ms (fires once)
  - Feed `"hello"` again immediately
  - Assert the second feed returns `None` (same FNV hash → suppressed)

- [ ] **`tests/unit_debounce.rs`** — `test_debounce_different_text_fires_again`
  - Feed `"hello"`, wait 400 ms (fires)
  - Feed `"world"` (different hash)
  - Wait 400 ms → assert fires again with `"world"`

- [ ] **`tests/unit_debounce.rs`** — `test_text_filter_min_length`
  - `filter_text("hi")` → `None` (2 chars, below minimum of 4)
  - `filter_text("hello")` → `Some("hello")`

- [ ] **`tests/unit_debounce.rs`** — `test_text_filter_max_length`
  - 1501-char string → `None`
  - 1500-char string → `Some(...)`

- [ ] **`tests/integration_event_monitor.rs`** — `test_ax_query_fires_on_selection`
  - Open TextEdit via `NSWorkspace`
  - Use AppleScript to type `"QuillFix test selection"` and select all
  - Start `event_monitor::start()` with a channel receiver
  - Assert a `SelectionInfo { text: "QuillFix test selection", .. }` arrives within 600 ms

- [ ] **`tests/integration_event_monitor.rs`** — `test_password_field_skipped`
  - Create a test `NSWindow` containing an `NSSecureTextField`
  - Focus it and programmatically set selected text
  - Assert no `SelectionInfo` is produced within 1 s

---

## Phase 4 — Popup Icon Display `[ ]`

**Goal**: A non-activating 32×32 `NSPanel` with a "sparkles" SF Symbol appears near the text selection with smooth CoreAnimation, and auto-hides on all expected triggers.

### 4.1 NSPanel creation `[ ]`

- [ ] Create `src/popup.rs`
- [ ] Implement `popup::create() -> NSPanel`:
  - Borderless, non-activating `NSPanel` (`NSBorderlessWindowMask | NSNonactivatingPanelMask`)
  - Frame: `{ width: 32, height: 32 }` (positioned dynamically)
  - `setLevel(NSPopUpMenuWindowLevel)` — floats above all other windows
  - `setHidesOnDeactivate(false)` — stays visible when another app is focused
  - `setOpaque(false)`, `setBackgroundColor(NSColor.clear)`
  - Content view: `NSVisualEffectView` (`.hudWindow` material) containing an `NSImageView` with SF Symbol `"sparkles"`
  - Initial `alphaValue = 0.0`

### 4.2 Position calculation `[ ]`

- [ ] Implement `popup::position_near(bounds: CGRect) -> NSPoint`:
  - Convert the AX `CGRect` (Quartz flipped, origin top-left) to AppKit coordinates (origin bottom-left)
  - Place the popup 8 px above and 8 px to the right of the top-right corner of the selection
  - Clamp so the popup never extends beyond any `NSScreen` boundary

### 4.3 Show animation `[ ]`

- [ ] Implement `popup::show(at: NSPoint)`:
  - Move the window frame origin to the computed position
  - Call `orderFront(nil)`
  - Apply two simultaneous `CABasicAnimation`s:
    - `"transform.scale"`: `0.6 → 1.0`, duration `0.2 s`, timing `.easeOut`
    - `"opacity"`: `0.0 → 1.0`, duration `0.2 s`
  - Schedule a 4-second auto-hide timer

### 4.4 Hide animation and dismissal triggers `[ ]`

- [ ] Implement `popup::hide()`:
  - `CABasicAnimation` on `"opacity"`: `1.0 → 0.0`, duration `0.15 s`
  - On animation completion: call `orderOut(nil)`
  - Cancel any active auto-hide timer
- [ ] Register `NSEvent.addGlobalMonitorForEvents(matching: .leftMouseDown)` → call `hide()` on any click outside the popup
- [ ] Register a local `NSEvent` monitor for the ESC key (`\u{1b}`) → call `hide()`
- [ ] When a new `SelectionInfo` arrives while the popup is visible, reposition to the new bounds without hiding/re-showing

### 4.5 Popup click and success feedback `[ ]`

- [ ] On popup click, call `trigger_correction()` (stub in Phase 4; real in Phase 5)
- [ ] Implement `popup::show_success()`:
  - Replace the `NSImageView` image with SF Symbol `"checkmark.circle.fill"` in `NSColor.systemGreen`
  - After 180 ms, call `popup::hide()`
  - Restore the image to `"sparkles"` ready for the next invocation

### 4.6 Docs `[ ]`

- [ ] Create `docs/phase-4/HOWTO.md` with:
  - **Build + Run** — `bash scripts/bundle.sh && open QuillFix.app` → enable
  - **Manual test** — Select ≥4 chars in TextEdit → sparkles popup appears → click → green checkmark flashes → popup hides
  - **Run tests** — `cargo test unit_popup && bash scripts/run_integration_tests.sh phase4`
  - **Expected result** — Popup visible within 300 ms of selection; hides on ESC, click-out, 4 s timeout, and after success flash

### Phase 4 Tests `[ ]`

- [ ] **`tests/unit_popup.rs`** — `test_position_near_basic`
  - Input: `CGRect { x: 100, y: 200, width: 300, height: 20 }` on a 1440×900 screen
  - Expected: `NSPoint { x: 408, y: 228 }` (right+8, top+8 of selection bounds)
  - Assert result matches within ±1 px

- [ ] **`tests/unit_popup.rs`** — `test_position_near_clamps_to_screen`
  - Input: selection near right edge `CGRect { x: 1420, y: 500, width: 100, height: 20 }`
  - Assert resulting `x + 32 ≤ screen_width` (popup does not overflow right edge)

- [ ] **`tests/unit_popup.rs`** — `test_auto_hide_timer_fires`
  - Create a `PopupController` in a test context
  - Call `show()` with a mock position
  - Advance the mock timer by 4001 ms
  - Assert `is_visible()` returns `false`

- [ ] **`tests/integration_popup.rs`** — `test_popup_appears_on_selection_event`
  - Start `event_monitor::start()`
  - Open TextEdit and select `"hello world"` via AppleScript
  - Assert `popup::is_visible()` returns `true` within 500 ms

- [ ] **`tests/integration_popup.rs`** — `test_popup_hides_on_esc`
  - Show popup programmatically
  - Post a synthetic ESC key event via `CGEventPost`
  - Assert `popup::is_visible()` returns `false` within 200 ms

- [ ] **`tests/integration_popup.rs`** — `test_popup_hides_on_timeout`
  - Show popup programmatically
  - Wait 4200 ms
  - Assert `popup::is_visible()` returns `false`

- [ ] **`tests/integration_popup.rs`** — `test_popup_shows_success_icon_on_click`
  - Show popup at a known position
  - Simulate a click at the popup's frame origin via `CGEventPost`
  - Assert the `NSImageView` image name contains `"checkmark.circle.fill"` within 50 ms
  - Assert `popup::is_visible()` returns `false` within 300 ms

---

## Phase 5 — LLM Inference and Text Replacement `[ ]`

**Goal**: Lazy-load the bundled Qwen2.5-0.5B-Instruct-4bit model on first enable, correct selected text, and replace it in the source app via AX setValue with a clipboard fallback.

### 5.1 LLM backend — model loading `[ ]`

- [ ] Create `src/llm/backend.rs`:

```rust
pub struct LlmBackend {
    model: Option<Arc<Mutex<QwenModel>>>,
}

impl LlmBackend {
    pub fn new() -> Self { ... }

    /// Load the model from the bundle on first call; no-op on subsequent calls.
    pub fn ensure_loaded(&mut self) -> Result<()> { ... }

    /// Run inference with temperature=0.0 and max_tokens=input_len+50.
    pub fn infer(&self, prompt: &str) -> Result<String> { ... }
}
```

- [ ] In `ensure_loaded()`, load weights from `Contents/Resources/model/` relative to the running `.app` bundle path
- [ ] After loading, run a pre-warm pass with an empty prompt to compile the ANE/GPU pipeline
- [ ] `infer()` uses `temperature: 0.0` and `max_tokens: prompt.len() + 50`

### 5.2 Prompt builder and post-processor `[ ]`

- [ ] Create `src/llm/prompt.rs`:

```rust
/// Build a Qwen2.5 ChatML prompt with a strict system instruction.
pub fn build_prompt(text: &str) -> String { ... }

pub enum CorrectionResult {
    Changed(String),   // corrected text differs from original
    Unchanged,         // model output is identical to input → show checkmark, skip replacement
    Error(String),     // output is empty, too long, or otherwise invalid
}

/// Validate and clean raw model output.
pub fn post_process(original: &str, generated: &str) -> CorrectionResult { ... }
```

- [ ] `build_prompt` uses the Qwen2.5 ChatML format:

```
<|im_start|>system
Fix ONLY spelling, grammar, and basic punctuation. Return ONLY the corrected text, no explanations, no quotes.<|im_end|>
<|im_start|>user
{text}<|im_end|>
<|im_start|>assistant
```

- [ ] `post_process` rules:
  - Trim leading/trailing whitespace from `generated`
  - If trimmed == original → `Unchanged`
  - If trimmed is empty or `trimmed.len() > original.len() * 3` → `Error("over-generation")`
  - Otherwise → `Changed(trimmed)`

### 5.3 Corrector public API `[ ]`

- [ ] Create `src/llm/mod.rs`:

```rust
pub struct Corrector {
    backend: LlmBackend,
}

impl Corrector {
    pub fn new() -> Self { ... }

    pub fn correct(&mut self, text: &str) -> Result<CorrectionResult> {
        self.backend.ensure_loaded()?;
        let prompt = prompt::build_prompt(text);
        let raw = self.backend.infer(&prompt)?;
        Ok(prompt::post_process(text, &raw))
    }
}
```

- [ ] Declare a `static CORRECTOR: OnceLock<Mutex<Corrector>>` in `main.rs` for shared access across threads

### 5.4 Text replacement — AX primary `[ ]`

- [ ] Create `src/replacement.rs`
- [ ] Implement `replace_selected_text(element: &AXUIElement, new_text: &str) -> Result<()>`:
  - Call `AXUIElementSetAttributeValue(element, kAXSelectedTextAttribute, new_text)`
  - Return `Ok(())` on `kAXErrorSuccess`
  - Return `Err(...)` on any other result code

### 5.5 Text replacement — clipboard fallback `[ ]`

- [ ] Implement `clipboard_replace(new_text: &str) -> Result<()>`:
  - Save current `NSPasteboard::general()` contents
  - Write `new_text` to the pasteboard
  - Post `kVK_ANSI_V` key-down + key-up with `.maskCommand` via `CGEventPost`
  - After 200 ms, restore the original clipboard contents
- [ ] Implement `replace_with_fallback(element: &AXUIElement, new_text: &str) -> Result<()>`:
  - Try `replace_selected_text()` first
  - On `Err`, call `clipboard_replace()`

### 5.6 Wire popup click to correction `[ ]`

- [ ] In `popup.rs`, when the popup is clicked:
  - Read the last `SelectionInfo` from a shared `Arc<Mutex<Option<SelectionInfo>>>`
  - Dispatch correction on a background thread: `tokio::task::spawn_blocking(|| CORRECTOR.lock().correct(&text))`
  - On `CorrectionResult::Changed(corrected)`: call `replace_with_fallback(&element, &corrected)`, then `show_success()`
  - On `CorrectionResult::Unchanged`: call `show_success()` (no replacement needed)
  - On `CorrectionResult::Error(_)`: log with `log::error!`, call `hide()`

### 5.7 Docs `[ ]`

- [ ] Create `docs/phase-5/HOWTO.md` with:
  - **Prerequisites** — Model must be present in `QuillFix.app/Contents/Resources/model/`; run `bash scripts/download_model.sh` if absent
  - **Build + Run** — `bash scripts/bundle.sh && open QuillFix.app`
  - **Manual test** — Type `"teh quik brwon fox"` in TextEdit, select it → popup → click → text replaced with `"the quick brown fox"`
  - **Run tests** — `cargo test unit_prompt && cargo test unit_replacement && bash scripts/run_integration_tests.sh phase5`
  - **Expected result** — Correction in <500 ms on Apple Silicon M1+; AX replacement in TextEdit/Notes; clipboard fallback covers Chrome/VS Code

### Phase 5 Tests `[ ]`

- [ ] **`tests/unit_prompt.rs`** — `test_build_prompt_contains_system_instruction`
  - Assert result contains `"Fix ONLY spelling, grammar"`
  - Assert result contains `"<|im_start|>user"`
  - Assert result contains the input text `"teh cat"`

- [ ] **`tests/unit_prompt.rs`** — `test_post_process_unchanged`
  - `post_process("hello world", "hello world")` → `CorrectionResult::Unchanged`

- [ ] **`tests/unit_prompt.rs`** — `test_post_process_whitespace_trim_is_unchanged`
  - `post_process("hello world", "  hello world  ")` → `CorrectionResult::Unchanged` (trim equals original)

- [ ] **`tests/unit_prompt.rs`** — `test_post_process_changed`
  - `post_process("teh cat", "the cat")` → `CorrectionResult::Changed("the cat")`

- [ ] **`tests/unit_prompt.rs`** — `test_post_process_overgeneration`
  - `post_process("hi", &"x".repeat(200))` → `CorrectionResult::Error(...)`

- [ ] **`tests/unit_replacement.rs`** — `test_clipboard_replace_writes_to_pasteboard`
  - Call `clipboard_replace("corrected text")`
  - Read `NSPasteboard::general().string(forType: .string)` immediately after
  - Assert it equals `"corrected text"`

- [ ] **`tests/integration_llm.rs`** — `test_corrector_fixes_known_misspelling`
  - `Corrector::new().correct("teh quik brwon fox")` → `Changed("the quick brown fox")`
  - _(Loads the real model; allow up to 60 s on first run for pre-warm)_

- [ ] **`tests/integration_llm.rs`** — `test_corrector_preserves_correct_text`
  - `correct("The quick brown fox jumps over the lazy dog.")` → `Unchanged`

- [ ] **`tests/integration_llm.rs`** — `test_corrector_does_not_over_edit`
  - `correct("I am going to the store.")` → `Unchanged` or trivially `Changed`

- [ ] **`tests/integration_llm.rs`** — `test_ax_replacement_in_textedit`
  - Open TextEdit, type `"teh quik"`, select all via AppleScript
  - Call `replace_with_fallback(&element, "the quick")`
  - Read back TextEdit content via AppleScript
  - Assert content == `"the quick"`

---

## Phase 6 — Integration, Hardening, and End-to-End `[ ]`

**Goal**: All components wired together with full error handling, thread safety, performance guardrails, and verified end-to-end across real macOS apps.

### 6.1 Thread safety audit `[ ]`

- [ ] Audit every `AppKit` and `AXUIElement` call site — wrap any not already on the main thread with `dispatch_async(dispatch_get_main_queue(), ...)`
- [ ] Replace all `unwrap()` and `expect()` in production code paths with `anyhow::Result` propagation
- [ ] Add `#[cfg(test)]` guards to all test-only helpers so they are excluded from release builds

### 6.2 Error logging `[ ]`

- [ ] All `Err` results from AX calls, model inference, and replacement log via `log::error!` to oslog with structured fields: `{ phase, error, input_len }`
- [ ] No `println!` or `eprintln!` in release builds (guard with `#[cfg(debug_assertions)]` if needed)
- [ ] Add a hidden debug mode: holding the Option key while clicking the menu bar icon switches the log level to verbose in Console.app

### 6.3 Performance guardrails `[ ]`

- [ ] In `event_monitor.rs`, track callback frequency — if the CGEventTap fires more than 20 times/second, log a warning and temporarily increase the debounce window to 500 ms
- [ ] After each correction, check process RSS via `sysctl`; if RSS > 380 MB, log a warning and call `mlx_rs::clear_cache()`
- [ ] Verify at runtime that when `IS_ENABLED == false` the event tap is actually disabled (not just its output ignored)

### 6.4 App compatibility map `[ ]`

- [ ] Create `src/known_apps.rs`:

```rust
pub enum ReplacementStrategy { AX, Clipboard, Unsupported }

pub fn strategy_for_bundle_id(bundle_id: &str) -> ReplacementStrategy { ... }
```

- [ ] Populate with known entries: TextEdit/Notes/Mail → `AX`; Chrome/VS Code → `Clipboard`; Terminal → `Unsupported`
- [ ] When `replace_selected_text()` fails for a bundle ID not in the map, record it in `NSUserDefaults` for diagnostics

### 6.5 First-launch onboarding `[ ]`

- [ ] On first launch (check `NSUserDefaults` key `"quillfix.onboarded"`), present a native `NSAlert`:
  - Title: `"Welcome to QuillFix"`
  - Message: `"QuillFix needs Accessibility access to read and correct selected text."`
  - Buttons: `"Open Settings"` (calls `open_accessibility_settings()`) and `"Later"`
- [ ] After the user grants access, poll `AXIsProcessTrusted()` every 2 s for up to 30 s; on grant: auto-enable and show a second `NSAlert` confirming readiness
- [ ] Set `"quillfix.onboarded" = true` in `NSUserDefaults` once the flow completes

### 6.6 Integration test runner script `[ ]`

- [ ] Create `scripts/run_integration_tests.sh`:

```bash
#!/usr/bin/env bash
# Usage: bash scripts/run_integration_tests.sh [phase1|phase2|...|all]
# Requires: Accessibility permission granted to the test runner binary
set -euo pipefail
PHASE=${1:-all}
case "$PHASE" in
  phase1) cargo test --test integration_bundle ;;
  phase2) cargo test --test integration_menu_bar ;;
  phase3) cargo test --test integration_event_monitor ;;
  phase4) cargo test --test integration_popup ;;
  phase5) cargo test --test integration_llm ;;
  phase6) cargo test --test integration_e2e ;;
  all)    cargo test ;;
esac
```

- [ ] `chmod +x scripts/run_integration_tests.sh`

### 6.7 Docs `[ ]`

- [ ] Create `docs/phase-6/HOWTO.md` with:
  - **Build release** — `bash scripts/bundle.sh`
  - **Run** — `open QuillFix.app`
  - **Full test suite** — `cargo test && bash scripts/run_integration_tests.sh all`
  - **App compatibility matrix** — manual steps to verify in TextEdit, Notes, Chrome, VS Code
  - **Performance check** — how to observe CPU/RAM in Activity Monitor at idle and during correction
  - **Debug mode** — hold Option + click menu bar icon → verbose logs in Console.app

### Phase 6 Tests `[ ]`

- [ ] **`tests/integration_e2e.rs`** — `test_full_correction_in_textedit`
  - Launch `QuillFix.app` and enable via programmatic menu click
  - Open TextEdit (blank document); type `"I hav a gret idear"` via AppleScript; select all
  - Assert `popup::is_visible()` becomes `true` within 1 s
  - Simulate click on popup; wait up to 2 s for correction
  - Read TextEdit content via AppleScript
  - Assert content equals `"I have a great idea"` (or equivalent corrected form)
  - Assert no crash and no zombie process

- [ ] **`tests/integration_e2e.rs`** — `test_performance_idle_cpu`
  - Launch `QuillFix.app` with the feature enabled; no text selected for 5 s
  - Sample CPU using `top -pid $PID -l 5`
  - Assert average CPU < 1.0%

- [ ] **`tests/integration_e2e.rs`** — `test_performance_peak_ram`
  - Trigger one full correction cycle (forces model load)
  - Sample RSS via `ps -o rss= -p $PID`
  - Assert RSS < 409600 KB (400 MB)

- [ ] **`tests/integration_e2e.rs`** — `test_correction_in_notes`
  - Open Notes; create a new note; type `"speling mistaeks"` via AppleScript; select all
  - Wait for popup; simulate click; read Notes content
  - Assert content equals `"spelling mistakes"`

- [ ] **`tests/integration_e2e.rs`** — `test_correction_in_chrome_input`
  - Open Chrome (skip with `#[ignore]` if not installed)
  - Navigate to `data:text/html,<input id=t>`; type `"Ths is wrng"` via AppleScript; select all
  - Wait for popup; simulate click; read input value
  - Assert value is corrected

- [ ] **`tests/integration_e2e.rs`** — `test_password_field_no_popup`
  - Open a test window with `NSSecureTextField` programmatically
  - Type and select text
  - Assert `popup::is_visible()` never becomes `true` within 1 s

- [ ] **`tests/integration_e2e.rs`** — `test_long_text_no_popup`
  - Select a 1600-character string in TextEdit
  - Assert popup does not appear within 1 s

- [ ] **`tests/integration_e2e.rs`** — `test_toggle_off_stops_monitoring`
  - Enable feature; confirm popup fires for a selection
  - Disable feature via menu
  - Make a new selection → assert popup does NOT appear within 1 s

---

## Overall Completion Checklist

- [ ] Phase 1 — Project Setup: all tests passing
- [ ] Phase 2 — Menu Bar UI: all tests passing
- [ ] Phase 3 — Event Monitoring: all tests passing
- [ ] Phase 4 — Popup Display: all tests passing
- [ ] Phase 5 — LLM + Replacement: all tests passing
- [ ] Phase 6 — Integration + Hardening: all tests passing
- [ ] `cargo test` exits 0 with zero failures
- [ ] `bash scripts/run_integration_tests.sh all` exits 0
- [ ] `codesign --verify --deep QuillFix.app` exits 0
- [ ] Activity Monitor confirms CPU < 1% idle and RAM < 400 MB peak
- [ ] All `docs/phase-N/HOWTO.md` files present and accurate
