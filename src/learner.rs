use crate::event::CommandEvent;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveredWorkflow {
    pub commands: Vec<String>,
    pub frequency: usize,
    pub shortcut: String,
    pub name: String,
}

pub trait WorkflowSummarizer {
    fn summarize(&self, commands: &[String]) -> String;
}

pub struct DefaultSummarizer;

impl WorkflowSummarizer for DefaultSummarizer {
    fn summarize(&self, commands: &[String]) -> String {
        if commands.is_empty() {
            return "empty".to_string();
        }
        let first_words: Vec<&str> = commands
            .iter()
            .map(|c| c.split_whitespace().next().unwrap_or(c.as_str()))
            .collect();
        first_words.join(" & ")
    }
}

pub struct SequenceMiner {
    min_length: usize,
    max_length: usize,
    min_frequency: usize,
    summarizer: Box<dyn WorkflowSummarizer>,
}

impl Default for SequenceMiner {
    fn default() -> Self {
        Self {
            min_length: 2,
            max_length: 5,
            min_frequency: 3,
            summarizer: Box::new(DefaultSummarizer),
        }
    }
}

impl SequenceMiner {
    pub fn new(min_freq: usize) -> Self {
        Self {
            min_frequency: min_freq,
            ..Default::default()
        }
    }

    fn filter_commands(events: &[CommandEvent]) -> Vec<Vec<String>> {
        let mut runs = Vec::new();
        let mut current = Vec::new();
        let mut session_id: Option<&str> = None;
        for event in events {
            if session_id.is_some_and(|id| id != event.session_id) {
                if !current.is_empty() {
                    runs.push(std::mem::take(&mut current));
                }
            }
            session_id = Some(&event.session_id);
            let command = event.command.trim();
            let first_word = command.split_whitespace().next().unwrap_or("");
            let shell_state = matches!(
                first_word,
                "cd" | "export" | "unset" | "source" | "." | "alias" | "unalias" | "set" | "shopt"
            );
            let inspection = matches!(
                first_word,
                "pwd"
                    | "pushd"
                    | "popd"
                    | "dirs"
                    | "ls"
                    | "ll"
                    | "tree"
                    | "which"
                    | "type"
                    | "man"
                    | "vim"
                    | "nvim"
                    | "nano"
                    | "less"
                    | "more"
                    | "jobs"
                    | "fg"
                    | "bg"
                    | "disown"
                    | "exit"
                    | "logout"
            );
            let help_or_version = command
                .split_whitespace()
                .any(|word| matches!(word, "-h" | "--help" | "-V" | "--version"));
            let command_lookup = command.starts_with("command -v ");
            let noise = command.is_empty()
                || command == "clear"
                || command == "reset"
                || command == "history"
                || command == "fc"
                || command == "rr"
                || command.starts_with("rr ")
                || shell_state
                || inspection
                || help_or_version
                || command_lookup;
            if event.exit_status != 0 || noise {
                if !current.is_empty() {
                    runs.push(std::mem::take(&mut current));
                }
            } else {
                current.push(command.to_string());
            }
        }
        if !current.is_empty() {
            runs.push(current);
        }
        runs
    }

    pub fn mine(&self, events: &[CommandEvent]) -> Vec<DiscoveredWorkflow> {
        let mut freq_map: HashMap<Vec<String>, usize> = HashMap::new();
        for clean_cmds in Self::filter_commands(events) {
            if clean_cmds.len() < self.min_length {
                continue;
            }
            for n in self.min_length..=self.max_length {
                if clean_cmds.len() < n {
                    break;
                }
                for window in clean_cmds.windows(n) {
                    let seq = window.to_vec();
                    *freq_map.entry(seq).or_insert(0) += 1;
                }
            }
        }

        // Filter by threshold
        let mut candidates: Vec<(Vec<String>, usize)> = freq_map
            .into_iter()
            .filter(|(_, count)| *count >= self.min_frequency)
            .collect();

        // Sort by length desc, frequency desc
        candidates.sort_by(|a, b| {
            b.0.len()
                .cmp(&a.0.len())
                .then_with(|| b.1.cmp(&a.1))
                .then_with(|| a.0.cmp(&b.0))
        });

        // Subsumption suppression: remove sub-sequences if a longer sequence covers it with same frequency
        let mut filtered: Vec<(Vec<String>, usize)> = Vec::new();
        for (seq, count) in candidates {
            let dominated = filtered.iter().any(|(longer_seq, longer_count)| {
                longer_count >= &count && is_subsequence(&seq, longer_seq)
            });
            if !dominated {
                filtered.push((seq, count));
            }
        }

        let mut workflows = Vec::new();
        let mut used_shortcuts = std::collections::HashSet::new();

        for (seq, count) in filtered {
            let candidates = shortcut_candidates(&seq);
            let primary = candidates
                .first()
                .cloned()
                .unwrap_or_else(|| "w".to_string());
            let mut shortcut = candidates
                .iter()
                .find(|candidate| !used_shortcuts.contains(*candidate))
                .cloned()
                .unwrap_or_else(|| primary.clone());
            let mut suffix = 2;
            while used_shortcuts.contains(&shortcut) {
                shortcut = format!("{}{}", primary, suffix);
                suffix += 1;
            }
            used_shortcuts.insert(shortcut.clone());

            let name = self.summarizer.summarize(&seq);
            workflows.push(DiscoveredWorkflow {
                commands: seq,
                frequency: count,
                shortcut,
                name,
            });
        }

        workflows
    }
}

