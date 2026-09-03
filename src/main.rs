use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use rerun::config::Paths;
use rerun::db::Database;
use rerun::event::CommandEvent;
use rerun::learner::SequenceMiner;
use rerun::project::detect_project;
use rerun::safety::SafetyChecker;
use rerun::shell::{BashIntegration, ShellIntegration};
use rerun::tui::run_tui;
use std::process::Command;

#[derive(Parser, Debug)]
#[command(name = "rr", version, about = "Learn repetitive command workflows and replay with short shortcuts", long_about = None)]
struct Cli {
    /// Shortcut to execute directly (e.g. `rr d`)
    shortcut: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Record a shell command event (called by shell hooks)
    Record {
        #[arg(long)]
        session: String,
        #[arg(long)]
        status: i32,
        #[arg(long)]
        cmd: String,
        #[arg(long, default_value = "bash")]
        shell: String,
    },
    /// Process recorded history and discover workflows
    Sync,
    /// List discovered workflows for current project
    List,
    /// Show recent project command history
    History {
        #[arg(short, long, default_value_t = 20)]
        limit: usize,
    },
    /// Show statistics
    Stats,
    /// Install shell hook
    Install {
        #[arg(default_value = "bash")]
        shell: String,
    },
    /// Uninstall shell hook
    Uninstall {
        #[arg(default_value = "bash")]
        shell: String,
    },
}

fn execute_commands(commands: &[String]) -> Result<bool> {
    if !SafetyChecker::prompt_confirmation(commands) {
        println!("Execution cancelled.");
        return Ok(false);
    }

    println!(
        "\x1b[1;36m▶ Running workflow ({} commands):\x1b[0m",
        commands.len()
    );
    for (i, cmd) in commands.iter().enumerate() {
        println!("\x1b[1;34m[{}/{}] $ {}\x1b[0m", i + 1, commands.len(), cmd);
        let status = Command::new("bash")
            .arg("-c")
            .arg(cmd)
            .status()
            .with_context(|| format!("Failed to execute command: {}", cmd))?;

        if !status.success() {
            eprintln!(
                "\x1b[1;31m✖ Command failed with exit code: {:?}\x1b[0m",
                status.code()
            );
            return Ok(false);
        }
    }
    println!("\x1b[1;32m✔ Workflow completed successfully!\x1b[0m");
    Ok(true)
}

fn sync_project_workflows(db: &Database, project_root: &str) -> Result<()> {
    let events = db.get_recent_events_for_project(project_root, 1000)?;
    let miner = SequenceMiner::default();
    let workflows = miner.mine(&events);
    let shortcuts: Vec<String> = workflows.iter().map(|wf| wf.shortcut.clone()).collect();
    db.remove_unpinned_workflows_not_in(project_root, &shortcuts)?;

    for wf in workflows {
        let commands_json = serde_json::to_string(&wf.commands)?;
        db.save_workflow(
            project_root,
            &wf.shortcut,
            &wf.name,
            &commands_json,
            wf.frequency as i32,
        )?;
    }
    Ok(())
}

fn main() -> Result<()> {
    let paths = Paths::resolve();
    paths.ensure_dirs()?;
    let db = Database::open(&paths.db_file)?;

    let cli = Cli::parse();
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let project = detect_project(&cwd);
    let project_root = project.root.to_string_lossy().to_string();

    match cli.command {
        Some(Commands::Record {
            session,
            status,
            cmd,
            shell,
        }) => {
            let event = CommandEvent::new(
                session,
                cmd,
                cwd.to_string_lossy().to_string(),
                status,
                shell,
            );
            db.insert_event(&event, &project_root)?;
            // Periodic sync (every time or light pass)
            let _ = sync_project_workflows(&db, &project_root);
            return Ok(());
        }
        Some(Commands::Sync) => {
            sync_project_workflows(&db, &project_root)?;
            println!("Workflows updated for project: {}", project.name);
            return Ok(());
        }
        Some(Commands::List) => {
            sync_project_workflows(&db, &project_root)?;
            let list = db.get_workflows_for_project(&project_root)?;
            if list.is_empty() {
                println!("No workflows discovered yet for project: {}", project.name);
                return Ok(());
            }
            println!("Workflows for {}:", project.name);
            for wf in list {
                let cmds: Vec<String> = serde_json::from_str(&wf.commands_json)
                    .with_context(|| format!("Invalid commands for workflow '{}'", wf.shortcut))?;
                println!(
                    "  \x1b[1;33mrr {:<3}\x1b[0m {:<20} ({}x) -> {}",
                    wf.shortcut,
                    wf.name,
                    wf.frequency,
                    cmds.join(" && ")
                );
            }
            return Ok(());
        }
        Some(Commands::History { limit }) => {
            let events = db.get_recent_events_for_project(&project_root, limit)?;
            println!("Recent command history for {}:", project.name);
            for e in events {
                println!("  [{}] {}", e.exit_status, e.command);
            }
            return Ok(());
        }
        Some(Commands::Stats) => {
            let (events, workflows, projects) = db.total_stats()?;
            println!("rerun statistics:");
            println!("  Total recorded events: {}", events);
            println!("  Total learned workflows: {}", workflows);
            println!("  Total tracked projects: {}", projects);
            return Ok(());
        }
        Some(Commands::Install { shell }) => {
            if shell == "bash" {
                BashIntegration.install()?;
            } else {
                eprintln!("Unsupported shell: {}. Supported: bash", shell);
            }
            return Ok(());
        }
        Some(Commands::Uninstall { shell }) => {
            if shell == "bash" {
                BashIntegration.uninstall()?;
            } else {
                eprintln!("Unsupported shell: {}. Supported: bash", shell);
            }
            return Ok(());
        }
        None => {}
    }

    if let Some(shortcut) = cli.shortcut {
        sync_project_workflows(&db, &project_root)?;
        if let Some(wf) = db.find_workflow(&project_root, &shortcut)? {
            let cmds: Vec<String> = serde_json::from_str(&wf.commands_json)?;
            let success = execute_commands(&cmds)?;
            db.record_execution(wf.id, success)?;
        } else {
            eprintln!("\x1b[1;31mUnknown shortcut '{}' for project '{}'. Run 'rr list' to see available.\x1b[0m", shortcut, project.name);
        }
        return Ok(());
    }

    // Default TUI interactive mode
    sync_project_workflows(&db, &project_root)?;
    let workflows = db.get_workflows_for_project(&project_root)?;
    if let Some(wf) = run_tui(workflows, &project.name)? {
        let cmds: Vec<String> = serde_json::from_str(&wf.commands_json)?;
        let success = execute_commands(&cmds)?;
        db.record_execution(wf.id, success)?;
    }

    Ok(())
}
