use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use polyglid_config::plugin_registry::{
    PluginRegistryEntry, PluginSource as CorePluginSource, PluginStatus as CorePluginStatus,
};
use polyglid_config::AppConfig;
use polyglid_core::execution::{
    ExecutionConfig, ExecutionEvent as CoreExecutionEvent, ExecutionManager, Job as CoreExecution,
    JobMetrics as CoreExecutionMetrics, JobState as CoreExecutionState,
};
use polyglid_core::plugin_manager::PluginManager;
use polyglid_core::services::WorkspaceCatalogService;
use polyglid_core::store::migrations::MigrationReport;
use polyglid_core::store::permission_store::{
    ApprovalBinding as CoreApprovalBinding, ApprovalDuration as CoreApprovalDuration,
    DbPermissionRecord,
};
use polyglid_core::store::{DbProject, DbWorkspace, WorkspaceStore};
use polyglid_core::{PermissionDecision as CorePermissionDecision, Target as CoreTarget};
use polyglid_plugin_api::{
    ApiPluginMetadata, Capability as CoreCapability, CapabilityRequest as CoreCapabilityRequest,
    CapabilityScope as CoreCapabilityScope, Issue as CoreIssue, PluginId, PluginManifest,
    PluginReport as CorePluginReport, Severity as CoreSeverity,
};
use polyglid_runtime::WasmRuntime;