fn shortcut_candidates(commands: &[String]) -> Vec<String> {
    let mut candidates = Vec::new();
    let has_git_pull = commands
        .iter()
        .any(|command| is_git_command(command, "pull"));
    let has_git_push = commands
        .iter()
        .any(|command| is_git_command(command, "push"));

    if has_git_pull && has_git_push {
        candidates.push("gp".to_string());
    }

    for command in commands {
        if let Some(mnemonic) = command_mnemonic(command) {
            push_unique(&mut candidates, mnemonic);
        }
    }

    for command in commands {
        if let Some(verb) = command_verb_mnemonic(command) {
            push_unique(&mut candidates, verb);
        }
    }

    if let Some(command) = dominant_command(commands) {
        if let Some(pair) = command_pair_mnemonic(command) {
            push_unique(&mut candidates, pair);
        }
        if let Some(fallback) = fallback_mnemonic(command) {
            push_unique(&mut candidates, fallback);
        }
    }

    candidates
}

fn command_mnemonic(command: &str) -> Option<String> {
    let words: Vec<&str> = command.split_whitespace().collect();
    let binary = words.first()?.rsplit('/').next()?;

    if binary == "git" {
        let subcommand = words.get(1).copied().unwrap_or("");
        return match subcommand {
            "add" => Some("a".into()),
            "commit" => Some("c".into()),
            "push" => Some("p".into()),
            "pull" => Some("l".into()),
            "status" => Some("s".into()),
            "diff" => Some("d".into()),
            "fetch" => Some("f".into()),
            "stash" => Some("t".into()),
            _ => None,
        };
    }

    let subcommands = words.iter().skip(1).filter(|word| !word.starts_with('-'));
    if binary == "pytest"
        || binary == "jest"
        || subcommands
            .clone()
            .any(|word| matches!(*word, "test" | "t"))
    {
        return Some("t".into());
    }
    if binary == "make"
        || binary == "webpack"
        || subcommands
            .clone()
            .any(|word| matches!(*word, "build" | "b"))
    {
        return Some("b".into());
    }
    for subcommand in subcommands {
        match *subcommand {
            "deploy" | "release" | "publish" => return Some("d".into()),
            "push" => return Some("p".into()),
            "run" => return Some("r".into()),
            "start" | "serve" | "dev" => return Some("s".into()),
            "lint" | "clippy" => return Some("l".into()),
            "fmt" | "prettier" => return Some("f".into()),
            _ => {}
        }
    }

    None
}

fn fallback_mnemonic(command: &str) -> Option<String> {
    command
        .split_whitespace()
        .next()?
        .rsplit('/')
        .next()?
        .chars()
        .next()
        .map(|character| character.to_ascii_lowercase().to_string())
}

fn command_verb_mnemonic(command: &str) -> Option<String> {
    let words: Vec<&str> = command.split_whitespace().collect();
    let verb = words
        .iter()
        .skip(1)
        .find(|word| !word.starts_with('-'))
        .copied()?;
    let mnemonic = match verb {
        "test" | "t" | "pytest" | "jest" => "t".to_string(),
        "build" | "b" | "make" => "b".to_string(),
        "deploy" | "release" | "publish" => "d".to_string(),
        "run" => "r".to_string(),
        "start" | "serve" | "dev" => "s".to_string(),
        "lint" | "clippy" => "l".to_string(),
        "fmt" | "prettier" => "f".to_string(),
        _ => verb.chars().next()?.to_ascii_lowercase().to_string(),
    };
    Some(mnemonic)
}

fn command_pair_mnemonic(command: &str) -> Option<String> {
    let words: Vec<&str> = command.split_whitespace().collect();
    let binary = words.first()?.rsplit('/').next()?.chars().next()?;
    let verb = words
        .iter()
        .skip(1)
        .find(|word| !word.starts_with('-'))?
        .chars()
        .next()?;
    Some(format!(
        "{}{}",
        binary.to_ascii_lowercase(),
        verb.to_ascii_lowercase()
    ))
}

fn dominant_command<'a>(commands: &'a [String]) -> Option<&'a str> {
    commands
        .iter()
        .max_by_key(|command| {
            let binary = command.split_whitespace().next().unwrap_or("");
            commands
                .iter()
                .filter(|other| other.split_whitespace().next().unwrap_or("") == binary)
                .count()
        })
        .map(String::as_str)
}

fn is_git_command(command: &str, subcommand: &str) -> bool {
    let mut words = command.split_whitespace();
    words.next() == Some("git") && words.next() == Some(subcommand)
}

fn push_unique(candidates: &mut Vec<String>, candidate: String) {
    if !candidates.contains(&candidate) {
        candidates.push(candidate);
    }
}

fn is_subsequence(sub: &[String], parent: &[String]) -> bool {
    if sub.len() >= parent.len() {
        return false;
    }
    parent.windows(sub.len()).any(|w| w == sub)
}
