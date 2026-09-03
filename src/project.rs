use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectInfo {
    pub root: PathBuf,
    pub name: String,
}

pub fn detect_project(start_dir: &Path) -> ProjectInfo {
    let mut curr = if start_dir.is_dir() {
        start_dir.to_path_buf()
    } else {
        start_dir.parent().unwrap_or(start_dir).to_path_buf()
    };

    let markers = [
        ".git",
        "Cargo.toml",
        "package.json",
        "pyproject.toml",
        "go.mod",
        "Makefile",
        "CMakeLists.txt",
    ];

    loop {
        for marker in markers {
            if curr.join(marker).exists() {
                let name = curr
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("root")
                    .to_string();
                return ProjectInfo { root: curr, name };
            }
        }
        if let Some(parent) = curr.parent() {
            curr = parent.to_path_buf();
        } else {
            break;
        }
    }

    let fallback = start_dir.to_path_buf();
    let name = fallback
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("cwd")
        .to_string();
    ProjectInfo {
        root: fallback,
        name,
    }
}
