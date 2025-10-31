# Dora TUI

  Standalone workspace for the Dora terminal UI. It bundles the shared TUI interface crate, the protocol client SDK, and the `dora-tui` binary that
  connects to a running Dora coordinator via HTTP/SSE.

  ## Repository Layout


  .
  ├── binaries/
  │   └── tui/               # dora-tui binary (ratatui-based)
  ├── crates/
  │   ├── dora-protocol-client/  # HTTP/SSE client for coordinator gateway
  │   └── tui-interface/         # Data contracts & service traits
  ├── docs/                 # ADRs, roadmap, usage notes
  ├── examples/             # Optional sample configs
  └── Cargo.toml            # Workspace definition


  ## Quick Start

  1. Ensure a Dora coordinator + gateway are running (or point at an existing cluster).
2. Clone this repository and run:

   ```bash
   cargo run -p dora-tui --features protocol --
   ```

   By default the TUI connects to `http://127.0.0.1:7267`. Override with:

   ```bash
   DORA_PROTOCOL_URL=http://hostname:7267 cargo run -p dora-tui --features protocol --
   ```

   Use `--view <dashboard|logs|monitor>` to pick the initial screen.

   > Note: the legacy `:` command mode is currently disabled in this standalone build.
  ## Development

  - Format and lint: cargo fmt --all && cargo clippy --all-targets --all-features
  - Test everything: cargo test --all --workspace
  - The workspace requires Rust 1.85.0 or newer (see rust-toolchain.toml if present).

  ## Crates

  - tui-interface: public data contracts and service traits used by the TUI and other clients.
  - dora-protocol-client: Rust client for the Dora protocol gateway (HTTP/JSON plus SSE streams).
  - tui binary: the ratatui-based UI that consumes the protocol client.

  ## Contributing

  Issues and pull requests are welcome! Please run the checks above before submitting. For roadmap,
  protocol details, and design decisions, see the documents under docs/.

  License: Apache-2.0.
