//! Protocol data contracts for Dora gateway interactions.
//!
//! These types mirror the transport-level schema described in ADR-002 and
//! are shared between the protocol gateway and client SDKs.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Summary information for a known dataflow.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DataflowSummary {
    pub id: Uuid,
    pub name: Option<String>,
    pub status: DataflowStatus,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub nodes: Vec<NodeDescriptor>,
}

/// Detailed information about a dataflow, including node metadata.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DataflowDetail {
    pub summary: DataflowSummary,
    pub nodes: Vec<NodeDescriptor>,
}

/// Known lifecycle states for a dataflow.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataflowStatus {
    Pending,
    Running,
    Stopped,
    Destroyed,
    Failed,
    Unknown,
}

/// Metadata describing a node within a dataflow.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NodeDescriptor {
    pub id: String,
    pub name: Option<String>,
    pub status: NodeStatus,
    pub kind: NodeKind,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub description: Option<String>,
    pub source: NodeSource,
}

/// High-level node status enum.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeStatus {
    Initializing,
    Running,
    Stopped,
    Failed,
    Unknown,
}

/// Core node type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Runtime,
    Operator,
    Custom,
}

/// Node source information.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NodeSource {
    Local {
        path: Option<String>,
    },
    Git {
        repo: String,
        rev: Option<String>,
    },
    Wasm {
        module: String,
    },
    Python {
        module: String,
        environment: Option<String>,
    },
    Unknown,
}

/// Handle to an asynchronous operation (start/stop/destroy).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationHandle {
    pub handle: String,
    pub submitted_at: DateTime<Utc>,
}

/// Request payload for starting a new dataflow via the protocol gateway.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StartDataflowRequest {
    /// Raw YAML descriptor to launch.
    pub descriptor: String,
    /// Optional name to assign when launching.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Whether to enable UV mode for Python nodes.
    #[serde(default)]
    pub uv: bool,
}

/// State of an asynchronous operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationState {
    Pending,
    Running,
    Completed,
    Failed,
}

/// Status payload for asynchronous operations.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OperationStatus {
    pub handle: String,
    pub state: OperationState,
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
}

/// Log event emitted by the gateway stream.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LogEvent {
    pub timestamp: DateTime<Utc>,
    pub level: LogLevel,
    pub node: Option<String>,
    pub line: String,
}

/// Log severity levels exposed through the protocol.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

/// Snapshot of system metrics exposed by the coordinator.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub timestamp: DateTime<Utc>,
    pub cpu_percent: f32,
    pub memory_percent: f32,
    pub total_memory_bytes: u64,
    pub used_memory_bytes: u64,
    pub load_average: Option<[f32; 3]>,
}

/// Snapshot of persisted user preferences relevant to UI clients.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UserPreferencesSnapshot {
    pub theme: Option<String>,
    pub ui_mode: Option<UiMode>,
    pub auto_refresh: Option<bool>,
    pub updated_at: DateTime<Utc>,
}

/// Preferred interface mode for a client.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiMode {
    Auto,
    Cli,
    Tui,
    Minimal,
}

/// Error envelope returned by the protocol gateway.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ErrorEnvelope {
    pub error: GatewayError,
}

/// Structured error for client consumption.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GatewayError {
    pub code: ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

/// Canonical error codes aligned with ADR-002.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    ResourceNotFound,
    InvalidArgument,
    AlreadyExists,
    FailedPrecondition,
    InternalError,
    NotImplemented,
    Unavailable,
}
