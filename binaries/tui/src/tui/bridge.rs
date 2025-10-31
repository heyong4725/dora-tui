use std::{sync::Arc, time::Instant};

#[cfg(feature = "protocol")]
use std::{
    sync::{Mutex, mpsc},
    thread,
};

use tui_interface::{CoordinatorClient, LegacyCliService, PreferencesStore, TelemetryService};
#[cfg(all(feature = "tui-cli-services", not(feature = "protocol")))]
use tui_interface::{InterfaceError, UserPreferencesSnapshot};

#[cfg(all(feature = "tui-cli-services", not(feature = "protocol")))]
use crate::{
    LOCALHOST,
    common::{connect_to_coordinator, query_running_dataflows},
    config::preferences::UserPreferences,
    execute_legacy_command,
    tui::{app::dataflow_from_entry, metrics::MetricsCollector},
};

#[cfg(all(feature = "tui-cli-services", not(feature = "protocol")))]
use dora_core::topics::DORA_COORDINATOR_PORT_CONTROL_DEFAULT;

#[cfg(all(feature = "tui-cli-services", not(feature = "protocol")))]
use tui_interface::DataflowSummary;

#[cfg(feature = "protocol")]
use dora_protocol::SystemMetrics as ProtocolSystemMetrics;
#[cfg(feature = "protocol")]
use dora_protocol_client::ProtocolClients;
#[cfg(feature = "protocol")]
use tracing::{error, warn};
#[cfg(feature = "protocol")]
use uuid::Uuid;

#[cfg(any(feature = "tui-cli-services", feature = "protocol"))]
pub struct ServiceBundle {
    pub preferences_store: Arc<dyn PreferencesStore>,
    pub coordinator_client: Arc<dyn CoordinatorClient>,
    pub telemetry_service: Arc<dyn TelemetryService>,
    pub legacy_cli_service: Arc<dyn LegacyCliService>,
    #[cfg(feature = "protocol")]
    pub protocol_clients: Arc<ProtocolClients>,
    #[cfg(feature = "protocol")]
    pub metrics_cache: Arc<Mutex<Option<tui_interface::SystemMetrics>>>,
}

#[cfg(all(not(feature = "protocol"), feature = "tui-cli-services"))]
pub fn default_service_bundle() -> ServiceBundle {
    ServiceBundle {
        preferences_store: Arc::new(CliPreferencesStore),
        coordinator_client: Arc::new(CliCoordinatorClient),
        telemetry_service: Arc::new(CliTelemetryService::default()),
        legacy_cli_service: Arc::new(CliLegacyCliService),
    }
}

#[cfg(feature = "protocol")]
pub fn default_service_bundle() -> ServiceBundle {
    let base_url =
        std::env::var("DORA_PROTOCOL_URL").unwrap_or_else(|_| "http://127.0.0.1:7267".to_string());
    let clients = Arc::new(ProtocolClients::new(&base_url).unwrap_or_else(|err| {
        panic!("failed to initialize protocol clients for {base_url}: {err}")
    }));

    let metrics_cache = spawn_metrics_stream(Arc::clone(&clients));

    ServiceBundle {
        preferences_store: clients.preferences_store(),
        coordinator_client: clients.coordinator_client(),
        telemetry_service: clients.telemetry_service(),
        legacy_cli_service: clients.legacy_cli_service(),
        protocol_clients: clients,
        metrics_cache,
    }
}

#[cfg(all(feature = "tui-cli-services", not(feature = "protocol")))]
#[derive(Debug, Default, Clone)]
struct CliPreferencesStore;

#[cfg(all(feature = "tui-cli-services", not(feature = "protocol")))]
impl PreferencesStore for CliPreferencesStore {
    fn load(&self) -> Result<UserPreferencesSnapshot, InterfaceError> {
        let prefs = UserPreferences::load_or_create()
            .map_err(|err| InterfaceError::from(err.to_string()))?;

        Ok(UserPreferencesSnapshot {
            theme: prefs.interface.tui.theme.clone(),
            auto_refresh_interval_secs: prefs.interface.tui.auto_refresh_interval.as_secs(),
            show_system_info: prefs.interface.hints.show_hints,
            default_view: Some(prefs.interface.tui.default_view.clone()),
        })
    }

    fn save(&self, snapshot: &UserPreferencesSnapshot) -> Result<(), InterfaceError> {
        let mut prefs = UserPreferences::load_or_create()
            .map_err(|err| InterfaceError::from(err.to_string()))?;

        prefs.interface.tui.theme = snapshot.theme.clone();
        prefs.interface.tui.auto_refresh_interval =
            Duration::from_secs(snapshot.auto_refresh_interval_secs.max(1));
        prefs.interface.hints.show_hints = snapshot.show_system_info;

        if let Some(default_view) = snapshot.default_view.clone() {
            prefs.interface.tui.default_view = default_view;
        }

        prefs
            .save()
            .map_err(|err| InterfaceError::from(err.to_string()))
    }
}

