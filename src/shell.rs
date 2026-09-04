use anyhow::{Context, Result};
use std::fs::{self, OpenOptions};
use std::io::Write;

pub trait ShellIntegration {
    fn shell_name(&self) -> &'static str;
    fn generate_hook_script(&self) -> String;
    fn install(&self) -> Result<()>;
    fn uninstall(&self) -> Result<()>;
}

pub struct BashIntegration;
pub struct ZshIntegration;
pub struct FishIntegration;

const HOOK_START_MARKER: &str = "# >>> rr shell hook start >>>";
const HOOK_END_MARKER: &str = "# <<< rr shell hook end <<<";
const ZSH_HOOK_START_MARKER: &str = "# >>> rr zsh hook start >>>";
const ZSH_HOOK_END_MARKER: &str = "# <<< rr zsh hook end <<<";
const FISH_HOOK_START_MARKER: &str = "# >>> rr fish hook start >>>";
const FISH_HOOK_END_MARKER: &str = "# <<< rr fish hook end <<<";

fn install_script(path: &std::path::Path, script: &str, marker: &str) -> Result<()> {
    let content = if path.exists() {
        fs::read_to_string(path)?
    } else {
        String::new()
    };
    if content.contains(marker) {
        println!("rr shell hook is already installed in {}", path.display());
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("Failed to open {}", path.display()))?;
    writeln!(file, "\n{}", script)?;
    println!("Successfully installed rr hook to {}", path.display());
    Ok(())
}

fn uninstall_script(path: &std::path::Path, start: &str, end: &str) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let content = fs::read_to_string(path)?;
    if !content.contains(start) {
        println!("rr shell hook not found in {}", path.display());
        return Ok(());
    }
    let mut new_lines = Vec::new();
    let mut inside_block = false;
    for line in content.lines() {
        if line.contains(start) {
            inside_block = true;
            continue;
        }
        if line.contains(end) {
            inside_block = false;
            continue;
        }
        if !inside_block {
            new_lines.push(line);
        }
    }
    fs::write(path, new_lines.join("\n") + "\n")?;
    println!("Successfully uninstalled rr hook from {}", path.display());
    Ok(())
}

impl ShellIntegration for BashIntegration {
    fn shell_name(&self) -> &'static str {
        "bash"
    }

    fn generate_hook_script(&self) -> String {
        format!(
            r#"{start}
__rr_precmd() {{
    local last_exit="$?"
    local last_cmd="$(HISTTIMEFORMAT= history 1 | sed 's/^[ ]*[0-9]*[ ]*//')"
    if [ -n "$last_cmd" ] && [ "$last_cmd" != "$__rr_last_cmd" ]; then
        __rr_last_cmd="$last_cmd"
        (rr record --session "$$"-bash --status "$last_exit" --cmd "$last_cmd" >/dev/null 2>&1 &)
    fi
}}

if [[ ! "$PROMPT_COMMAND" =~ "__rr_precmd" ]]; then
    PROMPT_COMMAND="__rr_precmd${{PROMPT_COMMAND:+; $PROMPT_COMMAND}}"
fi
{end}
"#,
            start = HOOK_START_MARKER,
            end = HOOK_END_MARKER
        )
    }

    fn install(&self) -> Result<()> {
        let home = dirs::home_dir().context("Could not find home directory")?;
        install_script(
            &home.join(".bashrc"),
            &self.generate_hook_script(),
            HOOK_START_MARKER,
        )
    }

    fn uninstall(&self) -> Result<()> {
        let home = dirs::home_dir().context("Could not find home directory")?;
        uninstall_script(&home.join(".bashrc"), HOOK_START_MARKER, HOOK_END_MARKER)
    }
}

impl ShellIntegration for ZshIntegration {
    fn shell_name(&self) -> &'static str {
        "zsh"
    }

    fn generate_hook_script(&self) -> String {
        format!(
            r#"{start}
typeset -g __rr_last_cmd=""
__rr_preexec() {{ __rr_last_cmd="$1" }}
__rr_precmd() {{
    local last_exit="$?"
    if [[ -n "$__rr_last_cmd" ]]; then
        (rr record --session "$$"-zsh --status "$last_exit" --cmd "$__rr_last_cmd" --shell zsh >/dev/null 2>&1 &)
        __rr_last_cmd=""
    fi
}}
preexec_functions+=(__rr_preexec)
precmd_functions+=(__rr_precmd)
{end}
"#,
            start = ZSH_HOOK_START_MARKER,
            end = ZSH_HOOK_END_MARKER
        )
    }

    fn install(&self) -> Result<()> {
        let home = dirs::home_dir().context("Could not find home directory")?;
        install_script(
            &home.join(".zshrc"),
            &self.generate_hook_script(),
            ZSH_HOOK_START_MARKER,
        )
    }

    fn uninstall(&self) -> Result<()> {
        let home = dirs::home_dir().context("Could not find home directory")?;
        uninstall_script(
            &home.join(".zshrc"),
            ZSH_HOOK_START_MARKER,
            ZSH_HOOK_END_MARKER,
        )
    }
}

impl ShellIntegration for FishIntegration {
    fn shell_name(&self) -> &'static str {
        "fish"
    }

    fn generate_hook_script(&self) -> String {
        format!(
            r#"{start}
function __rr_postexec --on-event fish_postexec
    set -l last_exit $status
    set -l last_cmd $argv[1]
    if test -n "$last_cmd"
        rr record --session "$fish_pid"-fish --status $last_exit --cmd "$last_cmd" --shell fish >/dev/null 2>&1 &
    end
end
{end}
"#,
            start = FISH_HOOK_START_MARKER,
            end = FISH_HOOK_END_MARKER
        )
    }

    fn install(&self) -> Result<()> {
        let config = dirs::config_dir()
            .context("Could not find config directory")?
            .join("fish")
            .join("config.fish");
        install_script(
            &config,
            &self.generate_hook_script(),
            FISH_HOOK_START_MARKER,
        )
    }

    fn uninstall(&self) -> Result<()> {
        let config = dirs::config_dir()
            .context("Could not find config directory")?
            .join("fish")
            .join("config.fish");
        uninstall_script(&config, FISH_HOOK_START_MARKER, FISH_HOOK_END_MARKER)
    }
}
