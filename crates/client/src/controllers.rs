use crate::client::{
    Approval, BootstrapSnapshot, ClientGateway, ClientResult, Document, Execution,
    ExecutionPreferences, ExecutionSubscription, JobId, PermissionDecisionRequest, Plugin,
    PluginInspection, Project, Report, ReportFormat, SavedTarget, ShellPreferences,
    StartExecutionRequest, Workspace, WorkspaceEntry,
};

pub type DesktopControllers = FeatureControllers<crate::client::LocalClient>;

#[derive(Clone)]
pub struct FeatureControllers<G: ClientGateway> {
    pub application: ApplicationController<G>,
    pub projects: ProjectsController<G>,
    pub plugins: PluginsController<G>,
    pub scanner: ScannerController<G>,
    pub executions: ExecutionsController<G>,
    pub reports: ReportsController<G>,
    pub settings: SettingsController<G>,
    pub documents: DocumentsController<G>,
}

impl<G: ClientGateway> FeatureControllers<G> {
    pub fn new(gateway: G) -> Self {
        Self {
            application: ApplicationController(gateway.clone()),
            projects: ProjectsController(gateway.clone()),
            plugins: PluginsController(gateway.clone()),
            scanner: ScannerController(gateway.clone()),
            executions: ExecutionsController(gateway.clone()),
            reports: ReportsController(gateway.clone()),
            settings: SettingsController(gateway.clone()),
            documents: DocumentsController(gateway),
        }
    }
}

#[derive(Clone)]
pub struct DocumentsController<G: ClientGateway>(G);

impl<G: ClientGateway> DocumentsController<G> {
    pub fn list_directory(
        &self,
        workspace_id: &str,
        relative_directory: &str,
    ) -> ClientResult<Vec<WorkspaceEntry>> {
        self.0
            .list_workspace_entries(workspace_id, relative_directory)
    }

    pub fn read(&self, workspace_id: &str, relative_path: &str) -> ClientResult<Document> {
        self.0.read_document(workspace_id, relative_path)
    }

    pub fn write(
        &self,
        workspace_id: &str,
        relative_path: &str,
        contents: &str,
    ) -> ClientResult<()> {
        self.0.write_document(workspace_id, relative_path, contents)
    }
}

#[derive(Clone)]
pub struct ApplicationController<G: ClientGateway>(G);

impl<G: ClientGateway> ApplicationController<G> {
    pub fn bootstrap(&self) -> ClientResult<BootstrapSnapshot> {
        self.0.bootstrap()
    }

    pub fn list_executions(&self) -> ClientResult<Vec<Execution>> {
        self.0.list_executions()
    }

    pub fn list_reports(&self) -> ClientResult<Vec<Report>> {
        self.0.list_reports()
    }

    pub fn list_targets(&self) -> ClientResult<Vec<SavedTarget>> {
        self.0.list_targets()
    }

    pub fn subscribe_executions(&self) -> ClientResult<ExecutionSubscription> {
        self.0.subscribe_executions()
    }

    pub fn activate_workspace(&self, workspace_id: &str) -> ClientResult<()> {
        self.0.activate_workspace(workspace_id)
    }

    pub fn register_workspace(&self, name: &str, root_path: &str) -> ClientResult<Workspace> {
        self.0.register_workspace(name, root_path)
    }
}

#[derive(Clone)]
pub struct ProjectsController<G: ClientGateway>(G);

impl<G: ClientGateway> ProjectsController<G> {
    pub fn list_workspaces(&self) -> ClientResult<Vec<Workspace>> {
        self.0.list_workspaces()
    }

    pub fn create(&self, workspace_id: &str, name: &str) -> ClientResult<Project> {
        self.0.create_project(workspace_id, name)
    }

    pub fn rename(&self, project_id: &str, name: &str) -> ClientResult<Project> {
        self.0.rename_project(project_id, name)
    }

    pub fn remove(&self, project_id: &str, delete_files: bool) -> ClientResult<()> {
        self.0.remove_project(project_id, delete_files)
    }
}

#[derive(Clone)]
pub struct PluginsController<G: ClientGateway>(G);

impl<G: ClientGateway> PluginsController<G> {
    pub fn inspect(&self, path: &str) -> ClientResult<PluginInspection> {
        self.0.inspect_plugin(path)
    }

