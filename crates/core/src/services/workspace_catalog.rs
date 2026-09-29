use std::fs;
use std::path::Path;

use crate::store::{DbProject, DbWorkspace, WorkspaceStore};

use super::workspace_discovery::discover_direct_children;

#[derive(Clone)]
pub struct WorkspaceCatalogService {
    pub(super) store: WorkspaceStore,
}

impl WorkspaceCatalogService {
    pub fn open(database_path: &Path) -> Result<Self, String> {
        Ok(Self {
            store: WorkspaceStore::new(database_path)?,
        })
    }

    pub fn from_store(store: WorkspaceStore) -> Self {
        Self { store }
    }

    pub fn list_workspaces(&self) -> Result<Vec<DbWorkspace>, String> {
        self.store.workspace_catalog().list()
    }

    pub fn active_workspace(&self) -> Result<Option<DbWorkspace>, String> {
        Ok(self
            .list_workspaces()?
            .into_iter()
            .find(|workspace| workspace.is_active))
    }

    pub fn register_workspace(&self, name: &str, root: &Path) -> Result<DbWorkspace, String> {
        validate_name(name, "workspace")?;
        fs::create_dir_all(root)
            .map_err(|err| format!("failed to create workspace '{}': {err}", root.display()))?;
        let root = root
            .canonicalize()
            .map_err(|err| format!("failed to resolve workspace '{}': {err}", root.display()))?;
        if !root.is_dir() {
            return Err("workspace root must be a directory".to_string());
        }

        let now = now_secs();
        let workspace = DbWorkspace {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.trim().to_string(),
            root_path: root.to_string_lossy().to_string(),
            is_active: false,
            discovery_state: "idle".to_string(),
            last_error: None,
            created_at: now,
            updated_at: now,
            last_opened_at: None,
        };
        let workspaces = self.store.workspace_catalog();
        workspaces.upsert(&workspace)?;
        let saved = workspaces
            .list()?
            .into_iter()
            .find(|item| item.root_path == workspace.root_path)
            .ok_or_else(|| "saved workspace could not be reloaded".to_string())?;
        if self.active_workspace()?.is_none() {
            workspaces.set_active(&saved.id, now)?;
        }
        self.discover(&saved.id)?;
        workspaces
            .get(&saved.id)?
            .ok_or_else(|| "workspace disappeared after discovery".to_string())
    }

    pub fn activate(&self, workspace_id: &str) -> Result<(), String> {
        self.store
            .workspace_catalog()
            .set_active(workspace_id, now_secs())
    }

    pub fn list_projects(&self, workspace_id: &str) -> Result<Vec<DbProject>, String> {
        self.store.project_catalog().list(workspace_id)
    }

    pub fn discover(&self, workspace_id: &str) -> Result<Vec<DbProject>, String> {
        let workspaces = self.store.workspace_catalog();
        let workspace = workspaces
            .get(workspace_id)?
            .ok_or_else(|| format!("workspace '{workspace_id}' was not found"))?;
        let now = now_secs();
        workspaces.set_discovery(workspace_id, "loading", None, now)?;
        match discover_direct_children(workspace_id, Path::new(&workspace.root_path), now) {
            Ok(projects) => {
                // Sync and the follow-up read share one lock acquisition so no
                // other operation can interleave between them.
                let projects = self.store.project_catalog().sync_with(
                    workspace_id,
                    &projects,
                    now,
                    |conn| crate::store::ProjectCatalogStore::list_from_conn(conn, workspace_id),
                )?;
                workspaces.set_discovery(workspace_id, "ready", None, now)?;
                Ok(projects)
            }
            Err(error) => {
                workspaces.set_discovery(workspace_id, "error", Some(&error), now)?;
                Err(error)
            }
        }
    }
}

pub(super) fn validate_name(value: &str, kind: &str) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{kind} name cannot be empty"));
    }
    if trimmed.len() > 100 {
        return Err(format!("{kind} name is too long"));
    }
    let path = Path::new(trimmed);
    if trimmed == "."
        || trimmed == ".."
        || path.components().count() != 1
        || path.is_absolute()
        || trimmed.contains(['/', '\\'])
    {
        return Err(format!("{kind} name must be one safe path segment"));
    }
    Ok(())
}

pub(super) fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
