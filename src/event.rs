use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandEvent {
    pub session_id: String,
    pub command: String,
    pub cwd: String,
    pub exit_status: i32,
    pub timestamp: DateTime<Utc>,
    pub shell: String,
}

impl CommandEvent {
    pub fn new(
        session_id: String,
        command: String,
        cwd: String,
        exit_status: i32,
        shell: String,
    ) -> Self {
        Self {
            session_id,
            command: command.trim().to_string(),
            cwd,
            exit_status,
            timestamp: Utc::now(),
            shell,
        }
    }
}
