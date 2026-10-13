use std::time::{Duration, Instant};

use polyglid_core::execution;
use tokio::sync::broadcast;

use super::{
    Approval, BootstrapSnapshot, ClientError, ClientResult, Document, Execution, ExecutionEvent,
    ExecutionPreferences, JobId, PermissionDecisionRequest, Plugin, PluginInspection, Project,
    Report, ReportFormat, SavedTarget, ShellPreferences, StartExecutionRequest, Workspace,
    WorkspaceEntry,
};

/// Operations exposed to desktop views and feature controllers.
///
/// Methods are synchronous so filesystem/database work can be moved into
/// `spawn_blocking` by the Dioxus controller without coupling this boundary to
/// a particular async runtime.
pub trait ClientGateway: Clone + Send + Sync + 'static {
    fn bootstrap(&self) -> ClientResult<BootstrapSnapshot>;

    fn list_workspaces(&self) -> ClientResult<Vec<Workspace>>;
    fn register_workspace(&self, name: &str, root_path: &str) -> ClientResult<Workspace>;
    fn activate_workspace(&self, workspace_id: &str) -> ClientResult<()>;
    fn refresh_workspace(&self, workspace_id: &str) -> ClientResult<Vec<Project>>;
    fn list_workspace_entries(
        &self,
        workspace_id: &str,
        relative_directory: &str,
    ) -> ClientResult<Vec<WorkspaceEntry>>;

    fn read_document(&self, workspace_id: &str, relative_path: &str) -> ClientResult<Document>;
    fn write_document(
        &self,
        workspace_id: &str,
        relative_path: &str,
        contents: &str,
    ) -> ClientResult<()>;

    fn list_projects(&self, workspace_id: &str) -> ClientResult<Vec<Project>>;
    fn create_project(&self, workspace_id: &str, name: &str) -> ClientResult<Project>;
    fn rename_project(&self, project_id: &str, name: &str) -> ClientResult<Project>;
    fn remove_project(&self, project_id: &str, delete_files: bool) -> ClientResult<()>;

    fn list_plugins(&self) -> ClientResult<Vec<Plugin>>;
    fn inspect_plugin(&self, path: &str) -> ClientResult<PluginInspection>;
    fn install_plugin(&self, path: &str) -> ClientResult<Plugin>;
    fn set_plugin_enabled(&self, plugin_id: &str, enabled: bool) -> ClientResult<()>;
    fn uninstall_plugin(&self, plugin_id: &str) -> ClientResult<()>;

    fn list_targets(&self) -> ClientResult<Vec<SavedTarget>>;
    fn add_target(
        &self,
        name: &str,
        group: Option<&str>,
        project_id: &str,
    ) -> ClientResult<SavedTarget>;
    fn remove_target(&self, name: &str, project_id: &str) -> ClientResult<()>;

    fn record_permission_decision(
        &self,
        request: PermissionDecisionRequest,
    ) -> ClientResult<Approval>;
    fn revoke_approval(&self, approval_id: &str) -> ClientResult<()>;
    fn list_approvals(&self) -> ClientResult<Vec<Approval>>;

    fn start_execution(&self, request: StartExecutionRequest) -> ClientResult<JobId>;
    fn subscribe_executions(&self) -> ClientResult<ExecutionSubscription>;
    fn wait_for_execution(&self, job_id: JobId, timeout: Duration) -> ClientResult<Execution>;
    fn cancel_execution(&self, job_id: JobId) -> ClientResult<()>;
    fn list_executions(&self) -> ClientResult<Vec<Execution>>;

    fn list_reports(&self) -> ClientResult<Vec<Report>>;
    fn get_report(&self, report_id: &str) -> ClientResult<Report>;
    fn export_report(&self, report_id: &str, format: ReportFormat) -> ClientResult<String>;

    fn save_shell_preferences(&self, preferences: &ShellPreferences) -> ClientResult<()>;
    fn save_execution_preferences(&self, preferences: &ExecutionPreferences) -> ClientResult<()>;
}

/// A local execution event stream suitable for a Dioxus controller running in
/// `spawn_blocking`.
pub struct ExecutionSubscription {
    pub(crate) receiver: broadcast::Receiver<execution::ExecutionEvent>,
}

impl ExecutionSubscription {
    pub fn blocking_recv(&mut self) -> ClientResult<ExecutionEvent> {
        self.receiver
            .blocking_recv()
            .map(super::local::execution_event_from_core)
            .map_err(map_receive_error)
    }

    pub fn try_recv(&mut self) -> ClientResult<Option<ExecutionEvent>> {
        match self.receiver.try_recv() {
            Ok(event) => Ok(Some(super::local::execution_event_from_core(event))),
            Err(broadcast::error::TryRecvError::Empty) => Ok(None),
            Err(broadcast::error::TryRecvError::Closed) => Err(ClientError::EventStreamClosed),
            Err(broadcast::error::TryRecvError::Lagged(count)) => {
                Err(ClientError::EventStreamLagged(count))
            }
        }
    }

    pub fn recv_timeout(&mut self, timeout: Duration) -> ClientResult<Option<ExecutionEvent>> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(event) = self.try_recv()? {
                return Ok(Some(event));
            }
            let now = Instant::now();
            if now >= deadline {
                return Ok(None);
            }
            std::thread::sleep((deadline - now).min(Duration::from_millis(10)));
        }
    }
}

fn map_receive_error(error: broadcast::error::RecvError) -> ClientError {
    match error {
        broadcast::error::RecvError::Closed => ClientError::EventStreamClosed,
        broadcast::error::RecvError::Lagged(count) => ClientError::EventStreamLagged(count),
    }
}
