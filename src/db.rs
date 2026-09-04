use crate::event::CommandEvent;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Result};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct WorkflowRecord {
    pub id: i64,
    pub project_root: String,
    pub shortcut: String,
    pub name: String,
    pub commands_json: String,
    pub frequency: i32,
    pub last_used: Option<DateTime<Utc>>,
    pub is_pinned: bool,
    pub created_at: DateTime<Utc>,
}

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;

            CREATE TABLE IF NOT EXISTS command_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                command TEXT NOT NULL,
                cwd TEXT NOT NULL,
                project_root TEXT NOT NULL,
                exit_status INTEGER NOT NULL,
                timestamp TEXT NOT NULL,
                shell TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_events_project_time 
            ON command_events(project_root, timestamp);

            CREATE TABLE IF NOT EXISTS workflows (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_root TEXT NOT NULL,
                shortcut TEXT NOT NULL,
                name TEXT NOT NULL,
                commands_json TEXT NOT NULL,
                frequency INTEGER DEFAULT 1,
                last_used TEXT,
                is_pinned INTEGER DEFAULT 0,
                created_at TEXT NOT NULL,
                UNIQUE(project_root, shortcut)
            );

            CREATE TABLE IF NOT EXISTS workflow_executions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                workflow_id INTEGER NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
                executed_at TEXT NOT NULL,
                success INTEGER NOT NULL
            );
            ",
        )?;
        Ok(Self { conn })
    }

    pub fn insert_event(&self, event: &CommandEvent, project_root: &str) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO command_events (session_id, command, cwd, project_root, exit_status, timestamp, shell)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                event.session_id,
                event.command,
                event.cwd,
                project_root,
                event.exit_status,
                event.timestamp.to_rfc3339(),
                event.shell,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_recent_events_for_project(
        &self,
        project_root: &str,
        limit: usize,
    ) -> Result<Vec<CommandEvent>> {
        let mut stmt = self.conn.prepare(
            "SELECT session_id, command, cwd, exit_status, timestamp, shell
             FROM command_events
             WHERE project_root = ?1
             ORDER BY id DESC
             LIMIT ?2",
        )?;

        let rows = stmt.query_map(params![project_root, limit as i64], |row| {
            let ts_str: String = row.get(4)?;
            let timestamp = DateTime::parse_from_rfc3339(&ts_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            Ok(CommandEvent {
                session_id: row.get(0)?,
                command: row.get(1)?,
                cwd: row.get(2)?,
                exit_status: row.get(3)?,
                timestamp,
                shell: row.get(5)?,
            })
        })?;

        let mut events = Vec::new();
        for r in rows {
            events.push(r?);
        }
        events.reverse();
        Ok(events)
    }

    pub fn get_all_events_for_project(&self, project_root: &str) -> Result<Vec<CommandEvent>> {
        let mut stmt = self.conn.prepare(
            "SELECT session_id, command, cwd, exit_status, timestamp, shell
             FROM command_events
             WHERE project_root = ?1
             ORDER BY id ASC",
        )?;

        let rows = stmt.query_map(params![project_root], |row| {
            let ts_str: String = row.get(4)?;
            let timestamp = DateTime::parse_from_rfc3339(&ts_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            Ok(CommandEvent {
                session_id: row.get(0)?,
                command: row.get(1)?,
                cwd: row.get(2)?,
                exit_status: row.get(3)?,
                timestamp,
                shell: row.get(5)?,
            })
        })?;

        rows.collect()
    }

    pub fn save_workflow(
        &self,
        project_root: &str,
        shortcut: &str,
        name: &str,
        commands_json: &str,
        frequency: i32,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let existing_id: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM workflows
                 WHERE project_root = ?1 AND commands_json = ?2
                 LIMIT 1",
                params![project_root, commands_json],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(id) = existing_id {
            self.conn.execute(
                "UPDATE workflows SET frequency = ?1 WHERE id = ?2 AND is_pinned = 0",
                params![frequency, id],
            )?;
            return Ok(());
        }
        self.conn.execute(
            "INSERT INTO workflows (project_root, shortcut, name, commands_json, frequency, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(project_root, shortcut) DO UPDATE SET
                frequency = excluded.frequency,
                commands_json = excluded.commands_json,
                name = excluded.name
             WHERE is_pinned = 0",
            params![project_root, shortcut, name, commands_json, frequency, now],
        )?;
        Ok(())
    }

    pub fn remove_unpinned_workflows_not_in(
        &self,
        project_root: &str,
        shortcuts: &[String],
    ) -> Result<()> {
        if shortcuts.is_empty() {
            self.conn.execute(
                "DELETE FROM workflows WHERE project_root = ?1 AND is_pinned = 0",
                params![project_root],
            )?;
        } else {
            let placeholders = std::iter::repeat("?")
                .take(shortcuts.len())
                .collect::<Vec<_>>()
                .join(", ");
            let sql = format!(
                "DELETE FROM workflows WHERE project_root = ?1 AND is_pinned = 0 AND shortcut NOT IN ({})",
                placeholders
            );
            let mut values: Vec<&dyn rusqlite::ToSql> = vec![&project_root];
            values.extend(
                shortcuts
                    .iter()
                    .map(|shortcut| shortcut as &dyn rusqlite::ToSql),
            );
            self.conn.execute(&sql, values.as_slice())?;
        }
        Ok(())
    }

    pub fn get_workflows_for_project(&self, project_root: &str) -> Result<Vec<WorkflowRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, project_root, shortcut, name, commands_json, frequency, last_used, is_pinned, created_at
             FROM workflows
             WHERE project_root = ?1
             ORDER BY is_pinned DESC, frequency DESC"
        )?;

        let rows = stmt.query_map(params![project_root], |row| {
            let last_used_str: Option<String> = row.get(6)?;
            let last_used = last_used_str.and_then(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .ok()
                    .map(|dt| dt.with_timezone(&Utc))
            });

            let created_at_str: String = row.get(8)?;
            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            Ok(WorkflowRecord {
                id: row.get(0)?,
                project_root: row.get(1)?,
                shortcut: row.get(2)?,
                name: row.get(3)?,
                commands_json: row.get(4)?,
                frequency: row.get(5)?,
                last_used,
                is_pinned: row.get::<_, i32>(7)? != 0,
                created_at,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn find_workflow(
        &self,
        project_root: &str,
        shortcut: &str,
    ) -> Result<Option<WorkflowRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, project_root, shortcut, name, commands_json, frequency, last_used, is_pinned, created_at
             FROM workflows
             WHERE project_root = ?1 AND shortcut = ?2"
        )?;

        let mut rows = stmt.query_map(params![project_root, shortcut], |row| {
            let last_used_str: Option<String> = row.get(6)?;
            let last_used = last_used_str.and_then(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .ok()
                    .map(|dt| dt.with_timezone(&Utc))
            });

            let created_at_str: String = row.get(8)?;
            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            Ok(WorkflowRecord {
                id: row.get(0)?,
                project_root: row.get(1)?,
                shortcut: row.get(2)?,
                name: row.get(3)?,
                commands_json: row.get(4)?,
                frequency: row.get(5)?,
                last_used,
                is_pinned: row.get::<_, i32>(7)? != 0,
                created_at,
            })
        })?;

        if let Some(r) = rows.next() {
            Ok(Some(r?))
        } else {
            Ok(None)
        }
    }

    pub fn record_execution(&self, workflow_id: i64, success: bool) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO workflow_executions (workflow_id, executed_at, success) VALUES (?1, ?2, ?3)",
            params![workflow_id, now, if success { 1 } else { 0 }],
        )?;
        self.conn.execute(
            "UPDATE workflows SET last_used = ?1 WHERE id = ?2",
            params![now, workflow_id],
        )?;
        Ok(())
    }

    pub fn update_workflow_metadata(
        &self,
        project_root: &str,
        current_shortcut: &str,
        new_shortcut: Option<&str>,
        new_name: Option<&str>,
    ) -> Result<bool> {
        let workflow = self.find_workflow(project_root, current_shortcut)?;
        let Some(workflow) = workflow else {
            return Ok(false);
        };
        let shortcut = new_shortcut.unwrap_or(&workflow.shortcut);
        let name = new_name.unwrap_or(&workflow.name);
        self.conn.execute(
            "UPDATE workflows
             SET shortcut = ?1, name = ?2, is_pinned = 1
             WHERE id = ?3",
            params![shortcut, name, workflow.id],
        )?;
        Ok(true)
    }

    pub fn total_stats(&self) -> Result<(i64, i64, i64)> {
        let event_count: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM command_events", [], |r| r.get(0))?;
        let workflow_count: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM workflows", [], |r| r.get(0))?;
        let project_count: i64 = self.conn.query_row(
            "SELECT COUNT(DISTINCT project_root) FROM command_events",
            [],
            |r| r.get(0),
        )?;
        Ok((event_count, workflow_count, project_count))
    }
}
