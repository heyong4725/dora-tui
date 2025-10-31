mod error;

use std::{
    io::{BufRead, BufReader, Lines},
    sync::Arc,
};

use chrono::Utc;
use reqwest::blocking::{Client, Response};
use serde::{Serialize, de::DeserializeOwned};
use tui_interface::{
    CoordinatorClient, DataflowSummary as UiDataflowSummary, InterfaceError, LegacyCliService,
    NodeSummary, PreferencesStore, SystemMetrics as UiSystemMetrics, TelemetryService,
    UserPreferencesSnapshot as UiPreferencesSnapshot,
};
use url::Url;

use dora_protocol::{
    DataflowSummary, NodeDescriptor, NodeKind, NodeSource, NodeStatus, SystemMetrics,
    UserPreferencesSnapshot,
};
use uuid::Uuid;

#[derive(Clone)]
pub struct ProtocolClients {
    transport: Arc<Transport>,
}

impl ProtocolClients {
    pub fn new(base_url: impl AsRef<str>) -> Result<Self, error::ProtocolClientError> {
        let base = normalize_base_url(base_url.as_ref())?;
        let client = Client::builder().no_proxy().build()?;
        Ok(Self {
            transport: Arc::new(Transport { client, base }),
        })
    }

    pub fn coordinator_client(&self) -> Arc<dyn CoordinatorClient> {
        Arc::new(ProtocolCoordinatorClient {
            transport: Arc::clone(&self.transport),
        })
    }

    pub fn telemetry_service(&self) -> Arc<dyn TelemetryService> {
        Arc::new(ProtocolTelemetryService {
            transport: Arc::clone(&self.transport),
        })
    }

    pub fn preferences_store(&self) -> Arc<dyn PreferencesStore> {
        Arc::new(ProtocolPreferencesStore {
            transport: Arc::clone(&self.transport),
        })
    }

    pub fn legacy_cli_service(&self) -> Arc<dyn LegacyCliService> {
        Arc::new(ProtocolLegacyCliService)
    }

    pub fn log_stream(&self, dataflow_id: &Uuid) -> Result<LogStream, error::ProtocolClientError> {
        let response = self
            .transport
            .get_stream(&format!("/v1/logs/{dataflow_id}/stream"))?;
        Ok(LogStream::new(response))
    }

    pub fn system_metrics_stream(&self) -> Result<SystemMetricsStream, error::ProtocolClientError> {
        let response = self.transport.get_stream("/v1/telemetry/system/stream")?;
        Ok(SystemMetricsStream::new(response))
    }
}

struct Transport {
    client: Client,
    base: Url,
}

impl Transport {
    fn endpoint(&self, path: &str) -> Result<Url, error::ProtocolClientError> {
        let normalized = path.strip_prefix('/').unwrap_or(path);
        Ok(self.base.join(normalized)?)
    }

    fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, error::ProtocolClientError> {
        let url = self.endpoint(path)?;
        let response = self.client.get(url).send()?.error_for_status()?;
        Ok(response.json()?)
    }

    fn put<B: Serialize>(&self, path: &str, body: &B) -> Result<(), error::ProtocolClientError> {
        let url = self.endpoint(path)?;
        self.client.put(url).json(body).send()?.error_for_status()?;
        Ok(())
    }

    fn get_stream(&self, path: &str) -> Result<Response, error::ProtocolClientError> {
        let url = self.endpoint(path)?;
        let response = self.client.get(url).send()?.error_for_status()?;
        Ok(response)
    }
}

fn normalize_base_url(raw: &str) -> Result<Url, error::ProtocolClientError> {
    let mut parsed = Url::parse(raw)?;
    if !parsed.path().ends_with('/') {
        let mut path = parsed.path().to_owned();
        if !path.ends_with('/') {
            path.push('/');
        }
        parsed.set_path(&path);
    }
    Ok(parsed)
}

#[derive(Clone)]
struct ProtocolCoordinatorClient {
    transport: Arc<Transport>,
}

