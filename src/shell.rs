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

const HOOK_START_MARKER: &str = "# >>> rr shell hook start >>>";
const HOOK_END_MARKER: &str = "# <<< rr shell hook end <<<";

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
        let bashrc = home.join(".bashrc");

        let content = if bashrc.exists() {
            fs::read_to_string(&bashrc)?
        } else {
            String::new()
        };

        if content.contains(HOOK_START_MARKER) {
            println!("rr bash hook is already installed in {}", bashrc.display());
            return Ok(());
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&bashrc)
            .with_context(|| format!("Failed to open {}", bashrc.display()))?;

        writeln!(file, "\n{}", self.generate_hook_script())?;
        println!("Successfully installed rr hook to {}", bashrc.display());
        Ok(())
    }

    fn uninstall(&self) -> Result<()> {
        let home = dirs::home_dir().context("Could not find home directory")?;
        let bashrc = home.join(".bashrc");
        if !bashrc.exists() {
            return Ok(());
        }

        let content = fs::read_to_string(&bashrc)?;
        if !content.contains(HOOK_START_MARKER) {
            println!("rr bash hook not found in {}", bashrc.display());
            return Ok(());
        }

        let mut new_lines = Vec::new();
        let mut inside_block = false;
        for line in content.lines() {
            if line.contains(HOOK_START_MARKER) {
                inside_block = true;
                continue;
            }
            if line.contains(HOOK_END_MARKER) {
                inside_block = false;
                continue;
            }
            if !inside_block {
                new_lines.push(line);
            }
        }

        fs::write(&bashrc, new_lines.join("\n") + "\n")?;
        println!("Successfully uninstalled rr hook from {}", bashrc.display());
        Ok(())
    }
}
