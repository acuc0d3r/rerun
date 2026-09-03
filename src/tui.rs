use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Terminal,
};
use std::io;
use crate::db::WorkflowRecord;

struct TerminalCleanup;

impl Drop for TerminalCleanup {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    }
}

pub struct TuiApp {
    workflows: Vec<WorkflowRecord>,
    state: ListState,
    should_quit: bool,
    selected_to_run: Option<WorkflowRecord>,
}

impl TuiApp {
    pub fn new(workflows: Vec<WorkflowRecord>) -> Self {
        let mut state = ListState::default();
        if !workflows.is_empty() {
            state.select(Some(0));
        }
        Self {
            workflows,
            state,
            should_quit: false,
            selected_to_run: None,
        }
    }

    pub fn next(&mut self) {
        if self.workflows.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.workflows.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn previous(&mut self) {
        if self.workflows.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.workflows.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn run_selected(&mut self) {
        if let Some(i) = self.state.selected() {
            if let Some(wf) = self.workflows.get(i) {
                self.selected_to_run = Some(wf.clone());
                self.should_quit = true;
            }
        }
    }
}

pub fn run_tui(workflows: Vec<WorkflowRecord>, project_name: &str) -> Result<Option<WorkflowRecord>> {
    enable_raw_mode()?;
    let _cleanup = TerminalCleanup;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = TuiApp::new(workflows);

    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(10),
                    Constraint::Length(3),
                ])
                .split(f.area());

            let header = Paragraph::new(format!(" rerun (rr) - Project: {}", project_name))
                .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
                .block(Block::default().borders(Borders::ALL).title("Workflows"));
            f.render_widget(header, chunks[0]);

            let main_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
                .split(chunks[1]);

            let items: Vec<ListItem> = app
                .workflows
                .iter()
                .map(|wf| {
                    let shortcut = format!("[{}] ", wf.shortcut);
                    let title = &wf.name;
                    let freq = format!(" ({}x)", wf.frequency);
                    let line = Line::from(vec![
                        Span::styled(shortcut, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                        Span::raw(title),
                        Span::styled(freq, Style::default().fg(Color::DarkGray)),
                    ]);
                    ListItem::new(line)
                })
                .collect();

            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title("Available Workflows"))
                .highlight_style(
                    Style::default()
                        .bg(Color::Blue)
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("▶ ");
            f.render_stateful_widget(list, main_chunks[0], &mut app.state);

            let detail_text = if let Some(idx) = app.state.selected() {
                if let Some(wf) = app.workflows.get(idx) {
                    let cmds: Vec<String> = serde_json::from_str(&wf.commands_json).unwrap_or_default();
                    let mut lines = vec![
                        Line::from(vec![
                            Span::styled("Name: ", Style::default().add_modifier(Modifier::BOLD)),
                            Span::raw(&wf.name),
                        ]),
                        Line::from(vec![
                            Span::styled("Shortcut: ", Style::default().add_modifier(Modifier::BOLD)),
                            Span::styled(&wf.shortcut, Style::default().fg(Color::Yellow)),
                        ]),
                        Line::from(vec![
                            Span::styled("Frequency: ", Style::default().add_modifier(Modifier::BOLD)),
                            Span::raw(wf.frequency.to_string()),
                        ]),
                        Line::from(""),
                        Line::from(Span::styled("Commands sequence:", Style::default().fg(Color::Green))),
                    ];
                    for (i, c) in cmds.iter().enumerate() {
                        lines.push(Line::from(format!("  {}. {}", i + 1, c)));
                    }
                    lines
                } else {
                    vec![Line::from("No selection")]
                }
            } else {
                vec![Line::from("No workflows discovered yet. Keep using your shell!")]
            };

            let details = Paragraph::new(detail_text)
                .block(Block::default().borders(Borders::ALL).title("Workflow Details"))
                .wrap(Wrap { trim: true });
            f.render_widget(details, main_chunks[1]);

            let footer = Paragraph::new(" [Enter] Run  [j/k/↑/↓] Navigate  [q/Esc] Exit")
                .style(Style::default().fg(Color::DarkGray))
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(footer, chunks[2]);
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        app.should_quit = true;
                    }
                    KeyCode::Char('j') | KeyCode::Down => {
                        app.next();
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        app.previous();
                    }
                    KeyCode::Enter => {
                        app.run_selected();
                    }
                    _ => {}
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    terminal.show_cursor()?;

    Ok(app.selected_to_run)
}
