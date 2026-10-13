use std::fmt;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BootstrapSnapshot {
    pub workspaces: Vec<Workspace>,
    pub active_workspace: Workspace,
    pub projects: Vec<Project>,
    pub plugins: Vec<Plugin>,
    pub targets: Vec<SavedTarget>,
    pub executions: Vec<Execution>,
    pub reports: Vec<Report>,
    pub shell: ShellPreferences,
    pub execution: ExecutionPreferences,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExecutionPreferences {
    pub fuel_limit: u64,
    pub timeout_seconds: u64,
    pub memory_limit_bytes: Option<u64>,
}

impl Default for ExecutionPreferences {
    fn default() -> Self {
        Self {
            fuel_limit: 25_000_000,
            timeout_seconds: 30,
            memory_limit_bytes: Some(64 * 1024 * 1024),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub root_path: String,
    pub is_active: bool,
    pub discovery_state: String,
    pub last_error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_opened_at: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub path: String,
    pub kind: String,
    pub archived: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceEntry {
    pub name: String,
    pub relative_path: String,
    pub is_directory: bool,
    pub is_symlink: bool,
}

/// A text file opened for editing, addressed by its workspace-relative path.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub relative_path: String,
    pub contents: String,
    pub byte_size: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Plugin {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub checksum: String,
    pub signature_status: String,
    pub publisher_fingerprint: Option<String>,
    /// Full live manifest requests, including scope, for permission review.
    pub requested_capabilities: Vec<CapabilityRequest>,
    /// Capability kinds retained as a compact list for filters and badges.
    pub capabilities: Vec<CapabilityKind>,
    pub status: PluginStatus,
    pub source: PluginSource,
    pub file_size: u64,
    pub installed_at: u64,
    pub last_updated: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PluginInspection {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub checksum: String,
    pub signature_status: String,
    pub publisher_fingerprint: Option<String>,
    pub requested_capabilities: Vec<CapabilityRequest>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum PluginSource {
    LocalPath(String),
    Marketplace(String),
    Url(String),
}

impl fmt::Display for PluginSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LocalPath(path) => write!(formatter, "Local ({path})"),
            Self::Marketplace(name) => write!(formatter, "Marketplace ({name})"),
            Self::Url(url) => write!(formatter, "URL ({url})"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginStatus {
    Enabled,
    Disabled,
    Invalid,
    UpdateAvailable,
}

impl PluginStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Enabled => "enabled",
            Self::Disabled => "disabled",
            Self::Invalid => "invalid",
            Self::UpdateAvailable => "update available",
        }
    }
}

impl fmt::Display for PluginStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CapabilityKind {
    NetworkConnect,
    NetworkListen,
    FilesystemRead,
    FilesystemWrite,
    ConfigRead,
    ReportWrite,
    Crypto,
    DnsResolve,
    ProcessSpawn,
    EnvironmentRead,
}

impl CapabilityKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NetworkConnect => "network-connect",
            Self::NetworkListen => "network-listen",
            Self::FilesystemRead => "filesystem-read",
            Self::FilesystemWrite => "filesystem-write",
            Self::ConfigRead => "config-read",
            Self::ReportWrite => "report-write",
            Self::Crypto => "crypto",
            Self::DnsResolve => "dns-resolve",
            Self::ProcessSpawn => "process-spawn",
            Self::EnvironmentRead => "environment-read",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::NetworkConnect => "Connect to remote network services.",
            Self::NetworkListen => "Listen for incoming network connections.",
            Self::FilesystemRead => "Read files available to the plugin.",
            Self::FilesystemWrite => "Create or modify files available to the plugin.",
            Self::ConfigRead => "Read PolyGlid configuration values.",
            Self::ReportWrite => "Write generated report artifacts.",
            Self::Crypto => "Use host cryptographic operations.",
            Self::DnsResolve => "Resolve host names through DNS.",
            Self::ProcessSpawn => "Start a process on the host system.",
            Self::EnvironmentRead => "Read host environment variables.",
        }
    }
}

impl fmt::Display for CapabilityKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CapabilityRequest {
    pub capability: CapabilityKind,
    pub scope: CapabilityScope,
}

impl fmt::Display for CapabilityRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} ({})", self.capability, self.scope)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum CapabilityScope {
    Any,
    Target { target: String },
    PathPrefix { path: String },
    HostPort { host: String, port: u16 },
}