use super::paths::{RuntimeInitialization, RuntimePaths};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetupStatus {
    FirstRun,
    Migrated,
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupReport {
    pub status: SetupStatus,
    pub created_directories: Vec<PathBuf>,
    pub database_previous_version: i32,
    pub database_version: i32,
    pub applied_migrations: Vec<i32>,
}
use super::{
    Approval, ApprovalDecision, ApprovalDuration, BootstrapSnapshot, CapabilityKind,
    CapabilityRequest, CapabilityScope, ClientError, ClientGateway, ClientResult, Document,
    Execution, ExecutionEvent, ExecutionMetrics, ExecutionPreferences, ExecutionReport,
    ExecutionState, ExecutionSubscription, Issue, JobId, PermissionDecisionRequest, Plugin,
    PluginInspection, PluginSource, PluginStatus, Project, Report, ReportFormat, SavedTarget,
    Severity, ShellPreferences, StartExecutionRequest, Workspace, WorkspaceEntry,
};

#[derive(Clone)]
pub struct LocalClient {
    host: Arc<ApplicationHost>,
}

/// One long-lived local application service graph shared by every desktop
/// feature. Views never construct databases, registries, runtimes, or managers.
struct ApplicationHost {
    catalog: WorkspaceCatalogService,
    plugins: Arc<PluginManager<WasmRuntime>>,
    executions: Arc<ExecutionManager<WasmRuntime>>,
    store: WorkspaceStore,
    default_workspace_root: PathBuf,
    data_directory: PathBuf,
    session_id: String,
}

impl LocalClient {
    pub fn open_default() -> ClientResult<Self> {
        Self::open_default_with_setup().map(|(client, _)| client)
    }

    pub fn open_default_with_setup() -> ClientResult<(Self, SetupReport)> {
        let paths = RuntimePaths::discover()
            .map_err(|error| ClientError::Unavailable(error.to_string()))?;
        Self::open_paths_with_setup(paths)
    }

    /// Open a local desktop client with explicit paths. This is useful for
    /// portable installations, tests, and future profile selection.
    pub fn open(
        data_directory: impl AsRef<Path>,
        default_workspace_root: impl AsRef<Path>,
    ) -> ClientResult<Self> {
        Self::open_with_setup(data_directory, default_workspace_root).map(|(client, _)| client)
    }

    pub fn open_with_setup(
        data_directory: impl AsRef<Path>,
        default_workspace_root: impl AsRef<Path>,
    ) -> ClientResult<(Self, SetupReport)> {
        Self::open_paths_with_setup(RuntimePaths::from_roots(
            data_directory.as_ref(),
            default_workspace_root.as_ref(),
        ))
    }

    fn open_paths_with_setup(paths: RuntimePaths) -> ClientResult<(Self, SetupReport)> {
        let initialization = paths.initialize().map_err(|error| {
            ClientError::operation("initialize desktop runtime directories", error.to_string())
        })?;

        let config = AppConfig {
            plugin_dir: paths.plugins.clone(),
            reports_dir: paths.reports.clone(),
            ..AppConfig::development()
        };
        let (store, migration_report) = WorkspaceStore::new_with_migrations(&paths.database())
            .map_err(|error| ClientError::operation("open desktop database", error))?;
        let catalog = WorkspaceCatalogService::from_store(store.clone());
        enroll_official_publisher(&store, option_env!("POLYGLID_OFFICIAL_PUBLISHER_KEY"))?;
        let plugins = Arc::new(
            PluginManager::new(Arc::new(WasmRuntime::new()), &config, store.clone())
                .map_err(|error| ClientError::operation("open plugin manager", error))?,
        );
        plugins
            .sync_directory()
            .map_err(|error| ClientError::operation("synchronize plugin directory", error))?;
        let executions = Arc::new(ExecutionManager::new_with_config(
            WasmRuntime::new(),
            Some(store.clone()),
            config,
        ));

        let setup_report = setup_report(initialization, migration_report);
        Ok((
            Self {
                host: Arc::new(ApplicationHost {
                    catalog,
                    plugins,
                    executions,
                    store,
                    default_workspace_root: paths.workspace,
                    data_directory: paths.data,
                    session_id: uuid::Uuid::now_v7().to_string(),
                }),
            },
            setup_report,
        ))
    }

    pub fn data_directory(&self) -> &Path {
        &self.host.data_directory
    }

    fn load_active_catalog(&self) -> ClientResult<(Vec<Workspace>, Workspace, Vec<Project>)> {
        let mut workspaces = self
            .host
            .catalog
            .list_workspaces()
            .map_err(|error| ClientError::operation("list workspaces", error))?;
        if workspaces.is_empty() {
            self.host
                .catalog
                .register_workspace("PolyGlid Projects", &self.host.default_workspace_root)
                .map_err(|error| ClientError::operation("create default workspace", error))?;
            workspaces = self
                .host
                .catalog
                .list_workspaces()
                .map_err(|error| ClientError::operation("reload workspaces", error))?;
        }

        let active_id = if let Some(workspace) = workspaces.iter().find(|item| item.is_active) {
            workspace.id.clone()
        } else {
            let first = workspaces
                .first()
                .ok_or_else(|| ClientError::Unavailable("no workspace is available".to_string()))?;
            self.host
                .catalog
                .activate(&first.id)
                .map_err(|error| ClientError::operation("activate default workspace", error))?;
            first.id.clone()
        };

        let projects = self
            .host
            .catalog
            .discover(&active_id)
            .map_err(|error| ClientError::operation("discover projects", error))?;
        let workspaces = self
            .host
            .catalog
            .list_workspaces()
            .map_err(|error| ClientError::operation("reload active workspace", error))?;
        let active = workspaces
            .iter()
            .find(|item| item.is_active)
            .cloned()
            .ok_or_else(|| {
                ClientError::Unavailable("active workspace was not found".to_string())
            })?;

        Ok((
            workspaces.into_iter().map(workspace_from_db).collect(),
            workspace_from_db(active),
            projects.into_iter().map(project_from_db).collect(),
        ))
    }

    fn load_shell_preferences(&self) -> ClientResult<ShellPreferences> {
        let settings = self.host.store.settings();
        let defaults = ShellPreferences::default();
        Ok(ShellPreferences {
            sidebar_visible: setting_bool(
                settings.get("ui.sidebar_visible"),
                defaults.sidebar_visible,
            )?,
            bottom_panel_visible: setting_bool(
                settings.get("ui.bottom_panel_visible"),
                defaults.bottom_panel_visible,
            )?,
            sidebar_width: setting_number(
                settings.get("ui.sidebar_width"),
                defaults.sidebar_width,
                180.0,
                480.0,
            )?,
            bottom_panel_height: setting_number(
                settings.get("ui.bottom_panel_height"),
                defaults.bottom_panel_height,
                120.0,
                520.0,
            )?,
        })
    }

    fn load_execution_preferences(&self) -> ClientResult<ExecutionPreferences> {
        let settings = self.host.store.settings();
        let defaults = ExecutionPreferences::default();
        Ok(ExecutionPreferences {
            fuel_limit: setting_u64(
                settings.get("execution.fuel_limit"),
                defaults.fuel_limit,
                1,
                1_000_000_000,
            )?,
            timeout_seconds: setting_u64(
                settings.get("execution.timeout_seconds"),
                defaults.timeout_seconds,
                1,
                300,
            )?,
            memory_limit_bytes: Some(setting_u64(
                settings.get("execution.memory_limit_bytes"),
                defaults.memory_limit_bytes.unwrap_or(64 * 1024 * 1024),
                1024 * 1024,
                1024 * 1024 * 1024,
            )?),
        })
    }

    fn execution(&self, job_id: JobId) -> ClientResult<Execution> {
        self.list_executions()?
            .into_iter()
            .find(|execution| execution.id == job_id)
            .ok_or_else(|| ClientError::NotFound {
                resource: "execution",
                id: job_id.to_string(),
            })
    }
}

/// Upper bound for text files read from or written by the editor.
const MAX_DOCUMENT_BYTES: u64 = 8 * 1024 * 1024;

impl LocalClient {
    /// Resolve a workspace-relative path, rejecting anything that would escape
    /// the workspace root. Returns the canonical root and the resolved path.
    fn resolve_workspace_path(
        &self,
        workspace_id: &str,
        relative_path: &str,
        field: &'static str,
    ) -> ClientResult<(PathBuf, PathBuf)> {
        use std::path::Component;

        let workspace = self
            .list_workspaces()?
            .into_iter()
            .find(|workspace| workspace.id == workspace_id)
            .ok_or_else(|| ClientError::NotFound {
                resource: "workspace",
                id: workspace_id.to_owned(),
            })?;

        let relative = Path::new(relative_path);
        if relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
        {
            return Err(ClientError::InvalidInput {
                field,
                message: "the path must stay inside the active workspace".to_owned(),
            });
        }

        let root = Path::new(&workspace.root_path)
            .canonicalize()
            .map_err(|error| ClientError::operation("resolve workspace root", error.to_string()))?;
        let resolved = root
            .join(relative)
            .canonicalize()
            .map_err(|error| ClientError::operation("resolve workspace path", error.to_string()))?;
        if !resolved.starts_with(&root) {
            return Err(ClientError::InvalidInput {
                field,
                message: "the path must resolve inside the active workspace".to_owned(),
            });
        }

        Ok((root, resolved))
    }
}

impl ClientGateway for LocalClient {
    fn bootstrap(&self) -> ClientResult<BootstrapSnapshot> {
        let (workspaces, active_workspace, projects) = self.load_active_catalog()?;
        Ok(BootstrapSnapshot {
            workspaces,
            active_workspace,
            projects,
            plugins: self.list_plugins()?,
            targets: self.list_targets()?,
            executions: self.list_executions()?,
            reports: self.list_reports()?,
            shell: self.load_shell_preferences()?,
            execution: self.load_execution_preferences()?,
        })
    }

    fn list_workspaces(&self) -> ClientResult<Vec<Workspace>> {
        self.host
            .catalog
            .list_workspaces()
            .map(|items| items.into_iter().map(workspace_from_db).collect())
            .map_err(|error| ClientError::operation("list workspaces", error))
    }

    fn register_workspace(&self, name: &str, root_path: &str) -> ClientResult<Workspace> {
        self.host
            .catalog
            .register_workspace(name, Path::new(root_path))
            .map(workspace_from_db)
            .map_err(|error| ClientError::operation("register workspace", error))
    }

    fn activate_workspace(&self, workspace_id: &str) -> ClientResult<()> {
        self.host
            .catalog
            .activate(workspace_id)
            .map_err(|error| ClientError::operation("activate workspace", error))
    }

    fn refresh_workspace(&self, workspace_id: &str) -> ClientResult<Vec<Project>> {
        self.host
            .catalog
            .discover(workspace_id)
            .map(|items| items.into_iter().map(project_from_db).collect())
            .map_err(|error| ClientError::operation("refresh workspace", error))
    }

    fn list_workspace_entries(
        &self,
        workspace_id: &str,
        relative_directory: &str,
    ) -> ClientResult<Vec<WorkspaceEntry>> {
        let (root, directory) =
            self.resolve_workspace_path(workspace_id, relative_directory, "relative directory")?;
        if !directory.is_dir() {
            return Err(ClientError::InvalidInput {
                field: "relative directory",
                message: "the path must resolve to a directory inside the active workspace"
                    .to_owned(),
            });
        }

        let mut entries = fs::read_dir(&directory)
            .map_err(|error| ClientError::operation("list workspace directory", error.to_string()))?
            .map(|entry| {
                let entry = entry.map_err(|error| {
                    ClientError::operation("read workspace entry", error.to_string())
                })?;
                let file_type = entry.file_type().map_err(|error| {
                    ClientError::operation("inspect workspace entry", error.to_string())
                })?;
                let name = entry.file_name().to_string_lossy().into_owned();
                let relative_path = entry
                    .path()
                    .strip_prefix(&root)
                    .map_err(|error| {
                        ClientError::operation("resolve workspace entry", error.to_string())
                    })?
                    .to_string_lossy()
                    .into_owned();
                Ok(WorkspaceEntry {
                    name,
                    relative_path,
                    is_directory: file_type.is_dir(),
                    is_symlink: file_type.is_symlink(),
                })
            })
            .collect::<ClientResult<Vec<_>>>()?;
        entries.sort_by(|left, right| {
            right
                .is_directory
                .cmp(&left.is_directory)
                .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
        });
        Ok(entries)
    }

    fn read_document(&self, workspace_id: &str, relative_path: &str) -> ClientResult<Document> {
        let (root, path) =
            self.resolve_workspace_path(workspace_id, relative_path, "document path")?;
        if !path.is_file() {
            return Err(ClientError::InvalidInput {
                field: "document path",
                message: "the path must resolve to a file inside the active workspace".to_owned(),
            });
        }
        let metadata = fs::metadata(&path)
            .map_err(|error| ClientError::operation("inspect document", error.to_string()))?;
        if metadata.len() > MAX_DOCUMENT_BYTES {
            return Err(ClientError::InvalidInput {
                field: "document path",
                message: format!(
                    "files larger than {MAX_DOCUMENT_BYTES} bytes are not opened in the editor"
                ),
            });
        }
        let contents = fs::read_to_string(&path)
            .map_err(|error| ClientError::operation("read document", error.to_string()))?;
        Ok(Document {
            relative_path: workspace_relative_path(&root, &path, relative_path),
            byte_size: metadata.len(),
            contents,
        })
    }

    fn write_document(
        &self,
        workspace_id: &str,
        relative_path: &str,
        contents: &str,
    ) -> ClientResult<()> {
        let (_, path) =
            self.resolve_workspace_path(workspace_id, relative_path, "document path")?;
        if !path.is_file() {
            return Err(ClientError::InvalidInput {
                field: "document path",
                message: "the path must resolve to an existing file inside the active workspace"
                    .to_owned(),
            });
        }
        if contents.len() as u64 > MAX_DOCUMENT_BYTES {
            return Err(ClientError::InvalidInput {
                field: "document contents",
                message: format!(
                    "contents larger than {MAX_DOCUMENT_BYTES} bytes are not saved from the editor"
                ),
            });
        }
        fs::write(&path, contents)
            .map_err(|error| ClientError::operation("write document", error.to_string()))
    }

    fn list_projects(&self, workspace_id: &str) -> ClientResult<Vec<Project>> {
        self.host
            .catalog
            .list_projects(workspace_id)
            .map(|items| items.into_iter().map(project_from_db).collect())
            .map_err(|error| ClientError::operation("list projects", error))
    }

    fn create_project(&self, workspace_id: &str, name: &str) -> ClientResult<Project> {
        self.host
            .catalog
            .create_project(workspace_id, name)
            .map(project_from_db)
            .map_err(|error| ClientError::operation("create project", error))
    }

    fn rename_project(&self, project_id: &str, name: &str) -> ClientResult<Project> {
        self.host
            .catalog
            .rename_project(project_id, name)
            .map(project_from_db)
            .map_err(|error| ClientError::operation("rename project", error))
    }

    fn remove_project(&self, project_id: &str, delete_files: bool) -> ClientResult<()> {
        self.host
            .catalog
            .remove_project(project_id, delete_files)
            .map_err(|error| ClientError::operation("remove project", error))
    }

    fn list_plugins(&self) -> ClientResult<Vec<Plugin>> {
        Ok(self
            .host
            .plugins
            .get_plugins()
            .into_iter()
            .map(|entry| {
                let signature = self
                    .host
                    .store
                    .signatures()
                    .get(entry.id.as_str())
                    .ok()
                    .flatten();
                let requests = self
                    .host
                    .plugins
                    .validate_plugin(&entry.path)
                    .ok()
                    .map_or_else(Vec::new, |(manifest, _)| {
                        manifest
                            .requested_capabilities
                            .into_iter()
                            .map(capability_request_from_core)
                            .collect()
                    });
                plugin_from_registry(entry, requests, signature)
            })
            .collect())
    }

    fn inspect_plugin(&self, path: &str) -> ClientResult<PluginInspection> {
        let (manifest, metadata, checksum, signature_status, publisher_fingerprint) = self
            .host
            .plugins
            .inspect_plugin_package(Path::new(path))
            .map_err(|error| ClientError::operation("inspect plugin", error))?;
        Ok(plugin_inspection(
            manifest,
            metadata,
            checksum,
            signature_status.to_string(),
            publisher_fingerprint,
        ))
    }

    fn install_plugin(&self, path: &str) -> ClientResult<Plugin> {
        let entry = self
            .host
            .plugins
            .install_plugin(
                Path::new(path),
                CorePluginSource::LocalPath(PathBuf::from(path)),
            )
            .map_err(|error| ClientError::operation("install plugin", error))?;
        let requests = self
            .host
            .plugins
            .validate_plugin(&entry.path)
            .map_err(|error| ClientError::operation("reload installed plugin", error))?
            .0
            .requested_capabilities
            .into_iter()
            .map(capability_request_from_core)
            .collect();
        let signature = self
            .host
            .store
            .signatures()
            .get(entry.id.as_str())
            .map_err(|error| ClientError::operation("load plugin signature", error))?;
        Ok(plugin_from_registry(entry, requests, signature))
    }

    fn set_plugin_enabled(&self, plugin_id: &str, enabled: bool) -> ClientResult<()> {
        let id = parse_plugin_id(plugin_id)?;
        self.host
            .plugins
            .toggle_plugin_enabled(&id, enabled)
            .map_err(|error| ClientError::operation("change plugin status", error))
    }

    fn uninstall_plugin(&self, plugin_id: &str) -> ClientResult<()> {
        let id = parse_plugin_id(plugin_id)?;
        self.host
            .plugins
            .uninstall_plugin(&id)
            .map_err(|error| ClientError::operation("uninstall plugin", error))
    }

    fn list_targets(&self) -> ClientResult<Vec<SavedTarget>> {
        self.host
            .store
            .targets()
            .list()
            .map(|items| {
                items
                    .into_iter()
                    .map(|(name, group, project_id)| SavedTarget {
                        name,
                        group,
                        project_id,
                    })
                    .collect()
            })
            .map_err(|error| ClientError::operation("list targets", error))
    }

    fn add_target(
        &self,
        name: &str,
        group: Option<&str>,
        project_id: &str,
    ) -> ClientResult<SavedTarget> {
        let target = CoreTarget::parse(name).map_err(|error| ClientError::InvalidInput {
            field: "target",
            message: error.to_string(),
        })?;
        let name = target.as_str().to_string();
        let group = group
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        self.host
            .store
            .targets()
            .add(&name, group.as_deref(), Some(project_id))
            .map_err(|error| ClientError::operation("save target", error))?;
        Ok(SavedTarget {
            name,
            group,
            project_id: Some(project_id.to_string()),
        })
    }

    fn remove_target(&self, name: &str, project_id: &str) -> ClientResult<()> {
        self.host
            .store
            .targets()
            .remove(name, Some(project_id))
            .map_err(|error| ClientError::operation("remove target", error))
    }

    fn record_permission_decision(
        &self,
        request: PermissionDecisionRequest,
    ) -> ClientResult<Approval> {
        let plugin_id = parse_plugin_id(&request.plugin_id)?;
        let entry =
            self.host
                .plugins
                .get_plugin(&plugin_id)
                .ok_or_else(|| ClientError::NotFound {
                    resource: "plugin",
                    id: request.plugin_id.clone(),
                })?;
        let projects = self
            .host
            .catalog
            .list_projects(&request.workspace_id)
            .map_err(|error| ClientError::operation("validate approval project", error))?;
        if !projects
            .iter()
            .any(|project| project.id == request.project_id)
        {
            return Err(ClientError::InvalidInput {
                field: "project",
                message: "the selected project does not belong to the active workspace".to_string(),
            });
        }
        let (manifest, _) = self
            .host
            .plugins
            .validate_plugin(&entry.path)
            .map_err(|error| ClientError::operation("verify approval plugin", error))?;
        let core_request = capability_request_to_core(request.request.clone());
        if !manifest.requested_capabilities.contains(&core_request) {
            return Err(ClientError::UnexpectedCapabilityApproval {
                plugin_id: request.plugin_id,
                capabilities: vec![request.request],
            });
        }
        let binding = CoreApprovalBinding {
            workspace_id: request.workspace_id,
            project_id: Some(request.project_id),
            plugin_version: entry.version.to_string(),
            plugin_checksum: entry.checksum,
            target: CoreTarget::parse(&request.target)
                .map_err(|error| ClientError::InvalidInput {
                    field: "target",
                    message: error.to_string(),
                })?
                .as_str()
                .to_string(),
        };
        let duration = match request.duration {
            ApprovalDuration::Once => CoreApprovalDuration::Once,
            ApprovalDuration::Session => CoreApprovalDuration::Session,
            ApprovalDuration::Workspace => CoreApprovalDuration::Workspace,
        };
        let decision = match request.decision {
            ApprovalDecision::Allow => CorePermissionDecision::Allow,
            ApprovalDecision::Deny => CorePermissionDecision::Deny {
                reason: "operator denied the capability request".to_string(),
            },
        };
        let expiration = match request.duration {
            ApprovalDuration::Once => Some(now_secs() + 5 * 60),
            ApprovalDuration::Session | ApprovalDuration::Workspace => None,
        };
        let session_id = (request.duration != ApprovalDuration::Workspace)
            .then_some(self.host.session_id.as_str());
        let record = self
            .host
            .store
            .permissions()
            .record_decision(
                &plugin_id,
                &core_request,
                &binding,
                decision,
                duration,
                session_id,
                expiration,
            )
            .map_err(|error| ClientError::operation("record permission decision", error))?;
        self.host
            .store
            .audit_logger()
            .log(
                match request.decision {
                    ApprovalDecision::Allow => "CapabilityApprovalGranted",
                    ApprovalDecision::Deny => "CapabilityApprovalDenied",
                },
                Some(plugin_id.as_str()),
                serde_json::json!({
                    "approval_id": record.id,
                    "workspace_id": binding.workspace_id,
                    "project_id": binding.project_id,
                    "target": binding.target,
                    "request": record.request.to_string(),
                    "duration": record.duration.as_str(),
                }),
            )
            .map_err(|error| ClientError::operation("audit permission decision", error))?;
        Ok(approval_from_db(record))
    }

    fn revoke_approval(&self, approval_id: &str) -> ClientResult<()> {
        if self
            .host
            .store
            .permissions()
            .revoke(approval_id)
            .map_err(|error| ClientError::operation("revoke approval", error))?
        {
            Ok(())
        } else {
            Err(ClientError::NotFound {
                resource: "approval",
                id: approval_id.to_string(),
            })
        }
    }

    fn list_approvals(&self) -> ClientResult<Vec<Approval>> {
        self.host
            .store
            .permissions()
            .list()
            .map(|records| records.into_iter().map(approval_from_db).collect())
            .map_err(|error| ClientError::operation("list approvals", error))
    }

    fn start_execution(&self, request: StartExecutionRequest) -> ClientResult<JobId> {
        if request.fuel_limit == 0 {
            return Err(ClientError::InvalidInput {
                field: "fuel limit",
                message: "must be greater than zero".to_string(),
            });
        }
        if request.timeout.is_zero() {
            return Err(ClientError::InvalidInput {
                field: "execution timeout",
                message: "must be greater than zero".to_string(),
            });
        }
        let plugin_id = parse_plugin_id(&request.plugin_id)?;
        let target =
            CoreTarget::parse(&request.target).map_err(|error| ClientError::InvalidInput {
                field: "target",
                message: error.to_string(),
            })?;
        let entry =
            self.host
                .plugins
                .get_plugin(&plugin_id)
                .ok_or_else(|| ClientError::NotFound {
                    resource: "plugin",
                    id: request.plugin_id.clone(),
                })?;
        if entry.status != CorePluginStatus::Enabled {
            return Err(ClientError::Conflict(format!(
                "plugin '{}' is {}",
                request.plugin_id, entry.status
            )));
        }

        // Re-inspect the installed component so approval is checked against the
        // executable being launched, not only against cached registry metadata.
        let (manifest, _) = self
            .host
            .plugins
            .validate_plugin(&entry.path)
            .map_err(|error| ClientError::operation("verify installed plugin", error))?;
        let projects = self
            .host
            .catalog
            .list_projects(&request.workspace_id)
            .map_err(|error| ClientError::operation("validate execution project", error))?;
        if !projects
            .iter()
            .any(|project| project.id == request.project_id)
        {
            return Err(ClientError::InvalidInput {
                field: "project",
                message: "the selected project does not belong to the active workspace".to_string(),
            });
        }
        let binding = CoreApprovalBinding {
            workspace_id: request.workspace_id.clone(),
            project_id: Some(request.project_id.clone()),
            plugin_version: entry.version.to_string(),
            plugin_checksum: entry.checksum.clone(),
            target: target.as_str().to_string(),
        };
        let approved_capabilities = self
            .host
            .store
            .permissions()
            .validate_and_consume(
                &request.approval_ids,
                &plugin_id,
                &binding,
                &manifest.requested_capabilities,
                &self.host.session_id,
            )
            .map_err(|error| {
                ClientError::Conflict(format!("permission validation failed: {error}"))
            })?;
        let job_id = self.host.executions.submit_job(
            entry.path.to_string_lossy().into_owned(),
            target.as_str().to_string(),
            ExecutionConfig {
                fuel_limit: request.fuel_limit,
                timeout: request.timeout,
                memory_limit: request.memory_limit,
                allowed_capabilities: approved_capabilities,
                project_id: Some(request.project_id),
                approval_ids: request.approval_ids,
                plugin_version: entry.version.to_string(),
                plugin_checksum: entry.checksum,
            },
        );
        Ok(JobId::new(job_id))
    }

    fn subscribe_executions(&self) -> ClientResult<ExecutionSubscription> {
        Ok(ExecutionSubscription {
            receiver: self.host.executions.subscribe(),
        })
    }

    fn wait_for_execution(&self, job_id: JobId, timeout: Duration) -> ClientResult<Execution> {
        let mut subscription = self.subscribe_executions()?;
        let current = self.execution(job_id)?;
        if current.state.is_terminal() {
            return Ok(current);
        }

        let deadline = Instant::now() + timeout;
        loop {
            let now = Instant::now();
            if now >= deadline {
                return Err(ClientError::WaitTimedOut { job_id });
            }
            let event = subscription.recv_timeout(deadline - now)?;
            if let Some(event) = event {
                if event.job_id() == job_id {
                    let execution = self.execution(job_id)?;
                    if execution.state.is_terminal() {
                        return Ok(execution);
                    }
                }
            } else {
                return Err(ClientError::WaitTimedOut { job_id });
            }
        }
    }

    fn cancel_execution(&self, job_id: JobId) -> ClientResult<()> {
        self.host
            .executions
            .cancel_job(job_id.as_uuid())
            .map_err(|error| ClientError::operation("cancel execution", error))
    }

    fn list_executions(&self) -> ClientResult<Vec<Execution>> {
        let records = self
            .host
            .store
            .executions()
            .list()
            .map_err(|error| ClientError::operation("list execution history", error))?;
        let by_id: HashMap<_, _> = records
            .into_iter()
            .map(|record| (record.job_id, record))
            .collect();
        let mut executions: Vec<_> = self
            .host
            .executions
            .get_jobs()
            .into_iter()
            .map(|job| {
                let record = by_id.get(&job.id);
                execution_from_core(job, record)
            })
            .collect();
        executions.sort_by_key(|item| std::cmp::Reverse((item.started_at, item.created_at)));
        Ok(executions)
    }

    fn list_reports(&self) -> ClientResult<Vec<Report>> {
        self.host
            .store
            .reports()
            .list()
            .map(|items| items.into_iter().map(report_from_db).collect())
            .map_err(|error| ClientError::operation("list reports", error))
    }

    fn get_report(&self, report_id: &str) -> ClientResult<Report> {
        self.host
            .store
            .reports()
            .get(report_id)
            .map_err(|error| ClientError::operation("load report", error))?
            .map(report_from_db)
            .ok_or_else(|| ClientError::NotFound {
                resource: "report",
                id: report_id.to_string(),
            })
    }

    fn export_report(&self, report_id: &str, format: ReportFormat) -> ClientResult<String> {
        let report = self.get_report(report_id)?;
        let plugin = self
            .host
            .plugins
            .get_plugins()
            .into_iter()
            .find(|item| item.id.as_str() == report.plugin_id);
        let payload = polyglid_core::execution::reports::ExportedReport {
            metadata: polyglid_core::execution::reports::ReportMetadata {
                polyglid_version: env!("CARGO_PKG_VERSION").to_string(),
                plugin_id: report.plugin_id.clone(),
                plugin_version: report.plugin_version.clone(),
                target: report.target.clone(),
                timestamp: report.created_at,
                security_profile: report.security_profile.clone(),
                execution_duration_ms: report.duration_ms,
                report_format_version: report.report_format_version.clone(),
            },
            report: CorePluginReport {
                plugin_name: plugin.map_or_else(|| report.plugin_id.clone(), |item| item.name),
                target_tested: report.target.clone(),
                issues: report.issues.into_iter().map(issue_to_core).collect(),
                summary: report.summary,
            },
        };
        let exported = match format {
            ReportFormat::Json => polyglid_core::execution::reports::json::export(&payload),
            ReportFormat::Markdown => polyglid_core::execution::reports::markdown::export(&payload),
            ReportFormat::Html => polyglid_core::execution::reports::html::export(&payload),
            ReportFormat::Sarif => polyglid_core::execution::reports::sarif::export(&payload),
        };
        exported.map_err(|error| ClientError::operation("export report", error))
    }

    fn save_shell_preferences(&self, preferences: &ShellPreferences) -> ClientResult<()> {
        if !preferences.sidebar_width.is_finite() || !preferences.bottom_panel_height.is_finite() {
            return Err(ClientError::InvalidInput {
                field: "shell dimensions",
                message: "dimensions must be finite numbers".to_string(),
            });
        }
        let settings = self.host.store.settings();
        let values = [
            (
                "ui.sidebar_visible",
                preferences.sidebar_visible.to_string(),
            ),
            (
                "ui.bottom_panel_visible",
                preferences.bottom_panel_visible.to_string(),
            ),
            (
                "ui.sidebar_width",
                preferences
                    .sidebar_width
                    .clamp(180.0, 480.0)
                    .round()
                    .to_string(),
            ),
            (
                "ui.bottom_panel_height",
                preferences
                    .bottom_panel_height
                    .clamp(120.0, 520.0)
                    .round()
                    .to_string(),
            ),
        ];
        for (key, value) in values {
            settings
                .set(key, &value, "Workspace")
                .map_err(|error| ClientError::operation("save shell preferences", error))?;
        }
        Ok(())
    }

    fn save_execution_preferences(&self, preferences: &ExecutionPreferences) -> ClientResult<()> {
        if preferences.fuel_limit == 0
            || !(1..=300).contains(&preferences.timeout_seconds)
            || preferences.memory_limit_bytes == Some(0)
        {
            return Err(ClientError::InvalidInput {
                field: "execution preferences",
                message: "fuel and memory must be positive and timeout must be 1–300 seconds"
                    .to_string(),
            });
        }
        let settings = self.host.store.settings();
        for (key, value) in [
            ("execution.fuel_limit", preferences.fuel_limit.to_string()),
            (
                "execution.timeout_seconds",
                preferences.timeout_seconds.to_string(),
            ),
            (
                "execution.memory_limit_bytes",
                preferences
                    .memory_limit_bytes
                    .unwrap_or(64 * 1024 * 1024)
                    .to_string(),
            ),
        ] {
            settings
                .set(key, &value, "Workspace")
                .map_err(|error| ClientError::operation("save execution preferences", error))?;
        }
        Ok(())
    }
}

pub(crate) fn execution_event_from_core(event: CoreExecutionEvent) -> ExecutionEvent {
    match event {
        CoreExecutionEvent::JobStateChanged { job_id, state } => ExecutionEvent::StateChanged {
            job_id: JobId::new(job_id),
            state: execution_state_from_core(state),
        },
        CoreExecutionEvent::JobFinished {
            job_id,
            report,
            metrics,
        } => ExecutionEvent::Finished {
            job_id: JobId::new(job_id),
            report: execution_report_from_core(report),
            metrics: execution_metrics_from_core(metrics),
        },
        CoreExecutionEvent::JobFailed {
            job_id,
            error,
            metrics,
        } => ExecutionEvent::Failed {
            job_id: JobId::new(job_id),
            error,
            metrics: metrics.map(execution_metrics_from_core),
        },
        CoreExecutionEvent::JobLog { job_id, message } => ExecutionEvent::Log {
            job_id: JobId::new(job_id),
            message,
        },
    }
}

fn workspace_from_db(value: DbWorkspace) -> Workspace {
    Workspace {
        id: value.id,
        name: value.name,
        root_path: value.root_path,
        is_active: value.is_active,
        discovery_state: value.discovery_state,
        last_error: value.last_error,
        created_at: value.created_at,
        updated_at: value.updated_at,
        last_opened_at: value.last_opened_at,
    }
}

fn project_from_db(value: DbProject) -> Project {
    Project {
        id: value.id,
        workspace_id: value.workspace_id,
        name: value.name,
        path: value.path,
        kind: value.kind,
        archived: value.archived,
        created_at: value.created_at,
        updated_at: value.updated_at,
    }
}

fn plugin_from_registry(
    value: PluginRegistryEntry,
    requested_capabilities: Vec<CapabilityRequest>,
    signature: Option<polyglid_core::store::signature_store::PluginSignatureRecord>,
) -> Plugin {
    let signature_status = signature
        .as_ref()
        .map_or_else(|| "Missing".to_string(), |record| record.status.clone());
    let publisher_fingerprint = signature.map(|record| record.fingerprint);
    Plugin {
        id: value.id.as_str().to_string(),
        name: value.name,
        version: value.version.to_string(),
        author: value.author,
        description: value.description,
        checksum: value.checksum,
        signature_status,
        publisher_fingerprint,
        requested_capabilities,
        capabilities: value
            .capabilities
            .into_iter()
            .map(capability_from_core)
            .collect(),
        status: match value.status {
            CorePluginStatus::Enabled => PluginStatus::Enabled,
            CorePluginStatus::Disabled => PluginStatus::Disabled,
            CorePluginStatus::Invalid => PluginStatus::Invalid,
            CorePluginStatus::UpdateAvailable => PluginStatus::UpdateAvailable,
        },
        source: match value.source {
            CorePluginSource::LocalPath(path) => {
                PluginSource::LocalPath(path.to_string_lossy().into_owned())
            }
            CorePluginSource::Marketplace(name) => PluginSource::Marketplace(name),
            CorePluginSource::Url(url) => PluginSource::Url(url),
        },
        file_size: value.file_size,
        installed_at: value.installed_at,
        last_updated: value.last_updated,
    }
}

fn plugin_inspection(
    manifest: PluginManifest,
    metadata: ApiPluginMetadata,
    checksum: String,
    signature_status: String,
    publisher_fingerprint: Option<String>,
) -> PluginInspection {
    PluginInspection {
        id: manifest.id.as_str().to_string(),
        name: manifest.name,
        display_name: metadata.display_name,
        version: metadata.version,
        description: metadata.description,
        author: metadata.author,
        checksum,
        signature_status,
        publisher_fingerprint,
        requested_capabilities: manifest
            .requested_capabilities
            .into_iter()
            .map(capability_request_from_core)
            .collect(),
    }
}

fn capability_request_from_core(value: CoreCapabilityRequest) -> CapabilityRequest {
    CapabilityRequest {
        capability: capability_from_core(value.capability),
        scope: match value.scope {
            CoreCapabilityScope::Any => CapabilityScope::Any,
            CoreCapabilityScope::Target(target) => CapabilityScope::Target { target },
            CoreCapabilityScope::PathPrefix(path) => CapabilityScope::PathPrefix { path },
            CoreCapabilityScope::HostPort { host, port } => {
                CapabilityScope::HostPort { host, port }
            }
        },
    }
}

fn capability_request_to_core(value: CapabilityRequest) -> CoreCapabilityRequest {
    CoreCapabilityRequest {
        capability: capability_to_core(value.capability),
        scope: match value.scope {
            CapabilityScope::Any => CoreCapabilityScope::Any,
            CapabilityScope::Target { target } => CoreCapabilityScope::Target(target),
            CapabilityScope::PathPrefix { path } => CoreCapabilityScope::PathPrefix(path),
            CapabilityScope::HostPort { host, port } => {
                CoreCapabilityScope::HostPort { host, port }
            }
        },
    }
}

fn capability_from_core(value: CoreCapability) -> CapabilityKind {
    match value {
        CoreCapability::NetworkConnect => CapabilityKind::NetworkConnect,
        CoreCapability::NetworkListen => CapabilityKind::NetworkListen,
        CoreCapability::FilesystemRead => CapabilityKind::FilesystemRead,
        CoreCapability::FilesystemWrite => CapabilityKind::FilesystemWrite,
        CoreCapability::ConfigRead => CapabilityKind::ConfigRead,
        CoreCapability::ReportWrite => CapabilityKind::ReportWrite,
        CoreCapability::Crypto => CapabilityKind::Crypto,
        CoreCapability::DnsResolve => CapabilityKind::DnsResolve,
        CoreCapability::ProcessSpawn => CapabilityKind::ProcessSpawn,
        CoreCapability::EnvironmentRead => CapabilityKind::EnvironmentRead,
    }
}

fn capability_to_core(value: CapabilityKind) -> CoreCapability {
    match value {
        CapabilityKind::NetworkConnect => CoreCapability::NetworkConnect,
        CapabilityKind::NetworkListen => CoreCapability::NetworkListen,
        CapabilityKind::FilesystemRead => CoreCapability::FilesystemRead,
        CapabilityKind::FilesystemWrite => CoreCapability::FilesystemWrite,
        CapabilityKind::ConfigRead => CoreCapability::ConfigRead,
        CapabilityKind::ReportWrite => CoreCapability::ReportWrite,
        CapabilityKind::Crypto => CoreCapability::Crypto,
        CapabilityKind::DnsResolve => CoreCapability::DnsResolve,
        CapabilityKind::ProcessSpawn => CoreCapability::ProcessSpawn,
        CapabilityKind::EnvironmentRead => CoreCapability::EnvironmentRead,
    }
}

fn execution_from_core(
    value: CoreExecution,
    record: Option<&polyglid_core::store::execution_store::DbJobRecord>,
) -> Execution {
    let metrics = value.metrics.as_ref();
    let plugin_id = record.map_or_else(
        || {
            Path::new(&value.plugin_path).file_stem().map_or_else(
                || value.plugin_path.clone(),
                |name| name.to_string_lossy().into_owned(),
            )
        },
        |record| record.plugin_id.clone(),
    );
    Execution {
        id: JobId::new(value.id),
        plugin_id,
        project_id: record.and_then(|record| record.project_id.clone()),
        approval_ids: record.map_or_else(Vec::new, |record| record.approval_ids.clone()),
        target: value.target,
        state: execution_state_from_core(value.state),
        started_at: record.map_or_else(
            || metrics.map_or(0, |metrics| metrics.timestamp),
            |record| record.started_at,
        ),
        duration_ms: metrics.map_or_else(
            || record.map_or(0, |record| record.duration_ms),
            |metrics| metrics.duration.as_millis() as u64,
        ),
        error: value
            .error
            .or_else(|| record.and_then(|record| record.error_message.clone())),
        fuel_consumed: metrics
            .and_then(|metrics| metrics.fuel_consumed)
            .or_else(|| record.map(|record| record.fuel_consumed)),
        created_at: record.map_or(0, |record| record.created_at),
        report: value.report.map(execution_report_from_core),
    }
}

fn execution_state_from_core(value: CoreExecutionState) -> ExecutionState {
    match value {
        CoreExecutionState::Queued => ExecutionState::Queued,
        CoreExecutionState::Starting => ExecutionState::Starting,
        CoreExecutionState::Running => ExecutionState::Running,
        CoreExecutionState::Completed => ExecutionState::Completed,
        CoreExecutionState::Failed => ExecutionState::Failed,
        CoreExecutionState::Cancelled => ExecutionState::Cancelled,
        CoreExecutionState::TimedOut => ExecutionState::TimedOut,
    }
}

fn execution_metrics_from_core(value: CoreExecutionMetrics) -> ExecutionMetrics {
    ExecutionMetrics {
        duration_ms: value.duration.as_millis() as u64,
        fuel_consumed: value.fuel_consumed,
        memory_used: value.memory_used,
        timestamp: value.timestamp,
        stage: value.stage,
    }
}

fn execution_report_from_core(value: CorePluginReport) -> ExecutionReport {
    ExecutionReport {
        plugin_name: value.plugin_name,
        target_tested: value.target_tested,
        issues: value.issues.into_iter().map(issue_from_core).collect(),
        summary: value.summary,
    }
}

fn report_from_db(value: polyglid_core::store::report_store::DbReportRecord) -> Report {
    Report {
        id: value.id,
        job_id: JobId::new(value.job_id),
        plugin_id: value.plugin_id,
        project_id: value.project_id,
        plugin_version: value.plugin_version,
        plugin_checksum: value.plugin_checksum,
        target: value.target,
        summary: value.summary,
        issues: value.issues.into_iter().map(issue_from_core).collect(),
        filepath: value.filepath,
        duration_ms: value.duration_ms,
        fuel_consumed: value.fuel_consumed,
        memory_used: value.memory_used,
        security_profile: value.security_profile,
        report_format_version: value.report_format_version,
        created_at: value.created_at,
    }
}

fn approval_from_db(value: DbPermissionRecord) -> Approval {
    Approval {
        id: value.id,
        plugin_id: value.plugin_id.as_str().to_string(),
        request: capability_request_from_core(value.request),
        decision: match value.decision {
            CorePermissionDecision::Allow => ApprovalDecision::Allow,
            CorePermissionDecision::Deny { .. } => ApprovalDecision::Deny,
        },
        duration: match value.duration {
            CoreApprovalDuration::Once => ApprovalDuration::Once,
            CoreApprovalDuration::Session => ApprovalDuration::Session,
            CoreApprovalDuration::Workspace => ApprovalDuration::Workspace,
        },
        expiration: value.expiration,
        revoked: value.revoked_at.is_some(),
    }
}

fn issue_from_core(value: CoreIssue) -> Issue {
    Issue {
        title: value.title,
        severity: severity_from_core(value.severity),
        description: value.description,
        recommendation: value.recommendation,
    }
}

fn issue_to_core(value: Issue) -> CoreIssue {
    CoreIssue {
        title: value.title,
        severity: severity_to_core(value.severity),
        description: value.description,
        recommendation: value.recommendation,
    }
}

fn severity_from_core(value: CoreSeverity) -> Severity {
    match value {
        CoreSeverity::Info => Severity::Info,
        CoreSeverity::Low => Severity::Low,
        CoreSeverity::Medium => Severity::Medium,
        CoreSeverity::High => Severity::High,
        CoreSeverity::Critical => Severity::Critical,
    }
}

fn severity_to_core(value: Severity) -> CoreSeverity {
    match value {
        Severity::Info => CoreSeverity::Info,
        Severity::Low => CoreSeverity::Low,
        Severity::Medium => CoreSeverity::Medium,
        Severity::High => CoreSeverity::High,
        Severity::Critical => CoreSeverity::Critical,
    }
}

fn parse_plugin_id(value: &str) -> ClientResult<PluginId> {
    PluginId::new(value).map_err(|error| ClientError::InvalidInput {
        field: "plugin id",
        message: error.to_string(),
    })
}

#[cfg(test)]
fn capability_approval_gaps(
    requested: &[CapabilityRequest],
    approved: &[CapabilityRequest],
) -> (Vec<CapabilityRequest>, Vec<CapabilityRequest>) {
    let missing = requested
        .iter()
        .filter(|request| !approved.contains(request))
        .cloned()
        .collect();
    let unexpected = approved
        .iter()
        .filter(|request| !requested.contains(request))
        .cloned()
        .collect();
    (missing, unexpected)
}

fn setting_bool(value: Result<Option<String>, String>, fallback: bool) -> ClientResult<bool> {
    let value = value.map_err(|error| ClientError::operation("load shell preferences", error))?;
    Ok(value.map_or(fallback, |value| value == "true"))
}

fn setting_number(
    value: Result<Option<String>, String>,
    fallback: f64,
    minimum: f64,
    maximum: f64,
) -> ClientResult<f64> {
    let value = value.map_err(|error| ClientError::operation("load shell preferences", error))?;
    Ok(value
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite())
        .unwrap_or(fallback)
        .clamp(minimum, maximum))
}

