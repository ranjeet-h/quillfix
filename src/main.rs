mod debounce;
mod event_monitor;
mod menu_bar;
mod permissions;
mod popup;
mod replacement;

mod llm;

use anyhow::Result;
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

#[tokio::main]
async fn main() -> Result<()> {
    init_logging();

    tracing::info!("quillfix startup");
    let state = permissions::accessibility_state();
    tracing::info!(?state, "accessibility status");

    menu_bar::run()?;
    Ok(())
}
