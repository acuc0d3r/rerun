# rerun

`rerun` (`rr`) learns repeated shell command workflows, assigns shortcuts,
and replays them from the current project.

It records successful Bash commands through a non-blocking shell hook, mines
repeated contiguous sequences, and stores workflows in SQLite. Destructive or
compound commands require confirmation before execution.

## Install

Build the release binary:

```bash
cargo build --release
```

Install it somewhere on `PATH`, for example:

```bash
install -Dm755 target/release/rr ~/.local/bin/rr
```

Install the Bash hook:

```bash
rr install bash
source ~/.bashrc
```

Remove the hook with:

```bash
rr uninstall bash
```

## Usage

Record commands through the shell hook, then inspect learned workflows:

```bash
rr sync
rr list
```

Run a workflow by shortcut:

```bash
rr t
```

Without a subcommand or shortcut, `rr` opens the interactive terminal UI:

```bash
rr
```

Other commands:

```text
rr history              Show recent commands
rr history --limit 50   Show more commands
rr stats                Show database statistics
rr record ...           Record one command event
```

Run `rr --help` for complete CLI options.

## Workflow learning

The miner:

- Considers n-grams from 2 through 5 commands.
- Requires frequency 3 by default.
- Uses only successful, contiguous commands from one shell session.
- Treats failed commands and shell noise as workflow boundaries.
- Suppresses shorter sequences covered by longer sequences.
- Assigns deterministic mnemonic shortcuts.

Shortcut selection prefers tool intent and Git operations:

```text
test, pytest, jest, cargo test  t
build, make, cargo build       b
deploy, release, publish       d
run, start, serve, dev         r/s
lint, clippy, fmt, prettier    l/f
git add                        a
git commit                     c
git push                       p
git pull                       l
git status                     s
git diff                       d
git fetch                      f
git stash                      t
git pull + git push            gp
```

Collisions use secondary verb mnemonics, two-letter command mnemonics, then
numbered suffixes such as `t2` or `b2`. Unknown tools fall back to the first
letter of the dominant binary.

## Safety

Before running a workflow, `rr` asks for confirmation when it contains
potentially dangerous commands such as:

- `rm`, `rmdir`, `mkfs`, `dd`, `fdisk`, or `parted`
- `sudo`, `chmod`, `chown`, `reboot`, `shutdown`, or `poweroff`
- `git reset --hard`
- Forced Git pushes or destructive `git clean`
- Shell composition, pipelines, command substitution, or redirection to
  `/dev/*`

Answer `y` or `yes` to continue. The default is cancellation.

## Storage

Database path:

```text
$XDG_DATA_HOME/.rerun/rerun.db
```

When `XDG_DATA_HOME` is unset, the platform data directory is used, with
`~/.local/share/.rerun/rerun.db` as the Linux fallback.

Spool path uses `$XDG_RUNTIME_DIR/.rerun/spool` when available; otherwise it
uses the data directory's `spool` subdirectory.

## Development

Run tests:

```bash
cargo test
```

Run one integration test:

```bash
cargo test --test integration_tests test_miner_discovers_frequent_sequences
```

Build release output:

```bash
cargo build --release
```

The library modules are under `src/`; integration coverage is under `tests/`.