impl CoordinatorClient for ProtocolCoordinatorClient {
    fn list_dataflows(&self) -> Result<Vec<UiDataflowSummary>, InterfaceError> {
        let list: Vec<DataflowSummary> = self
            .transport
            .get("/v1/dataflows")
            .map_err(InterfaceError::from_proto_error)?;

        Ok(list.into_iter().map(map_summary_to_ui).collect())
    }
}

#[derive(Clone)]
struct ProtocolTelemetryService {
    transport: Arc<Transport>,
}

impl TelemetryService for ProtocolTelemetryService {
    fn latest_metrics(&self) -> Result<UiSystemMetrics, InterfaceError> {
        let snapshot: SystemMetrics = self
            .transport
            .get("/v1/telemetry/system")
            .map_err(InterfaceError::from_proto_error)?;
        Ok(map_metrics_to_ui(snapshot))
    }
}

#[derive(Clone)]
struct ProtocolPreferencesStore {
    transport: Arc<Transport>,
}

impl PreferencesStore for ProtocolPreferencesStore {
    fn load(&self) -> Result<UiPreferencesSnapshot, InterfaceError> {
        let snapshot: UserPreferencesSnapshot = self
            .transport
            .get("/v1/preferences/ui")
            .map_err(InterfaceError::from_proto_error)?;
        Ok(map_preferences_to_ui(snapshot))
    }

    fn save(&self, prefs: &UiPreferencesSnapshot) -> Result<(), InterfaceError> {
        let payload = map_preferences_to_protocol(prefs);
        self.transport
            .put("/v1/preferences/ui", &payload)
            .map_err(InterfaceError::from_proto_error)
    }
}

#[derive(Clone)]
struct ProtocolLegacyCliService;

impl LegacyCliService for ProtocolLegacyCliService {
    fn execute(
        &self,
        _argv: &[String],
        _working_dir: &std::path::Path,
    ) -> Result<(), InterfaceError> {
        Err(InterfaceError::Unimplemented)
    }
}

fn map_summary_to_ui(summary: DataflowSummary) -> UiDataflowSummary {
    let id = summary.id;
    let name = summary.name.clone().unwrap_or_else(|| id.to_string());

    let nodes = summary.nodes.into_iter().map(map_node_to_ui).collect();

    UiDataflowSummary {
        id: id.to_string(),
        name,
        status: format_status(summary.status),
        nodes,
    }
}

fn map_node_to_ui(node: NodeDescriptor) -> NodeSummary {
    NodeSummary {
        id: node.id,
        name: node.name.unwrap_or_default(),
        status: format_node_status(node.status),
        kind: format_node_kind(node.kind),
        description: node.description,
        inputs: node.inputs,
        outputs: node.outputs,
        source: describe_node_source(&node.source),
        details: None,
    }
}

fn describe_node_source(source: &NodeSource) -> Option<String> {
    match source {
        NodeSource::Local { path } => path.clone(),
        NodeSource::Git { repo, rev } => Some(match rev {
            Some(rev) => format!("{repo} ({rev})"),
            None => repo.clone(),
        }),
        NodeSource::Wasm { module } => Some(module.clone()),
        NodeSource::Python {
            module,
            environment,
        } => environment
            .as_ref()
            .map(|env| format!("{module} (env: {env})"))
            .or_else(|| Some(module.clone())),
        NodeSource::Unknown => None,
    }
}

fn format_status(status: dora_protocol::DataflowStatus) -> String {
    match status {
        dora_protocol::DataflowStatus::Pending => "pending".into(),
        dora_protocol::DataflowStatus::Running => "running".into(),
        dora_protocol::DataflowStatus::Stopped => "stopped".into(),
        dora_protocol::DataflowStatus::Destroyed => "destroyed".into(),
        dora_protocol::DataflowStatus::Failed => "failed".into(),
        dora_protocol::DataflowStatus::Unknown => "unknown".into(),
    }
}

