use directories::ProjectDirs;
use std::path::PathBuf;

pub struct Paths {
    pub data_dir: PathBuf,
    pub db_file: PathBuf,
    pub spool_dir: PathBuf,
}

impl Paths {
    pub fn resolve() -> Self {
        let base_dir = if let Ok(data_home) = std::env::var("XDG_DATA_HOME") {
            PathBuf::from(data_home).join(".rerun")
        } else if let Some(proj_dirs) = ProjectDirs::from("", "", "rerun") {
            proj_dirs.data_dir().to_path_buf()
        } else {
            dirs::home_dir()
                .map(|h| h.join(".local").join("share").join(".rerun"))
                .unwrap_or_else(|| PathBuf::from(".rerun"))
        };

        let spool_dir = if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
            PathBuf::from(runtime_dir).join(".rerun").join("spool")
        } else {
            base_dir.join("spool")
        };

        let db_file = base_dir.join("rerun.db");

        Self {
            data_dir: base_dir,
            db_file,
            spool_dir,
        }
    }

    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.data_dir)?;
        std::fs::create_dir_all(&self.spool_dir)?;
        Ok(())
    }
}
