# Copilot instructions

## Commands

- `cargo test` runs the full test suite.
- `cargo test --test integration_tests` runs integration tests.
- `cargo test --test integration_tests test_miner_discovers_frequent_sequences`
  runs one integration test by name.
- `cargo build --release` builds the release binary.
- `cargo fmt --check` checks Rust formatting.
- `cargo clippy --all-targets --all-features -- -D warnings` runs lint checks.

The executable is named `rr`; the crate and library are named `rerun`.

## Architecture

- `src/main.rs` owns the CLI (`clap`), project-scoped command dispatch,
  workflow synchronization, shortcut execution, and TUI handoff.
- `src/shell.rs` provides the `ShellIntegration` abstraction. Bash installs a
  marker-protected `PROMPT_COMMAND` hook in `~/.bashrc`; the hook records the
  previous command asynchronously with `rr record`.
- `src/event.rs` defines serialized command events. Events include session,
  command, working directory, exit status, timestamp, and shell.
- `src/db.rs` owns SQLite initialization and all persistence. The database uses
  WAL mode and stores command events, learned workflows, and executions.
- `src/project.rs` finds the nearest project root by checking common repository
  and build markers. Events and workflows are isolated by that root.
- `src/learner.rs` mines successful contiguous command runs within one shell
  session. It counts n-grams of length 2 through 5, requires frequency 3 by
  default, suppresses shorter sequences covered by longer ones, and assigns
  deterministic shortcuts.
- `src/safety.rs` gates workflow execution with an interactive confirmation for
  destructive commands or shell composition.
- `src/tui.rs` displays project workflows and returns the selected
  `WorkflowRecord`; execution remains in `main.rs`.

Runtime data is stored under `$XDG_DATA_HOME/.rerun/rerun.db`, falling back to
the platform data directory or `~/.local/share/.rerun/rerun.db`. Runtime spool
data uses `$XDG_RUNTIME_DIR/.rerun/spool` when that variable is set.

## Repository conventions

- Keep project behavior project-scoped: pass the detected project root through
  database queries and workflow synchronization.
- Preserve event ordering when mining. The database returns recent events in
  chronological order after applying its limit.
- Treat failed commands, shell noise (`cd`, `export`, `source`, `clear`,
  `history`, and `rr`), and session changes as workflow boundaries.
- Keep shortcut generation deterministic; stable ordering is required so sync
  does not randomly remap shortcuts.
- Use parameterized SQLite queries. Workflow command sequences are stored as
  JSON and must be deserialized with an explicit error, not silently replaced
  with an empty sequence.
- Preserve pinned workflows during synchronization; only stale unpinned
  workflows may be removed.
- Shell hook installation and removal must remain marker-bounded and avoid
  modifying unrelated `.bashrc` content.
- Execute learned commands through Bash, matching the installed hook's shell,
  and retain the safety confirmation before running a workflow.
- Keep terminal cleanup reliable: raw mode and the alternate screen must be
  restored even when TUI operations return an error.
- Extend shell support through `ShellIntegration` and workflow naming through
  `WorkflowSummarizer` rather than adding shell-specific logic to CLI dispatch.
