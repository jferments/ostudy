use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Wrap};

use crate::model::{AppModel, RunStatus};
use crate::parser::TestStatus;
use crate::workflow::WORKFLOWS;

const OSTUDY_PURPLE: Color = Color::Rgb(139, 92, 246);
const DIM: Color = Color::Rgb(142, 154, 175);
const PANEL: Color = Color::Rgb(52, 61, 77);
const SUCCESS: Color = Color::Rgb(74, 222, 128);
const WARNING: Color = Color::Rgb(251, 191, 36);
const FAILURE: Color = Color::Rgb(248, 113, 113);
const CYAN: Color = Color::Rgb(103, 232, 249);

const WORDMARK: [&str; 5] = [
    " ██████  ███████ ████████ ██    ██ ██████  ██    ██ ",
    "██    ██ ██         ██    ██    ██ ██   ██  ██    ██ ",
    "██    ██ ███████    ██    ██    ██ ██   ██   ██████  ",
    "██    ██      ██    ██    ██    ██ ██   ██     ██    ",
    " ██████  ███████    ██     ██████  ██████      ██    ",
];

const RACCOON: [&str; 8] = [
    "        .-\"\"\"-.        ",
    "      .'  _   _  '.      ",
    "     /   (o)_(o)   \\     ",
    "    |   .-`---'-.   |    ",
    "    |  /  .---.  \\  |    ",
    "     \\ |  `---'  | /     ",
    "      '.\\_______/.'      ",
    "        /_/   \\_\\        ",
];

pub fn draw(frame: &mut Frame<'_>, model: &AppModel) {
    let area = frame.area();
    if area.width < 54 || area.height < 16 {
        draw_compact(frame, model, area);
        return;
    }

    let header_height = if area.width >= 84 { 7 } else { 3 };
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_height),
            Constraint::Min(8),
            Constraint::Length(2),
        ])
        .split(area);
    draw_header(frame, areas[0]);
    if model.status == RunStatus::Menu {
        draw_menu(frame, model, areas[1]);
    } else {
        draw_run(frame, model, areas[1]);
    }
    draw_footer(frame, model, areas[2]);
}

fn draw_header(frame: &mut Frame<'_>, area: Rect) {
    if area.width >= 84 {
        let lines = WORDMARK
            .iter()
            .map(|line| Line::from(Span::styled(*line, Style::default().fg(OSTUDY_PURPLE))))
            .chain(std::iter::once(Line::from(Span::styled(
                "developer console • Linux build / test / run",
                Style::default().fg(DIM),
            ))))
            .collect::<Vec<_>>();
        frame.render_widget(Paragraph::new(lines), area);
    } else {
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled(
                    "OSTUDY",
                    Style::default()
                        .fg(OSTUDY_PURPLE)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(Span::styled("developer console", Style::default().fg(DIM))),
            ]),
            area,
        );
    }
}

fn draw_menu(frame: &mut Frame<'_>, model: &AppModel, area: Rect) {
    if area.width >= 94 {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(29), Constraint::Min(40)])
            .split(area);
        draw_mascot(frame, columns[0]);
        draw_workflows(frame, model, columns[1]);
    } else {
        draw_workflows(frame, model, area);
    }
}

fn draw_mascot(frame: &mut Frame<'_>, area: Rect) {
    let lines = RACCOON
        .iter()
        .map(|line| Line::from(Span::styled(*line, Style::default().fg(DIM))))
        .chain(std::iter::once(Line::from("")))
        .chain(std::iter::once(Line::from(Span::styled(
            " local-first study tools ",
            Style::default().fg(OSTUDY_PURPLE),
        ))))
        .collect::<Vec<_>>();
    let block = Block::default()
        .title(" raccoon ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(PANEL));
    frame.render_widget(Paragraph::new(lines).alignment(Alignment::Center).block(block), area);
}

fn draw_workflows(frame: &mut Frame<'_>, model: &AppModel, area: Rect) {
    let items = WORKFLOWS
        .iter()
        .enumerate()
        .map(|(index, workflow)| {
            let selected = index == model.selected_index;
            let pointer = if selected { "▶" } else { " " };
            let title_style = if selected {
                Style::default()
                    .fg(OSTUDY_PURPLE)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(vec![
                Line::from(vec![
                    Span::styled(format!("{pointer} "), title_style),
                    Span::styled(workflow.title, title_style),
                    Span::styled(format!("  {}", workflow.shell_preview()), Style::default().fg(DIM)),
                ]),
                Line::from(vec![
                    Span::raw("   "),
                    Span::styled(workflow.description, Style::default().fg(DIM)),
                ]),
            ])
        })
        .collect::<Vec<_>>();
    let block = Block::default()
        .title(" workflows ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(PANEL));
    frame.render_widget(List::new(items).block(block), area);
}

fn draw_run(frame: &mut Frame<'_>, model: &AppModel, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(4),
            Constraint::Min(6),
        ])
        .split(area);
    draw_session(frame, model, rows[0]);
    draw_progress(frame, model, rows[1]);

    if rows[2].width >= 94 {
        let lower = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(44), Constraint::Percentage(56)])
            .split(rows[2]);
        draw_tests(frame, model, lower[0]);
        draw_output(frame, model, lower[1]);
    } else {
        let lower = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(48), Constraint::Percentage(52)])
            .split(rows[2]);
        draw_tests(frame, model, lower[0]);
        draw_output(frame, model, lower[1]);
    }
}

