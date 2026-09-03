use std::collections::HashMap;
use crate::event::CommandEvent;
use serde::{Deserialize, Serialize};

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
            let noise = command.is_empty()
                || command == "clear"
                || command == "history"
                || command == "rr"
                || command.starts_with("cd ")
                || command.starts_with("export ")
                || command.starts_with("source ")
                || command.starts_with("rr ");
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

        let letters = ["d", "b", "t", "s", "r", "p", "c", "m", "w", "x", "y", "z", "a", "e", "f", "g"];
        let mut workflows = Vec::new();
        let mut used_shortcuts = std::collections::HashSet::new();

        for (i, (seq, count)) in filtered.into_iter().enumerate() {
            let base_shortcut = letters.get(i).copied().unwrap_or("w");
            let mut shortcut = base_shortcut.to_string();
            let mut suffix = 1;
            while used_shortcuts.contains(&shortcut) {
                shortcut = format!("{}{}", base_shortcut, suffix);
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

fn is_subsequence(sub: &[String], parent: &[String]) -> bool {
    if sub.len() >= parent.len() {
        return false;
    }
    parent.windows(sub.len()).any(|w| w == sub)
}
