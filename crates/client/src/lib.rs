//! UI-independent client boundary shared by PolyGlid desktop frontends.
//!
//! This crate gives Dioxus and GPUI one stable dependency without making
//! either UI framework depend on the other.

mod error;
mod gateway;
mod local;
mod models;
mod paths;

pub use error::{ClientError, ClientResult};
pub use gateway::{ClientGateway, ExecutionSubscription};
pub use local::{LocalClient, SetupReport, SetupStatus};
pub use models::{
    Approval, ApprovalDecision, ApprovalDuration, BootstrapSnapshot, CapabilityKind,
    CapabilityRequest, CapabilityScope, Document, Execution, ExecutionEvent, ExecutionMetrics,
    ExecutionPreferences, ExecutionReport, ExecutionState, Issue, JobId, PermissionDecisionRequest,
    Plugin, PluginInspection, PluginSource, PluginStatus, Project, Report, ReportFormat,
    SavedTarget, Severity, ShellPreferences, StartExecutionRequest, Workspace, WorkspaceEntry,
};
pub use paths::{RuntimeInitialization, RuntimePaths};

/// Compatibility namespace used by both desktop clients.
pub mod client {
    pub use super::{
        Approval, ApprovalDecision, ApprovalDuration, BootstrapSnapshot, CapabilityKind,
        CapabilityRequest, CapabilityScope, ClientError, ClientGateway, ClientResult, Document,
        Execution, ExecutionEvent, ExecutionMetrics, ExecutionPreferences, ExecutionReport,
        ExecutionState, ExecutionSubscription, Issue, JobId, LocalClient,
        PermissionDecisionRequest, Plugin, PluginInspection, PluginSource, PluginStatus, Project,
        Report, ReportFormat, SavedTarget, SetupReport, SetupStatus, Severity, ShellPreferences,
        StartExecutionRequest, Workspace, WorkspaceEntry,
    };
}

pub mod controllers;