    pub fn install(&self, path: &str) -> ClientResult<Plugin> {
        self.0.install_plugin(path)
    }

    pub fn set_enabled(&self, plugin_id: &str, enabled: bool) -> ClientResult<()> {
        self.0.set_plugin_enabled(plugin_id, enabled)
    }

    pub fn uninstall(&self, plugin_id: &str) -> ClientResult<()> {
        self.0.uninstall_plugin(plugin_id)
    }
}

#[derive(Clone)]
pub struct ScannerController<G: ClientGateway>(G);

impl<G: ClientGateway> ScannerController<G> {
    pub fn add_target(
        &self,
        name: &str,
        group: Option<&str>,
        project_id: &str,
    ) -> ClientResult<SavedTarget> {
        self.0.add_target(name, group, project_id)
    }

    pub fn remove_target(&self, name: &str, project_id: &str) -> ClientResult<()> {
        self.0.remove_target(name, project_id)
    }

    pub fn record_decision(&self, request: PermissionDecisionRequest) -> ClientResult<Approval> {
        self.0.record_permission_decision(request)
    }

    pub fn start(&self, request: StartExecutionRequest) -> ClientResult<JobId> {
        self.0.start_execution(request)
    }
}

#[derive(Clone)]
pub struct ExecutionsController<G: ClientGateway>(G);

impl<G: ClientGateway> ExecutionsController<G> {
    pub fn cancel(&self, job_id: JobId) -> ClientResult<()> {
        self.0.cancel_execution(job_id)
    }
}

#[derive(Clone)]
pub struct ReportsController<G: ClientGateway>(G);

impl<G: ClientGateway> ReportsController<G> {
    pub fn export(&self, report_id: &str, format: ReportFormat) -> ClientResult<String> {
        self.0.export_report(report_id, format)
    }
}

#[derive(Clone)]
pub struct SettingsController<G: ClientGateway>(G);

impl<G: ClientGateway> SettingsController<G> {
    pub fn save_shell(&self, preferences: &ShellPreferences) -> ClientResult<()> {
        self.0.save_shell_preferences(preferences)
    }

    pub fn save_execution(&self, preferences: &ExecutionPreferences) -> ClientResult<()> {
        self.0.save_execution_preferences(preferences)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::LocalClient;

    #[test]
    fn controllers_route_operations_through_one_shared_gateway() {
        let root =
            std::env::temp_dir().join(format!("polyglid-controller-test-{}", uuid::Uuid::now_v7()));
        let data = root.join("data");
        let projects = root.join("projects");
        std::fs::create_dir_all(&projects).expect("project root");

        let controllers =
            DesktopControllers::new(LocalClient::open(&data, &projects).expect("local client"));
        let snapshot = controllers.application.bootstrap().expect("bootstrap");
        let project = controllers
            .projects
            .create(&snapshot.active_workspace.id, "controller-project")
            .expect("create project");
        let saved = controllers
            .scanner
            .add_target("example.com", None, &project.id)
            .expect("save target");

        assert_eq!(saved.project_id.as_deref(), Some(project.id.as_str()));
        assert!(controllers
            .application
            .list_targets()
            .expect("list targets")
            .iter()
            .any(|target| target.name == "example.com"));
        assert!(controllers
            .documents
            .list_directory(&snapshot.active_workspace.id, "")
            .expect("list workspace root")
            .iter()
            .any(|entry| entry.name == "controller-project" && entry.is_directory));
        assert!(controllers
            .documents
            .list_directory(&snapshot.active_workspace.id, "..")
            .is_err());

        std::fs::write(
            projects.join("controller-project").join("README.md"),
            "# controller\n",
        )
        .expect("seed controller document");
        let document = controllers
            .documents
            .read(
                &snapshot.active_workspace.id,
                "controller-project/README.md",
            )
            .expect("read controller document");
        assert_eq!(document.contents, "# controller\n");
        controllers
            .documents
            .write(
                &snapshot.active_workspace.id,
                "controller-project/README.md",
                "# updated\n",
            )
            .expect("write controller document");
        assert_eq!(
            std::fs::read_to_string(projects.join("controller-project").join("README.md"))
                .expect("reread controller document"),
            "# updated\n"
        );

        std::fs::remove_dir_all(root).expect("remove test workspace");
    }
}
