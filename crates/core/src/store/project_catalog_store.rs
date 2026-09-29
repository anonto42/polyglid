use std::sync::{Arc, Mutex};

use rusqlite::{params, Connection};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DbProject {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub path: String,
    pub kind: String,
    pub archived: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

pub struct ProjectCatalogStore {
    conn: Arc<Mutex<Connection>>,
}

impl ProjectCatalogStore {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    pub fn sync(&self, workspace_id: &str, projects: &[DbProject], now: i64) -> Result<(), String> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn
            .transaction()
            .map_err(|err| format!("failed to sync projects: {err}"))?;
        sync_in_transaction(&tx, workspace_id, projects, now)?;
        tx.commit()
            .map_err(|err| format!("failed to commit project sync: {err}"))
    }

    /// List a workspace's active, non-excluded projects using an existing
    /// connection. Used with `sync_with` to read back the synced catalog
    /// without reacquiring the store lock.
    pub fn list_from_conn(conn: &Connection, workspace_id: &str) -> Result<Vec<DbProject>, String> {
        let mut stmt = conn
            .prepare(
                "SELECT id,workspace_id,name,path,kind,archived,created_at,updated_at
             FROM projects WHERE workspace_id=?1 AND archived=0 AND excluded=0
             ORDER BY name COLLATE NOCASE",
            )
            .map_err(|err| format!("failed to prepare project query: {err}"))?;
        let rows = stmt
            .query_map([workspace_id], map_project)
            .map_err(|err| format!("failed to query projects: {err}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| format!("failed to read project row: {err}"))
    }

    pub fn list(&self, workspace_id: &str) -> Result<Vec<DbProject>, String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT id,workspace_id,name,path,kind,archived,created_at,updated_at
             FROM projects WHERE workspace_id=?1 AND archived=0 AND excluded=0
             ORDER BY name COLLATE NOCASE",
            )
            .map_err(|err| format!("failed to prepare project query: {err}"))?;
        let rows = stmt
            .query_map([workspace_id], map_project)
            .map_err(|err| format!("failed to query projects: {err}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| format!("failed to read project row: {err}"))
    }

    pub fn get(&self, id: &str) -> Result<Option<DbProject>, String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id,workspace_id,name,path,kind,archived,created_at,updated_at FROM projects WHERE id=?1")
            .map_err(|err| format!("failed to prepare project lookup: {err}"))?;
        match stmt.query_row([id], map_project) {
            Ok(value) => Ok(Some(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => Err(format!("failed to load project: {err}")),
        }
    }

    pub fn update_path(&self, id: &str, name: &str, path: &str, now: i64) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE projects SET name=?2,path=?3,updated_at=?4 WHERE id=?1",
            params![id, name, path, now],
        )
        .map(|_| ())
        .map_err(|err| format!("failed to rename project: {err}"))
    }

    pub fn archive(&self, id: &str, now: i64) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE projects SET archived=1,excluded=1,updated_at=?2 WHERE id=?1",
            params![id, now],
        )
        .map(|_| ())
        .map_err(|err| format!("failed to archive project: {err}"))
    }

    /// Replace the discovered project set for a workspace inside one held lock
    /// and transaction. `replace` receives a locked connection because
    /// discovery follows the sync with a read of the same data; reusing the
    /// lock avoids interleaving with other catalog operations between the two
    /// steps.
    pub fn sync_with<F, T>(
        &self,
        workspace_id: &str,
        projects: &[DbProject],
        now: i64,
        replace: F,
    ) -> Result<T, String>
    where
        F: FnOnce(&Connection) -> Result<T, String>,
    {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn
            .transaction()
            .map_err(|err| format!("failed to sync projects: {err}"))?;
        sync_in_transaction(&tx, workspace_id, projects, now)?;
        tx.commit()
            .map_err(|err| format!("failed to commit project sync: {err}"))?;
        replace(&conn)
    }
}

fn sync_in_transaction(
    tx: &rusqlite::Transaction<'_>,
    workspace_id: &str,
    projects: &[DbProject],
    now: i64,
) -> Result<(), String> {
    tx.execute(
        "UPDATE projects SET archived=1,updated_at=?2 WHERE workspace_id=?1 AND excluded=0",
        params![workspace_id, now],
    )
    .map_err(|err| format!("failed to mark stale projects: {err}"))?;
    for project in projects {
        tx.execute(
            "INSERT INTO projects (id,workspace_id,name,path,kind,archived,created_at,updated_at)
             VALUES (?1,?2,?3,?4,?5,0,?6,?7)
             ON CONFLICT(workspace_id,path) DO UPDATE SET
                name=excluded.name,
                kind=excluded.kind,
                archived=CASE WHEN projects.excluded=1 THEN 1 ELSE 0 END,
                updated_at=excluded.updated_at",
            params![
                project.id,
                project.workspace_id,
                project.name,
                project.path,
                project.kind,
                project.created_at,
                project.updated_at
            ],
        )
        .map_err(|err| format!("failed to save discovered project: {err}"))?;
    }
    Ok(())
}

fn map_project(row: &rusqlite::Row<'_>) -> rusqlite::Result<DbProject> {
    Ok(DbProject {
        id: row.get(0)?,
        workspace_id: row.get(1)?,
        name: row.get(2)?,
        path: row.get(3)?,
        kind: row.get(4)?,
        archived: row.get::<_, i32>(5)? != 0,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}
