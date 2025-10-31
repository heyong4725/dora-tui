pub mod tui;

use anyhow::Result;
use tui::app::ViewType;
use tui::bridge::default_service_bundle;

pub fn run_tui(initial_view: ViewType, protocol_url: Option<&str>) -> Result<()> {
    if let Some(url) = protocol_url {
        unsafe {
            std::env::set_var("DORA_PROTOCOL_URL", url);
        }
    }

    let bundle = default_service_bundle();
    let mut app = tui::app::DoraApp::from_service_bundle(initial_view, bundle);

    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async move {
        app.run()
            .await
            .map_err(|err| anyhow::Error::msg(err.to_string()))
    })?;
    Ok(())
}
