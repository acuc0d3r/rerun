#[cfg(test)]
mod tests {
    use rerun::db::Database;
    use rerun::event::CommandEvent;
    use rerun::learner::SequenceMiner;
    use rerun::safety::SafetyChecker;
    use tempfile::tempdir;

    #[test]
    fn test_miner_discovers_frequent_sequences() {
        let events = vec![
            CommandEvent::new(
                "s1".into(),
                "git status".into(),
                "/tmp".into(),
                0,
                "bash".into(),
            ),
            CommandEvent::new(
                "s1".into(),
                "cargo check".into(),
                "/tmp".into(),
                0,
                "bash".into(),
            ),
            CommandEvent::new(
                "s1".into(),
                "cargo test".into(),
                "/tmp".into(),
                0,
                "bash".into(),
            ),
            CommandEvent::new(
                "s1".into(),
                "git status".into(),
                "/tmp".into(),
                0,
                "bash".into(),
            ),
            CommandEvent::new(
                "s1".into(),
                "cargo check".into(),
                "/tmp".into(),
                0,
                "bash".into(),
            ),
            CommandEvent::new(
                "s1".into(),
                "cargo test".into(),
                "/tmp".into(),
                0,
                "bash".into(),
            ),
            CommandEvent::new(
                "s1".into(),
                "git status".into(),
                "/tmp".into(),
                0,
                "bash".into(),
            ),
            CommandEvent::new(
                "s1".into(),
                "cargo check".into(),
                "/tmp".into(),
                0,
                "bash".into(),
            ),
            CommandEvent::new(
                "s1".into(),
                "cargo test".into(),
                "/tmp".into(),
                0,
                "bash".into(),
            ),
        ];

        let miner = SequenceMiner::new(3);
        let workflows = miner.mine(&events);

        assert!(!workflows.is_empty());
        let top = &workflows[0];
        assert_eq!(
            top.commands,
            vec!["git status", "cargo check", "cargo test"]
        );
        assert_eq!(top.frequency, 3);
    }

    #[test]
    fn test_safety_checker() {
        assert!(SafetyChecker::is_dangerous("rm -rf /tmp/test"));
        assert!(SafetyChecker::is_dangerous("sudo apt update"));
        assert!(SafetyChecker::is_dangerous("git reset --hard HEAD~1"));
        assert!(!SafetyChecker::is_dangerous("cargo test"));
        assert!(!SafetyChecker::is_dangerous("git status"));
    }

    #[test]
    fn test_database_roundtrip() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = Database::open(&db_path).unwrap();

        let event = CommandEvent::new(
            "s1".into(),
            "cargo build".into(),
            "/repo".into(),
            0,
            "bash".into(),
        );
        let id = db.insert_event(&event, "/repo").unwrap();
        assert!(id > 0);

        let events = db.get_recent_events_for_project("/repo", 10).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].command, "cargo build");
    }

    #[test]
    fn test_workflow_metadata_edit_is_persisted() {
        let dir = tempdir().unwrap();
        let db = Database::open(&dir.path().join("test.db")).unwrap();
        db.save_workflow(
            "/repo",
            "a",
            "git & git",
            r#"["git add .","git commit"]"#,
            3,
        )
        .unwrap();

        assert!(db
            .update_workflow_metadata("/repo", "a", Some("c"), Some("Commit changes"))
            .unwrap());
        let workflow = db.find_workflow("/repo", "c").unwrap().unwrap();
        assert_eq!(workflow.name, "Commit changes");
        assert!(workflow.is_pinned);
    }

    #[test]
    fn test_shell_noise_breaks_workflow_runs() {
        let commands = [
            "cargo build",
            "pwd",
            "cargo test",
            "cargo build",
            "ls",
            "cargo test",
            "cargo build",
            "cargo test",
        ];
        let events = commands
            .iter()
            .map(|command| {
                CommandEvent::new(
                    "s1".into(),
                    (*command).into(),
                    "/tmp".into(),
                    0,
                    "bash".into(),
                )
            })
            .collect::<Vec<_>>();

        let workflows = SequenceMiner::new(3).mine(&events);

        assert!(workflows
            .iter()
            .all(|workflow| { workflow.commands != vec!["cargo build", "cargo test"] }));
    }

    #[test]
    fn test_mnemonic_shortcuts() {
        let commands = [
            "cargo test",
            "cargo test",
            "clear",
            "cargo build",
            "cargo build",
            "clear",
            "cargo test",
            "cargo test",
            "clear",
            "cargo build",
            "cargo build",
            "clear",
            "cargo test",
            "cargo test",
            "clear",
            "cargo build",
            "cargo build",
        ];
        let events = commands
            .iter()
            .map(|command| {
                CommandEvent::new(
                    "s1".into(),
                    (*command).into(),
                    "/tmp".into(),
                    0,
                    "bash".into(),
                )
            })
            .collect::<Vec<_>>();

        let workflows = SequenceMiner::new(3).mine(&events);

        assert!(workflows.iter().any(|workflow| workflow.shortcut == "t"));
        assert!(workflows.iter().any(|workflow| workflow.shortcut == "b"));
    }

    #[test]
    fn test_git_pull_push_pair_prefers_pair_mnemonic() {
        let commands = ["git pull", "git push"];
        let events = commands
            .iter()
            .cycle()
            .take(12)
            .map(|command| {
                CommandEvent::new(
                    "s1".into(),
                    (*command).into(),
                    "/tmp".into(),
                    0,
                    "bash".into(),
                )
            })
            .collect::<Vec<_>>();

        let workflows = SequenceMiner::new(3).mine(&events);

        assert!(workflows.iter().any(|workflow| workflow.shortcut == "gp"));
    }

    #[test]
    fn test_git_add_commit_workflow_prefers_commit_mnemonic() {
        let commands = ["git add .", "git commit"];
        let events = commands
            .iter()
            .cycle()
            .take(12)
            .map(|command| {
                CommandEvent::new(
                    "s1".into(),
                    (*command).into(),
                    "/tmp".into(),
                    0,
                    "bash".into(),
                )
            })
            .collect::<Vec<_>>();

        let workflows = SequenceMiner::new(3).mine(&events);

        assert!(workflows.iter().any(|workflow| workflow.shortcut == "c"));
    }

    #[test]
    fn test_tool_and_git_mnemonics_match_workflow_intent() {
        let commands = [
            "git status",
            "cargo test",
            "clear",
            "git status",
            "cargo test",
            "clear",
            "git status",
            "cargo test",
            "clear",
            "git diff",
            "cargo build",
            "clear",
            "git diff",
            "cargo build",
            "clear",
            "git diff",
            "cargo build",
        ];
        let events = commands
            .iter()
            .map(|command| {
                CommandEvent::new(
                    "s1".into(),
                    (*command).into(),
                    "/tmp".into(),
                    0,
                    "bash".into(),
                )
            })
            .collect::<Vec<_>>();

        let workflows = SequenceMiner::new(3).mine(&events);
        let shortcut_for = |commands: &[&str]| {
            workflows
                .iter()
                .find(|workflow| workflow.commands == commands)
                .map(|workflow| workflow.shortcut.as_str())
        };

        assert_eq!(shortcut_for(&["git status", "cargo test"]), Some("s"));
        assert_eq!(shortcut_for(&["git diff", "cargo build"]), Some("d"));
    }
}