fn draw_session(frame: &mut Frame<'_>, model: &AppModel, area: Rect) {
    let workflow = model.workflow;
    let title = workflow.map_or("workflow", |workflow| workflow.title);
    let command = workflow.map_or("", |workflow| workflow.shell_preview());
    let pid = model
        .child_pid
        .map(|pid| pid.to_string())
        .unwrap_or_else(|| "—".to_owned());
    let log = model
        .log_path
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "—".to_owned());

    let lines = vec![
        Line::from(vec![
            Span::styled(title, Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("  "),
            status_span(model.status),
        ]),
        Line::from(vec![
            Span::styled("command  ", Style::default().fg(DIM)),
            Span::raw(command),
            Span::styled("    elapsed  ", Style::default().fg(DIM)),
            Span::raw(format_duration(model.elapsed())),
            Span::styled("    pid  ", Style::default().fg(DIM)),
            Span::raw(pid),
        ]),
        Line::from(vec![
            Span::styled("log      ", Style::default().fg(DIM)),
            Span::raw(truncate(&log, area.width.saturating_sub(12) as usize)),
        ]),
    ];
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .title(" session ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(PANEL)),
        ),
        area,
    );
}

fn draw_progress(frame: &mut Frame<'_>, model: &AppModel, area: Rect) {
    let total = model.workflow.and_then(|workflow| workflow.total_steps);
    let stage = if let Some(total) = total {
        format!(
            " {}  [{}/{}] ",
            model.current_stage,
            model.stage_index.min(total),
            total
        )
    } else {
        format!(" {} ", model.current_stage)
    };
    let progress_style = match model.status {
        RunStatus::Passed => Style::default().fg(SUCCESS),
        RunStatus::Failed | RunStatus::Cancelled => Style::default().fg(FAILURE),
        _ => Style::default().fg(OSTUDY_PURPLE),
    };
    let label = match model.status {
        RunStatus::Passed => "complete".to_owned(),
        RunStatus::Failed => "failed".to_owned(),
        RunStatus::Cancelled => "cancelled".to_owned(),
        RunStatus::Running => model
            .nested_stage
            .clone()
            .unwrap_or_else(|| "working".to_owned()),
        RunStatus::Menu => String::new(),
    };
    let gauge = Gauge::default()
        .block(
            Block::default()
                .title(stage)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(PANEL)),
        )
        .gauge_style(progress_style.add_modifier(Modifier::BOLD))
        .ratio(model.progress_ratio().unwrap_or(0.0))
        .label(label);
    frame.render_widget(gauge, area);
}

fn draw_tests(frame: &mut Frame<'_>, model: &AppModel, area: Rect) {
    let mut lines = vec![
        Line::from(vec![
            Span::styled(format!("✓ {}", model.passed), Style::default().fg(SUCCESS)),
            Span::raw("   "),
            Span::styled(format!("✗ {}", model.failed), Style::default().fg(FAILURE)),
            Span::raw("   "),
            Span::styled(format!("○ {}", model.skipped), Style::default().fg(WARNING)),
            Span::raw("   "),
            Span::styled(format!("Σ {}", model.total_tests()), Style::default().fg(DIM)),
        ]),
        Line::from(test_bar(model, area.width.saturating_sub(6) as usize)),
    ];
    if model.recent_tests.is_empty() {
        lines.push(Line::from(Span::styled(
            "No individual test results observed yet.",
            Style::default().fg(DIM),
        )));
    } else {
        lines.extend(model.recent_tests.iter().rev().take(6).map(|test| {
            let (symbol, color) = match test.status {
                TestStatus::Passed => ("✓", SUCCESS),
                TestStatus::Failed => ("✗", FAILURE),
                TestStatus::Skipped => ("○", WARNING),
            };
            Line::from(vec![
                Span::styled(format!("{symbol} "), Style::default().fg(color)),
                Span::raw(truncate(&test.name, area.width.saturating_sub(10) as usize)),
                Span::styled(
                    if test.count > 1 {
                        format!(" ×{}", test.count)
                    } else {
                        String::new()
                    },
                    Style::default().fg(DIM),
                ),
            ])
        }));
    }

    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: true }).block(
            Block::default()
                .title(" test signal ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(PANEL)),
        ),
        area,
    );
}