fn setting_u64(
    value: Result<Option<String>, String>,
    fallback: u64,
    minimum: u64,
    maximum: u64,
) -> ClientResult<u64> {
    let value =
        value.map_err(|error| ClientError::operation("load execution preferences", error))?;
    Ok(value
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(fallback)
        .clamp(minimum, maximum))
}

fn setup_report(initialization: RuntimeInitialization, migrations: MigrationReport) -> SetupReport {
    let status = if migrations.previous_version == 0 && migrations.current_version > 0 {
        SetupStatus::FirstRun
    } else if !migrations.applied_versions.is_empty() {
        SetupStatus::Migrated
    } else {
        SetupStatus::Ready
    };
    SetupReport {
        status,
        created_directories: initialization.created_directories,
        database_previous_version: migrations.previous_version,
        database_version: migrations.current_version,
        applied_migrations: migrations.applied_versions,
    }
}

fn enroll_official_publisher(
    store: &WorkspaceStore,
    configured_key: Option<&str>,
) -> ClientResult<()> {
    let Some(public_key) = configured_key.map(str::trim).filter(|key| !key.is_empty()) else {
        return Ok(());
    };
    let key_bytes = hex::decode(public_key).map_err(|error| ClientError::InvalidInput {
        field: "official publisher key",
        message: format!("the release-embedded key is not valid hexadecimal: {error}"),
    })?;
    if key_bytes.len() != 32 {
        return Err(ClientError::InvalidInput {
            field: "official publisher key",
            message: "the release-embedded Ed25519 key must be exactly 32 bytes".to_string(),
        });
    }
    let normalized_key = hex::encode(key_bytes);
    let fingerprint =
        polyglid_core::security::publisher::PublisherManager::compute_fingerprint(&normalized_key)
            .map_err(|error| ClientError::operation("fingerprint official publisher", error))?;
    let trust = store.trust_store();

    if let Some(existing) = trust
        .get_publisher("polyglid-official-recon")
        .map_err(|error| ClientError::operation("load official publisher", error))?
    {
        if existing.public_key != normalized_key {
            return Err(ClientError::Conflict(
                "the stored official publisher key differs from the key pinned in this build"
                    .to_string(),
            ));
        }
        return Ok(());
    }
    if trust
        .get_publisher_by_fingerprint(&fingerprint)
        .map_err(|error| ClientError::operation("find official publisher", error))?
        .is_some()
    {
        return Ok(());
    }
    trust
        .add_publisher(
            "polyglid-official-recon",
            "PolyGlid Official Recon Publisher",
            &normalized_key,
            &fingerprint,
            "Official",
        )
        .map_err(|error| ClientError::operation("enroll official publisher", error))
}

