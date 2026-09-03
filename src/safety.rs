use std::io::{self, Write};

pub struct SafetyChecker;

impl SafetyChecker {
    pub fn is_dangerous(command: &str) -> bool {
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return false;
        }

        let cmd = parts[0].trim_start_matches(|c| c == '(' || c == '{');
        let dangerous_bins = [
            "rm", "rmdir", "mkfs", "dd", "fdisk", "parted", "chmod", "chown", "reboot", "shutdown", "poweroff",
        ];

        if dangerous_bins.contains(&cmd) {
            return true;
        }

        if cmd == "sudo" || parts.iter().any(|part| dangerous_bins.contains(part) || *part == "sudo") {
            return true;
        }

        if cmd == "git" {
            if parts.contains(&"reset") && parts.contains(&"--hard") {
                return true;
            }
            if parts.contains(&"clean") && (parts.contains(&"-f") || parts.contains(&"-fd") || parts.contains(&"-xdf")) {
                return true;
            }
            if parts.contains(&"push") && (parts.contains(&"--force") || parts.contains(&"-f")) {
                return true;
            }
        }

        if command.contains(" > /dev/") || command.contains(":(){ :|:& };:")
            || command.contains(';') || command.contains("&&") || command.contains("||")
            || command.contains('|') || command.contains("$(") || command.contains('`')
        {
            return true;
        }

        false
    }

    pub fn prompt_confirmation(commands: &[String]) -> bool {
        let has_danger = commands.iter().any(|c| Self::is_dangerous(c));
        if !has_danger {
            return true;
        }

        eprintln!("\x1b[1;33m[WARNING] Workflow contains potentially dangerous commands:\x1b[0m");
        for cmd in commands {
            if Self::is_dangerous(cmd) {
                eprintln!("  \x1b[1;31m! {}\x1b[0m", cmd);
            } else {
                eprintln!("    {}", cmd);
            }
        }
        eprint!("\x1b[1;33mDo you want to proceed? [y/N]: \x1b[0m");
        io::stdout().flush().unwrap_or(());

        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_ok() {
            let trimmed = input.trim().to_lowercase();
            trimmed == "y" || trimmed == "yes"
        } else {
            false
        }
    }
}
