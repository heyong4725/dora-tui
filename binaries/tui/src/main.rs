use anyhow::Result;
use clap::Parser;
use dora_tui::{run_tui, tui::ViewType};

#[derive(Debug, Parser)]
#[command(name = "dora-tui", version, about = "Dora Terminal UI")]
struct Cli {
    /// Override the protocol gateway URL (default: http://127.0.0.1:7267)
    #[arg(long, env = "DORA_PROTOCOL_URL")]
    protocol_url: Option<String>,

    /// Initial view (dashboard, logs, etc.)
    #[arg(long, default_value = "dashboard")]
    view: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let view = match cli.view.as_str() {
        "logs" => ViewType::LogViewer {
            target: "system".to_string(),
        },
        "monitor" => ViewType::SystemMonitor,
        _ => ViewType::Dashboard,
    };

    run_tui(view, cli.protocol_url.as_deref())
}