impl fmt::Display for CapabilityScope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Any => formatter.write_str("any scope"),
            Self::Target { target } => write!(formatter, "target {target}"),
            Self::PathPrefix { path } => write!(formatter, "files under {path}"),
            Self::HostPort { host, port } => write!(formatter, "{host}:{port}"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SavedTarget {
    pub name: String,
    pub group: Option<String>,
    pub project_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct JobId(pub Uuid);

impl JobId {
    pub const fn new(id: Uuid) -> Self {
        Self(id)
    }

    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl fmt::Display for JobId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionState {
    Queued,
    Starting,
    Running,
    Completed,
    Failed,
    Cancelled,
    TimedOut,
}

impl ExecutionState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::TimedOut => "timed out",
        }
    }

    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::Cancelled | Self::TimedOut
        )
    }
}

impl fmt::Display for ExecutionState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StartExecutionRequest {
    pub workspace_id: String,
    pub project_id: String,
    pub plugin_id: String,
    pub target: String,
    pub fuel_limit: u64,
    pub timeout: Duration,
    pub memory_limit: Option<u64>,
    pub approval_ids: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDuration {
    Once,
    Session,
    Workspace,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    Allow,
    Deny,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PermissionDecisionRequest {
    pub workspace_id: String,
    pub project_id: String,
    pub plugin_id: String,
    pub target: String,
    pub request: CapabilityRequest,
    pub decision: ApprovalDecision,
    pub duration: ApprovalDuration,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Approval {
    pub id: String,
    pub plugin_id: String,
    pub request: CapabilityRequest,
    pub decision: ApprovalDecision,
    pub duration: ApprovalDuration,
    pub expiration: Option<u64>,
    pub revoked: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Execution {
    pub id: JobId,
    pub plugin_id: String,
    pub project_id: Option<String>,
    pub approval_ids: Vec<String>,
    pub target: String,
    pub state: ExecutionState,
    pub started_at: u64,
    pub duration_ms: u64,
    pub error: Option<String>,
    pub fuel_consumed: Option<u64>,
    pub created_at: u64,
    pub report: Option<ExecutionReport>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExecutionMetrics {
    pub duration_ms: u64,
    pub fuel_consumed: Option<u64>,
    pub memory_used: Option<u64>,
    pub timestamp: u64,
    pub stage: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExecutionReport {
    pub plugin_name: String,
    pub target_tested: String,
    pub issues: Vec<Issue>,
    pub summary: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ExecutionEvent {
    StateChanged {
        job_id: JobId,
        state: ExecutionState,
    },
    Finished {
        job_id: JobId,
        report: ExecutionReport,
        metrics: ExecutionMetrics,
    },
    Failed {
        job_id: JobId,
        error: String,
        metrics: Option<ExecutionMetrics>,
    },
    Log {
        job_id: JobId,
        message: String,
    },
}

impl ExecutionEvent {
    pub const fn job_id(&self) -> JobId {
        match self {
            Self::StateChanged { job_id, .. }
            | Self::Finished { job_id, .. }
            | Self::Failed { job_id, .. }
            | Self::Log { job_id, .. } => *job_id,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub id: String,
    pub job_id: JobId,
    pub plugin_id: String,
    pub project_id: Option<String>,
    pub plugin_version: String,
    pub plugin_checksum: String,
    pub target: String,
    pub summary: String,
    pub issues: Vec<Issue>,
    pub filepath: String,
    pub duration_ms: u64,
    pub fuel_consumed: Option<u64>,
    pub memory_used: Option<u64>,
    pub security_profile: String,
    pub report_format_version: String,
    pub created_at: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportFormat {
    Json,
    Markdown,
    Html,
    Sarif,
}

impl ReportFormat {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Markdown => "markdown",
            Self::Html => "html",
            Self::Sarif => "sarif",
        }
    }
}

impl fmt::Display for ReportFormat {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

impl Severity {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Issue {
    pub title: String,
    pub severity: Severity,
    pub description: String,
    pub recommendation: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShellPreferences {
    pub sidebar_visible: bool,
    pub bottom_panel_visible: bool,
    pub sidebar_width: f64,
    pub bottom_panel_height: f64,
}

impl Default for ShellPreferences {
    fn default() -> Self {
        Self {
            sidebar_visible: true,
            bottom_panel_visible: true,
            sidebar_width: 280.0,
            bottom_panel_height: 210.0,
        }
    }
}