#[cfg(all(feature = "tui-cli-services", not(feature = "protocol")))]
#[derive(Default, Debug)]
struct CliCoordinatorClient;

#[cfg(all(feature = "tui-cli-services", not(feature = "protocol")))]
impl CoordinatorClient for CliCoordinatorClient {
    fn list_dataflows(&self) -> Result<Vec<DataflowSummary>, InterfaceError> {
        let coordinator_addr = (LOCALHOST, DORA_COORDINATOR_PORT_CONTROL_DEFAULT).into();
        let mut session = connect_to_coordinator(coordinator_addr)
            .map_err(|err| InterfaceError::from(err.to_string()))?;
        let list = query_running_dataflows(&mut *session)
            .map_err(|err| InterfaceError::from(err.to_string()))?;

        Ok(list.0.into_iter().map(dataflow_from_entry).collect())
    }
}

#[cfg(all(feature = "tui-cli-services", not(feature = "protocol")))]
#[derive(Default, Debug)]
pub struct CliTelemetryService;

#[cfg(all(feature = "tui-cli-services", not(feature = "protocol")))]
impl TelemetryService for CliTelemetryService {
    fn latest_metrics(&self) -> Result<tui_interface::SystemMetrics, InterfaceError> {
        let mut collector = MetricsCollector::new();
        collector
            .collect()
            .map_err(|err| InterfaceError::from(err.to_string()))
    }
}

#[cfg(all(feature = "tui-cli-services", not(feature = "protocol")))]
#[derive(Default, Debug)]
struct CliLegacyCliService;

#[cfg(all(feature = "tui-cli-services", not(feature = "protocol")))]
impl LegacyCliService for CliLegacyCliService {
    fn execute(
        &self,
        argv: &[String],
        working_dir: &std::path::Path,
    ) -> Result<(), InterfaceError> {
        execute_legacy_command(argv.iter().map(|s| s.as_str()), Some(working_dir))
            .map_err(|err| InterfaceError::from(err.to_string()))
    }
}

#[cfg(feature = "protocol")]
fn spawn_metrics_stream(
    clients: Arc<ProtocolClients>,
) -> Arc<Mutex<Option<tui_interface::SystemMetrics>>> {
    let cache = Arc::new(Mutex::new(None));
    let cache_clone = Arc::clone(&cache);

    thread::spawn(move || match clients.system_metrics_stream() {
        Ok(stream) => {
            for next in stream {
                match next {
                    Ok(raw) => {
                        let metrics = convert_metrics(&raw);
                        if let Ok(mut guard) = cache_clone.lock() {
                            *guard = Some(metrics);
                        }
                    }
                    Err(err) => {
                        warn!("system metrics stream ended: {err}");
                        break;
                    }
                }
            }
        }
        Err(err) => warn!("failed to start system metrics stream: {err}"),
    });

    cache
}

#[cfg(feature = "protocol")]
fn convert_metrics(protocol: &ProtocolSystemMetrics) -> tui_interface::SystemMetrics {
    let load_average = protocol
        .load_average
        .map(|avg| tui_interface::LoadAverages {
            one: f64::from(avg[0]),
            five: f64::from(avg[1]),
            fifteen: f64::from(avg[2]),
        });

    tui_interface::SystemMetrics {
        cpu_usage: protocol.cpu_percent,
        memory_usage: protocol.memory_percent,
        memory: tui_interface::MemoryMetrics {
            total_bytes: protocol.total_memory_bytes,
            used_bytes: protocol.used_memory_bytes,
            free_bytes: protocol
                .total_memory_bytes
                .saturating_sub(protocol.used_memory_bytes),
            usage_percent: protocol.memory_percent,
            ..Default::default()
        },
        load_average,
        last_update: Some(Instant::now()),
        ..Default::default()
    }
}

#[cfg(feature = "protocol")]
pub fn spawn_protocol_log_stream(
    clients: Arc<ProtocolClients>,
    dataflow_id: Uuid,
) -> Option<mpsc::Receiver<dora_protocol::LogEvent>> {
    match clients.log_stream(&dataflow_id) {
        Ok(stream) => {
            let (tx, rx) = mpsc::channel();
            thread::spawn(move || {
                for item in stream {
                    match item {
                        Ok(event) => {
                            if tx.send(event).is_err() {
                                break;
                            }
                        }
                        Err(err) => {
                            warn!(target: "tui", "log stream terminated: {err}");
                            break;
                        }
                    }
                }
            });
            Some(rx)
        }
        Err(err) => {
            error!(target: "tui", "failed to open log stream: {err}");
            None
        }
    }
}