/// Describe a resolved path the same way the workspace tree does.
fn workspace_relative_path(root: &Path, path: &Path, fallback: &str) -> String {
    path.strip_prefix(root)
        .map(|relative| relative.to_string_lossy().into_owned())
        .unwrap_or_else(|_| fallback.to_owned())
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_directory(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!("polyglid-desktop-{label}-{}", uuid::Uuid::new_v4()))
    }

    #[test]
    fn bootstrap_returns_ui_models_and_persists_real_targets() {
        let root = temporary_directory("gateway");
        let data = root.join("data");
        let workspace = root.join("projects");
        let client = LocalClient::open(&data, &workspace).expect("open local client");

        let bootstrap = client.bootstrap().expect("bootstrap");
        assert_eq!(
            bootstrap.active_workspace.root_path,
            workspace.to_string_lossy()
        );
        assert!(bootstrap.projects.is_empty());
        assert!(bootstrap.targets.is_empty());

        let saved = client
            .add_target("example.com", Some("Production"), "project-test")
            .expect("save target");
        assert_eq!(saved.group.as_deref(), Some("Production"));
        assert_eq!(client.list_targets().expect("list targets"), vec![saved]);

        fs::remove_dir_all(root).expect("clean temporary client");
    }

    #[test]
    fn first_run_setup_is_reported_and_is_idempotent() {
        let root = temporary_directory("setup-report");
        let data = root.join("data");
        let workspace = root.join("projects");

        let (_, first) = LocalClient::open_with_setup(&data, &workspace).expect("first setup");
        assert_eq!(first.status, SetupStatus::FirstRun);
        assert_eq!(first.database_previous_version, 0);
        assert_eq!(first.database_version, 7);
        assert_eq!(first.applied_migrations, (1..=7).collect::<Vec<_>>());
        assert_eq!(first.created_directories.len(), 7);

        let (_, second) = LocalClient::open_with_setup(&data, &workspace).expect("second setup");
        assert_eq!(second.status, SetupStatus::Ready);
        assert_eq!(second.database_previous_version, 7);
        assert!(second.applied_migrations.is_empty());
        assert!(second.created_directories.is_empty());

        fs::remove_dir_all(root).expect("clean setup test");
    }

    #[test]
    fn shell_preferences_are_clamped_and_reloaded() {
        let root = temporary_directory("preferences");
        let client =
            LocalClient::open(root.join("data"), root.join("projects")).expect("open local client");
        client
            .save_shell_preferences(&ShellPreferences {
                sidebar_visible: false,
                bottom_panel_visible: true,
                sidebar_width: 900.0,
                bottom_panel_height: 20.0,
            })
            .expect("save preferences");
        client
            .save_execution_preferences(&ExecutionPreferences {
                fuel_limit: 12_345,
                timeout_seconds: 45,
                memory_limit_bytes: Some(16 * 1024 * 1024),
            })
            .expect("save execution preferences");

        let snapshot = client.bootstrap().expect("bootstrap");
        assert!(!snapshot.shell.sidebar_visible);
        assert_eq!(snapshot.shell.sidebar_width, 480.0);
        assert_eq!(snapshot.shell.bottom_panel_height, 120.0);
        assert_eq!(snapshot.execution.fuel_limit, 12_345);
        assert_eq!(snapshot.execution.timeout_seconds, 45);
        assert_eq!(
            snapshot.execution.memory_limit_bytes,
            Some(16 * 1024 * 1024)
        );

        fs::remove_dir_all(root).expect("clean temporary client");
    }

    #[test]
    fn execution_approval_must_match_every_requested_capability() {
        let dns = CapabilityRequest {
            capability: CapabilityKind::DnsResolve,
            scope: CapabilityScope::Target {
                target: "example.com".to_string(),
            },
        };
        let report = CapabilityRequest {
            capability: CapabilityKind::ReportWrite,
            scope: CapabilityScope::PathPrefix {
                path: "reports".to_string(),
            },
        };
        let process = CapabilityRequest {
            capability: CapabilityKind::ProcessSpawn,
            scope: CapabilityScope::Any,
        };
        let requested = [dns.clone(), report.clone()];

        let (missing, unexpected) =
            capability_approval_gaps(&requested, std::slice::from_ref(&dns));
        assert_eq!(missing, vec![report.clone()]);
        assert!(unexpected.is_empty());

        let (missing, unexpected) =
            capability_approval_gaps(&requested, &[dns.clone(), report.clone(), process.clone()]);
        assert!(missing.is_empty());
        assert_eq!(unexpected, vec![process]);

        let (missing, unexpected) = capability_approval_gaps(&requested, &requested);
        assert!(missing.is_empty());
        assert!(unexpected.is_empty());
    }

    #[test]
    fn release_pinned_publisher_is_enrolled_without_overwriting_a_different_key() {
        let store = WorkspaceStore::new(Path::new(":memory:")).expect("workspace");
        let official_key = hex::encode([7_u8; 32]);
        enroll_official_publisher(&store, Some(&official_key)).expect("enroll publisher");
        let publisher = store
            .trust_store()
            .get_publisher("polyglid-official-recon")
            .expect("publisher query")
            .expect("publisher");
        assert_eq!(publisher.public_key, official_key);

        let different_key = hex::encode([8_u8; 32]);
        assert!(enroll_official_publisher(&store, Some(&different_key))
            .expect_err("pinned key mismatch must fail")
            .to_string()
            .contains("differs"));
    }

    #[test]
    fn documents_round_trip_inside_the_workspace() {
        let root = temporary_directory("document-roundtrip");
        let data = root.join("data");
        let workspace = root.join("projects");
        let client = LocalClient::open(&data, &workspace).expect("open local client");
        let workspace_id = client.bootstrap().expect("bootstrap").active_workspace.id;
        fs::write(workspace.join("notes.txt"), "first\n").expect("seed document");

        let document = client
            .read_document(&workspace_id, "notes.txt")
            .expect("read document");
        assert_eq!(document.relative_path, "notes.txt");
        assert_eq!(document.contents, "first\n");
        assert_eq!(document.byte_size, 6);

        client
            .write_document(&workspace_id, "notes.txt", "second\n")
            .expect("write document");
        assert_eq!(
            fs::read_to_string(workspace.join("notes.txt")).expect("reread document"),
            "second\n"
        );

        fs::remove_dir_all(root).expect("clean temporary client");
    }

    #[test]
    fn documents_reject_paths_that_escape_the_workspace() {
        let root = temporary_directory("document-escape");
        let data = root.join("data");
        let workspace = root.join("projects");
        let client = LocalClient::open(&data, &workspace).expect("open local client");
        let workspace_id = client.bootstrap().expect("bootstrap").active_workspace.id;
        fs::write(root.join("outside.txt"), "secret\n").expect("seed outside document");

        for escape in ["../outside.txt", "/etc/hostname"] {
            assert!(
                client.read_document(&workspace_id, escape).is_err(),
                "{escape} must not be readable"
            );
            assert!(
                client
                    .write_document(&workspace_id, escape, "overwritten")
                    .is_err(),
                "{escape} must not be writable"
            );
        }
        assert_eq!(
            fs::read_to_string(root.join("outside.txt")).expect("reread outside document"),
            "secret\n"
        );

        fs::remove_dir_all(root).expect("clean temporary client");
    }

    #[test]
    fn documents_reject_directories_missing_files_and_non_text() {
        let root = temporary_directory("document-guard");
        let data = root.join("data");
        let workspace = root.join("projects");
        let client = LocalClient::open(&data, &workspace).expect("open local client");
        let workspace_id = client.bootstrap().expect("bootstrap").active_workspace.id;
        fs::create_dir(workspace.join("folder")).expect("seed directory");
        fs::write(workspace.join("blob.bin"), [0xff_u8, 0xfe]).expect("seed binary document");

        assert!(client.read_document(&workspace_id, "folder").is_err());
        assert!(client.read_document(&workspace_id, "missing.txt").is_err());
        assert!(client
            .write_document(&workspace_id, "missing.txt", "new")
            .is_err());
        assert!(client.read_document(&workspace_id, "blob.bin").is_err());

        fs::remove_dir_all(root).expect("clean temporary client");
    }
}
