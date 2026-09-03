# Memory Index

- [Project Overview](#project-overview)
- [Architecture & Design Decisions](#architecture--design-decisions)
- [Storage Paths](#storage-paths)

## Project Overview <!-- project: path:/home/acuc0d3r/Projects/rerun -->
- Tool name: `rr` / `rerun`
- Language: Rust
- Purpose: Automatic CLI workflow learning, shortcut generation, non-intrusive shell integration (Bash first), deterministic n-gram mining, TUI interface, safety validation.

## Architecture & Design Decisions <!-- project: path:/home/acuc0d3r/Projects/rerun -->
- Non-blocking spool logging for shell prompt hooks (zero shell lag).
- Deterministic sequence miner (N-grams 2-6, frequency threshold >= 3, subsumption suppression).
- Trait-based extensible architecture for shell hooks (`ShellIntegration`) and workflow summarizers (`WorkflowSummarizer`).
- Interactive safety check for destructive commands (`rm`, `sudo`, `dd`, `git reset`, etc.).
- Commit after every logical change.

## Storage Paths <!-- project: path:/home/acuc0d3r/Projects/rerun -->
- Database: `$XDG_DATA_HOME/.rerun/rerun.db` (default `~/.local/share/.rerun/rerun.db`)
- Spool directory: `$XDG_DATA_HOME/.rerun/spool` (or `$XDG_RUNTIME_DIR/.rerun/spool`)
