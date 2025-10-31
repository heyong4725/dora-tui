#[derive(Debug, Clone)]
pub enum StateUpdate {
    DataflowAdded(super::app::DataflowInfo),
    DataflowRemoved(String),
    DataflowStatusChanged {
        name: String,
        new_status: String,
    },
    NodeStatusChanged {
        dataflow: String,
        node: String,
        status: String,
    },
    SystemMetricsUpdated,
    ConfigurationChanged,
    RefreshRequired,
}