fn format_node_kind(kind: NodeKind) -> String {
    match kind {
        NodeKind::Runtime => "runtime".into(),
        NodeKind::Operator => "operator".into(),
        NodeKind::Custom => "custom".into(),
    }
}

fn format_node_status(status: NodeStatus) -> String {
    match status {
        NodeStatus::Initializing => "initializing".into(),
        NodeStatus::Running => "running".into(),
        NodeStatus::Stopped => "stopped".into(),
        NodeStatus::Failed => "failed".into(),
        NodeStatus::Unknown => "unknown".into(),
    }
}

fn map_metrics_to_ui(snapshot: SystemMetrics) -> UiSystemMetrics {
    let load_average = snapshot.load_average.map(|load| tui_interface::LoadAverages {
        one: f64::from(load[0]),
        five: f64::from(load[1]),
        fifteen: f64::from(load[2]),
    });

    UiSystemMetrics {
        cpu_usage: snapshot.cpu_percent,
        memory_usage: snapshot.memory_percent,
        memory: tui_interface::MemoryMetrics {
            total_bytes: snapshot.total_memory_bytes,
            used_bytes: snapshot.used_memory_bytes,
            free_bytes: snapshot
                .total_memory_bytes
                .saturating_sub(snapshot.used_memory_bytes),
            usage_percent: snapshot.memory_percent,
            ..Default::default()
        },
        load_average,
        ..Default::default()
    }
}

fn map_preferences_to_ui(snapshot: UserPreferencesSnapshot) -> UiPreferencesSnapshot {
    UiPreferencesSnapshot {
        theme: snapshot.theme.unwrap_or_else(|| "auto".to_string()),
        auto_refresh_interval_secs: if snapshot.auto_refresh.unwrap_or(true) {
            1
        } else {
            0
        },
        show_system_info: snapshot.auto_refresh.unwrap_or(true),
        default_view: snapshot
            .ui_mode
            .map(|mode| format!("{mode:?}").to_lowercase()),
    }
}

fn map_preferences_to_protocol(prefs: &UiPreferencesSnapshot) -> UserPreferencesSnapshot {
    let auto_refresh = prefs.auto_refresh_interval_secs > 0;
    UserPreferencesSnapshot {
        theme: Some(prefs.theme.clone()),
        ui_mode: None,
        auto_refresh: Some(auto_refresh),
        updated_at: Utc::now(),
    }
}

impl From<error::ProtocolClientError> for InterfaceError {
    fn from(value: error::ProtocolClientError) -> Self {
        InterfaceError::Message(value.to_string())
    }
}

trait InterfaceErrorExt {
    fn from_proto_error(err: error::ProtocolClientError) -> InterfaceError;
}

impl InterfaceErrorExt for InterfaceError {
    fn from_proto_error(err: error::ProtocolClientError) -> InterfaceError {
        InterfaceError::Message(err.to_string())
    }
}

pub use error::ProtocolClientError;

pub struct LogStream {
    lines: Lines<Box<dyn BufRead + Send>>,
    buffer: Vec<String>,
}

impl LogStream {
    fn new(response: Response) -> Self {
        let reader: Box<dyn BufRead + Send> = Box::new(BufReader::new(response));
        let lines = reader.lines();
        Self {
            lines,
            buffer: Vec::new(),
        }
    }
}

impl Iterator for LogStream {
    type Item = Result<dora_protocol::LogEvent, error::ProtocolClientError>;

    fn next(&mut self) -> Option<Self::Item> {
        read_next_event(&mut self.lines, &mut self.buffer).map(|res| {
            res.and_then(|payload| {
                serde_json::from_str(&payload)
                    .map_err(error::ProtocolClientError::Deserialize)
            })
        })
    }
}

pub struct SystemMetricsStream {
    lines: Lines<Box<dyn BufRead + Send>>,
    buffer: Vec<String>,
}

impl SystemMetricsStream {
    fn new(response: Response) -> Self {
        let reader: Box<dyn BufRead + Send> = Box::new(BufReader::new(response));
        let lines = reader.lines();
        Self {
            lines,
            buffer: Vec::new(),
        }
    }
}

