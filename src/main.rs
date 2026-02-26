mod corrector;
mod llm;
mod menu_bar;
mod permissions;

use tracing_subscriber::prelude::*;

fn init_logging() {
    #[cfg(target_os = "macos")]
    {
        let subscriber = tracing_subscriber::fmt()
            .with_target(true)
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .finish()
            .with(tracing_oslog::OsLogger::new("com.quillfix.app", "default"));

        let _ = tracing::subscriber::set_global_default(subscriber);
    }

    #[cfg(not(target_os = "macos"))]
    {
        let subscriber = tracing_subscriber::fmt()
            .with_target(true)
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .finish();

        let _ = tracing::subscriber::set_global_default(subscriber);
    }
}

fn main() {
    init_logging();

    tracing::info!(phase = "startup", "quillfix startup");

    // Check permissions
    let state = permissions::accessibility_state();
    tracing::info!(phase = "permissions", ?state, "accessibility status");

    if state == permissions::PermissionState::Denied {
        tracing::warn!(phase = "permissions", "accessibility denied");
    }

    // Check onboarding
    let onboarded = permissions::run_first_launch_onboarding(
        std::time::Duration::from_secs(2),
        std::time::Duration::from_millis(250),
    );
    tracing::info!(phase = "onboarding", onboarded, "first-launch onboarding checked");

    // Pre-warm the LLM backend on a background thread so the first correction
    // is instant and the main run-loop is never blocked by model loading.
    let corrector_ref = corrector::get();
    std::thread::spawn(move || {
        tracing::info!(phase = "llm", "pre-warming backend on background thread");
        match corrector_ref.lock() {
            Ok(c) => match c.ensure_loaded() {
                Ok(()) => tracing::info!(phase = "llm", "backend ready"),
                Err(e) => tracing::error!(phase = "llm", ?e, "backend warm-up failed"),
            },
            Err(e) => tracing::error!(phase = "llm", ?e, "corrector lock poisoned"),
        }
    });

    // Setup menu bar
    menu_bar::run();

    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
        use objc2_foundation::MainThreadMarker;

        if let Some(mtm) = MainThreadMarker::new() {
            let app = NSApplication::sharedApplication(mtm);
            let _ = app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

            // Register the NSServices provider so "Correct with QuillFix"
            // appears in Keyboard > Keyboard Shortcuts > Services.
            menu_bar::register_services(&app);

            app.run();
        }
    }
}