fn test_bar(model: &AppModel, available: usize) -> Vec<Span<'static>> {
    let total = model.total_tests();
    if total == 0 {
        return vec![Span::styled("░ no results", Style::default().fg(DIM))];
    }
    let width = available.clamp(8, 48);
    let passed_width = width * model.passed / total;
    let failed_width = width * model.failed / total;
    let skipped_width = width.saturating_sub(passed_width + failed_width);
    vec![
        Span::styled("█".repeat(passed_width), Style::default().fg(SUCCESS)),
        Span::styled("█".repeat(failed_width), Style::default().fg(FAILURE)),
        Span::styled("▒".repeat(skipped_width), Style::default().fg(WARNING)),
    ]
}

fn draw_output(frame: &mut Frame<'_>, model: &AppModel, area: Rect) {
    let use_errors = model.show_errors || model.status == RunStatus::Failed;
    let source = if use_errors {
        &model.errors
    } else {
        &model.recent_output
    };
    let mut lines = source
        .iter()
        .rev()
        .take(area.height.saturating_sub(2) as usize)
        .collect::<Vec<_>>();
    lines.reverse();
    let lines = if lines.is_empty() {
        vec![Line::from(Span::styled(
            if use_errors {
                "No error lines captured."
            } else {
                "Waiting for useful output. Full output is still going to the log."
            },
            Style::default().fg(DIM),
        ))]
    } else {
        lines
            .into_iter()
            .map(|line| Line::from(Span::raw(line.clone())))
            .collect()
    };
    let title = if use_errors {
        " errors — copy/paste ready "
    } else {
        " recent output "
    };
    let border = if use_errors && !model.errors.is_empty() {
        FAILURE
    } else {
        PANEL
    };
    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }).block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border)),
        ),
        area,
    );
}

fn draw_footer(frame: &mut Frame<'_>, model: &AppModel, area: Rect) {
    let hint = match model.status {
        RunStatus::Menu => "↑/↓ or j/k  select    Enter  run    q  quit",
        RunStatus::Running => "e  errors/output    q/Esc  cancel    Ctrl-C  stop",
        RunStatus::Passed | RunStatus::Failed | RunStatus::Cancelled => {
            "e  errors/output    Enter  workflows    q/Esc  quit"
        }
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("● ", Style::default().fg(OSTUDY_PURPLE)),
            Span::styled(hint, Style::default().fg(DIM)),
        ])),
        area,
    );
}

fn draw_compact(frame: &mut Frame<'_>, model: &AppModel, area: Rect) {
    let workflow = model.workflow.map_or("workflow", |workflow| workflow.title);
    let status = match model.status {
        RunStatus::Menu => "Choose a workflow; enlarge the terminal for the full console.",
        RunStatus::Running => "Running — enlarge terminal for progress and logs.",
        RunStatus::Passed => "Passed.",
        RunStatus::Failed => "Failed — error lines will print after exit.",
        RunStatus::Cancelled => "Cancelled.",
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                "OSTUDY",
                Style::default()
                    .fg(OSTUDY_PURPLE)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(workflow),
            Line::from(Span::styled(status, Style::default().fg(CYAN))),
            Line::from(Span::styled("q/Esc quit • Enter select/run", Style::default().fg(DIM))),
        ])
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(PANEL))),
        area,
    );
}

fn status_span(status: RunStatus) -> Span<'static> {
    let (label, color) = match status {
        RunStatus::Menu => ("READY", DIM),
        RunStatus::Running => ("RUNNING", CYAN),
        RunStatus::Passed => ("PASSED", SUCCESS),
        RunStatus::Failed => ("FAILED", FAILURE),
        RunStatus::Cancelled => ("CANCELLED", WARNING),
    };
    Span::styled(
        format!(" {label} "),
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    )
}

fn truncate(value: &str, max_chars: usize) -> String {
    if max_chars < 2 {
        return String::new();
    }
    let count = value.chars().count();
    if count <= max_chars {
        return value.to_owned();
    }
    value
        .chars()
        .take(max_chars.saturating_sub(1))
        .collect::<String>()
        + "…"
}

fn format_duration(duration: Duration) -> String {
    let seconds = duration.as_secs();
    if seconds >= 60 {
        format!("{}m {:02}s", seconds / 60, seconds % 60)
    } else {
        format!("{seconds}s")
    }
}