impl Iterator for SystemMetricsStream {
    type Item = Result<SystemMetrics, error::ProtocolClientError>;

    fn next(&mut self) -> Option<Self::Item> {
        read_next_event(&mut self.lines, &mut self.buffer).map(|res| {
            res.and_then(|payload| {
                serde_json::from_str(&payload)
                    .map_err(error::ProtocolClientError::Deserialize)
            })
        })
    }
}

fn read_next_event(
    lines: &mut Lines<Box<dyn BufRead + Send>>,
    buffer: &mut Vec<String>,
) -> Option<Result<String, error::ProtocolClientError>> {
    for line_result in lines.by_ref() {
        match line_result {
            Ok(line) => {
                let trimmed = line.trim_end().to_string();
                if trimmed.is_empty() {
                    if buffer.is_empty() {
                        continue;
                    }
                    let payload = buffer.join("\n");
                    buffer.clear();
                    return Some(Ok(payload));
                }
                if let Some(rest) = trimmed.strip_prefix("data:") {
                    buffer.push(rest.trim_start().to_string());
                }
            }
            Err(err) => return Some(Err(err.into())),
        }
    }

    if buffer.is_empty() {
        None
    } else {
        let payload = buffer.join("\n");
        buffer.clear();
        Some(Ok(payload))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::io::Cursor;

    #[test]
    fn parse_single_sse_event() {
        let data = b"event: log\ndata: {\"message\":\"hello\"}\n\n";
        let reader: Box<dyn BufRead + Send> = Box::new(Cursor::new(&data[..]));
        let mut lines = reader.lines();
        let mut buffer = Vec::new();

        let payload = read_next_event(&mut lines, &mut buffer)
            .expect("expected event")
            .expect("payload parse");

        assert_eq!(payload, "{\"message\":\"hello\"}");
    }

    #[test]
    fn log_stream_yields_structured_events() {
        let timestamp = Utc::now().to_rfc3339();
        let sse_frame = format!(
            "data: {{\"timestamp\":\"{timestamp}\",\"level\":\"INFO\",\"node\":null,\"line\":\"ready\"}}\n\n"
        );
        let reader: Box<dyn BufRead + Send> = Box::new(Cursor::new(sse_frame.into_bytes()));
        let lines = reader.lines();
        let mut stream = LogStream {
            lines,
            buffer: Vec::new(),
        };

        let event = stream.next().expect("event present").expect("event parsed");
        assert_eq!(event.line, "ready");
        assert_eq!(event.level, dora_protocol::LogLevel::Info);
        assert!(stream.next().is_none());
    }

    #[test]
    fn system_metrics_stream_parses_multiple_frames() {
        let timestamp = Utc::now().to_rfc3339();
        let payload = format!(
            "data: {{\"timestamp\":\"{timestamp}\",\"cpu_percent\":12.5,\"memory_percent\":42.0,\"total_memory_bytes\":8192,\"used_memory_bytes\":4096,\"load_average\":[0.1,0.2,0.3]}}\n\n\
             data: {{\"timestamp\":\"{timestamp}\",\"cpu_percent\":20.0,\"memory_percent\":50.0,\"total_memory_bytes\":8192,\"used_memory_bytes\":4096,\"load_average\":null}}\n\n"
        );
        let reader: Box<dyn BufRead + Send> = Box::new(Cursor::new(payload.into_bytes()));
        let lines = reader.lines();
        let mut stream = SystemMetricsStream {
            lines,
            buffer: Vec::new(),
        };

        let first = stream.next().expect("first frame").expect("first parsed");
        assert_eq!(first.cpu_percent, 12.5);
        assert_eq!(first.memory_percent, 42.0);

        let second = stream.next().expect("second frame").expect("second parsed");
        assert_eq!(second.cpu_percent, 20.0);
        assert_eq!(second.load_average, None);

        assert!(stream.next().is_none());
    }
}
