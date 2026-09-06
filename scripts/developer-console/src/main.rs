mod model;
mod parser;
mod runner;
mod ui;
mod workflow;

use std::env;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::mpsc::{RecvTimeoutError, TryRecvError};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::DefaultTerminal;

use model::{AppModel, RunStatus};
use parser::{ConsoleControl, ParsedLine, parse_line};
use runner::{RunnerEvent, RunningChild, create_log_path, spawn_workflow, terminate_process_group};
use workflow::{WORKFLOWS, Workflow};

struct Cli {
    root: PathBuf,
    workflow: Option<Workflow>,
    plain: bool,
}

struct RunSummary {
    workflow: Option<Workflow>,
    status: RunStatus,
    exit_code: i32,
    elapsed: Duration,
    passed: usize,
    failed: usize,
    skipped: usize,
    log_path: Option<PathBuf>,
    errors: Vec<String>,
}

#[derive(Debug)]
enum TuiBoundary {
    Finished,
    Takeover(String),
}

fn main() -> ExitCode {
    match real_main() {
        Ok(code) => ExitCode::from(code.clamp(0, u8::MAX as i32) as u8),
        Err(error) => {
            eprintln!("ostudy developer console: {error}");
            ExitCode::from(1)
        }
    }
}

fn real_main() -> io::Result<i32> {
    let cli = match parse_cli()? {
        Some(cli) => cli,
        None => return Ok(0),
    };
    if cli.plain || !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        let workflow = cli.workflow.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "a workflow is required when the developer console is not interactive",
            )
        })?;
        return run_plain(workflow, &cli.root);
    }

    let summary = run_interactive(&cli.root, cli.workflow)?;
    print_summary(&summary);
    Ok(summary.exit_code)
}

fn parse_cli() -> io::Result<Option<Cli>> {
    let mut args = env::args().skip(1).peekable();
    let mut root = env::current_dir()?;
    let mut workflow = None;
    let mut plain = false;

    while let Some(argument) = args.next() {
        match argument.as_str() {
            "-h" | "--help" => {
                print_help();
                return Ok(None);
            }
            "--plain" => plain = true,
            "--root" => {
                let value = args.next().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "--root requires a path")
                })?;
                root = PathBuf::from(value);
            }
            value if value.starts_with('-') => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unknown option: {value}"),
                ));
            }
            value => {
                if workflow.is_some() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "only one workflow may be selected",
                    ));
                }
                workflow = Some(Workflow::from_key(value).ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("unknown workflow: {value}"),
                    )
                })?);
            }
        }
    }

    Ok(Some(Cli {
        root,
        workflow,
        plain,
    }))
}

fn run_interactive(root: &Path, initial_workflow: Option<Workflow>) -> io::Result<RunSummary> {
    let mut model = AppModel::default();
    let mut running: Option<RunningChild> = None;
    let mut exit_code = 0;

    if let Some(workflow) = initial_workflow {
        start_workflow(&mut model, &mut running, root, workflow)?;
    }

    loop {
        let boundary = ratatui::run(|terminal| {
            run_tui_segment(terminal, root, &mut model, &mut running, &mut exit_code)
        })?;
        match boundary {
            TuiBoundary::Finished => break,
            TuiBoundary::Takeover(message) => {
                println!("\n[oStudy] Terminal control: {message}");
                io::stdout().flush()?;
                let acknowledge = running
                    .as_ref()
                    .ok_or_else(|| io::Error::other("terminal takeover requested without a workflow"))?
                    .acknowledge_takeover();
                if let Err(error) = acknowledge {
                    stop_running(&mut running);
                    return Err(error);
                }
                if let Err(error) = wait_for_takeover_end(&mut model, &mut running) {
                    stop_running(&mut running);
                    return Err(error);
                }
                println!("\n[oStudy] Returning to the developer console…");
                io::stdout().flush()?;
            }
        }
    }

    stop_running(&mut running);

    Ok(RunSummary {
        workflow: model.workflow,
        status: model.status,
        exit_code,
        elapsed: model.elapsed(),
        passed: model.passed,
        failed: model.failed,
        skipped: model.skipped,
        log_path: model.log_path.clone(),
        errors: model.errors.iter().cloned().collect(),
    })
}

fn run_tui_segment(
    terminal: &mut DefaultTerminal,
    root: &Path,
    model: &mut AppModel,
    running: &mut Option<RunningChild>,
    exit_code: &mut i32,
) -> io::Result<TuiBoundary> {
    loop {
        if let Some(message) = drain_runner(model, running)? {
            return Ok(TuiBoundary::Takeover(message));
        }
        update_exit_code(model, exit_code);

        terminal.draw(|frame| ui::draw(frame, model))?;

        if model.status == RunStatus::Passed {
            return Ok(TuiBoundary::Finished);
        }

        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
            && handle_key(key, model, running, root, exit_code)?
        {
            return Ok(TuiBoundary::Finished);
        }
    }
}

fn start_workflow(
    model: &mut AppModel,
    running: &mut Option<RunningChild>,
    root: &Path,
    workflow: Workflow,
) -> io::Result<()> {
    let log_path = create_log_path(workflow)?;
    let child = spawn_workflow(root, workflow, &log_path)?;
    let pid = child.child.id();
    model.begin(workflow, log_path, pid);
    *running = Some(child);
    Ok(())
}

fn process_runner_event(
    model: &mut AppModel,
    child: &mut RunningChild,
    event: RunnerEvent,
) -> Option<ConsoleControl> {
    match event {
        RunnerEvent::Line(_kind, line) => model.apply(parse_line(&line)),
        RunnerEvent::Control(control) => return Some(control),
        RunnerEvent::ReadError(error) => {
            model.apply(ParsedLine::Error(format!("console stream read error: {error}")));
        }
        RunnerEvent::StreamClosed(_kind) => child.closed_streams += 1,
    }
    None
}

fn refresh_child_exit(child: &mut RunningChild) -> io::Result<Option<i32>> {
    if child.exit_code.is_none()
        && let Some(status) = child.child.try_wait()?
    {
        child.exit_code = Some(status.code().unwrap_or(1));
    }
    if child.closed_streams >= 2 {
        Ok(child.exit_code)
    } else {
        Ok(None)
    }
}

fn finish_running(model: &mut AppModel, running: &mut Option<RunningChild>, code: i32) {
    if let Some(child) = running.as_ref() {
        child.cleanup_takeover_ack();
    }
    model.finish(code);
    *running = None;
}

fn drain_runner(model: &mut AppModel, running: &mut Option<RunningChild>) -> io::Result<Option<String>> {
    let mut finished = None;
    let mut takeover = None;
    {
        let Some(child) = running.as_mut() else {
            return Ok(None);
        };
        loop {
            match child.receiver.try_recv() {
                Ok(event) => {
                    if let Some(control) = process_runner_event(model, child, event) {
                        match control {
                            ConsoleControl::TakeoverStart { message } => {
                                takeover = Some(message);
                                break;
                            }
                            ConsoleControl::TakeoverEnd { exit_code, message } => {
                                let line = format!(
                                    "interactive terminal command returned unexpectedly: {message} (exit {exit_code})"
                                );
                                model.apply(if exit_code == 0 {
                                    ParsedLine::Output(line)
                                } else {
                                    ParsedLine::Error(line)
                                });
                            }
                        }
                    }
                }
                Err(TryRecvError::Empty) => {
                    finished = refresh_child_exit(child)?;
                    break;
                }
                Err(TryRecvError::Disconnected) => {
                    let code = if let Some(code) = child.exit_code {
                        code
                    } else {
                        child.child.wait()?.code().unwrap_or(1)
                    };
                    finished = Some(code);
                    break;
                }
            }
        }
    }

    if let Some(code) = finished {
        finish_running(model, running, code);
    }
    Ok(takeover)
}

fn wait_for_takeover_end(model: &mut AppModel, running: &mut Option<RunningChild>) -> io::Result<()> {
    loop {
        let mut finished = None;
        let mut returned = false;
        {
            let Some(child) = running.as_mut() else {
                return Ok(());
            };
            match child.receiver.recv_timeout(Duration::from_millis(100)) {
                Ok(event) => {
                    if let Some(control) = process_runner_event(model, child, event) {
                        match control {
                            ConsoleControl::TakeoverStart { message } => {
                                return Err(io::Error::other(format!(
                                    "nested terminal takeover is not supported: {message}"
                                )));
                            }
                            ConsoleControl::TakeoverEnd { exit_code, message } => {
                                let line = format!(
                                    "interactive terminal command finished: {message} (exit {exit_code})"
                                );
                                model.apply(if exit_code == 0 {
                                    ParsedLine::Output(line)
                                } else {
                                    ParsedLine::Error(line)
                                });
                                returned = true;
                            }
                        }
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    let code = if let Some(code) = child.exit_code {
                        code
                    } else {
                        child.child.wait()?.code().unwrap_or(1)
                    };
                    finished = Some(code);
                }
            }
            if finished.is_none() {
                finished = refresh_child_exit(child)?;
            }
        }

        if let Some(code) = finished {
            finish_running(model, running, code);
            return Ok(());
        }
        if returned {
            return Ok(());
        }
    }
}

fn update_exit_code(model: &AppModel, exit_code: &mut i32) {
    if model.status == RunStatus::Failed {
        *exit_code = model.exit_code.unwrap_or(1);
    } else if model.status == RunStatus::Passed {
        *exit_code = 0;
    } else if model.status == RunStatus::Cancelled {
        *exit_code = 130;
    }
}

fn stop_running(running: &mut Option<RunningChild>) {
    if let Some(child) = running.as_mut() {
        child.cleanup_takeover_ack();
        terminate_process_group(&mut child.child);
        let _ = child.child.wait();
    }
    *running = None;
}

fn handle_key(
    key: KeyEvent,
    model: &mut AppModel,
    running: &mut Option<RunningChild>,
    root: &Path,
    exit_code: &mut i32,
) -> io::Result<bool> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        if let Some(child) = running.as_mut() {
            terminate_process_group(&mut child.child);
            model.cancel();
        }
        *exit_code = 130;
        return Ok(true);
    }

    match model.status {
        RunStatus::Menu => match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return Ok(true),
            KeyCode::Down | KeyCode::Char('j') => {
                model.selected_index = (model.selected_index + 1) % WORKFLOWS.len();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                model.selected_index = if model.selected_index == 0 {
                    WORKFLOWS.len() - 1
                } else {
                    model.selected_index - 1
                };
            }
            KeyCode::Enter => {
                start_workflow(model, running, root, WORKFLOWS[model.selected_index])?;
            }
            _ => {}
        },
        RunStatus::Running => match key.code {
            KeyCode::Char('e') => model.show_errors = !model.show_errors,
            KeyCode::Char('q') | KeyCode::Esc => {
                if let Some(child) = running.as_mut() {
                    terminate_process_group(&mut child.child);
                }
                model.cancel();
                *exit_code = 130;
                return Ok(true);
            }
            _ => {}
        },
        RunStatus::Passed | RunStatus::Failed | RunStatus::Cancelled => match key.code {
            KeyCode::Char('e') => model.show_errors = !model.show_errors,
            KeyCode::Enter => {
                model.reset_to_menu();
                *exit_code = 0;
            }
            KeyCode::Char('q') | KeyCode::Esc => return Ok(true),
            _ => {}
        },
    }
    Ok(false)
}

fn run_plain(workflow: Workflow, root: &Path) -> io::Result<i32> {
    let status = workflow.command(root, false).status()?;
    Ok(status.code().unwrap_or(1))
}

fn print_summary(summary: &RunSummary) {
    let Some(workflow) = summary.workflow else {
        return;
    };
    match summary.status {
        RunStatus::Passed => println!(
            "✓ {} passed in {}.",
            workflow.title,
            format_duration(summary.elapsed)
        ),
        RunStatus::Failed => eprintln!(
            "✗ {} failed after {} (exit {}).",
            workflow.title,
            format_duration(summary.elapsed),
            summary.exit_code
        ),
        RunStatus::Cancelled => eprintln!(
            "○ {} cancelled after {}.",
            workflow.title,
            format_duration(summary.elapsed)
        ),
        _ => return,
    }

    let total_tests = summary.passed + summary.failed + summary.skipped;
    if total_tests > 0 {
        println!(
            "Tests: {} passed, {} failed, {} skipped ({} total).",
            summary.passed, summary.failed, summary.skipped, total_tests
        );
    }
    if let Some(log_path) = &summary.log_path {
        println!("Full log: {}", log_path.display());
    }
    if summary.status == RunStatus::Failed && !summary.errors.is_empty() {
        eprintln!("\nCopy/paste error tail:");
        let start = summary.errors.len().saturating_sub(10);
        for line in &summary.errors[start..] {
            eprintln!("{line}");
        }
    }
}

fn format_duration(duration: Duration) -> String {
    let seconds = duration.as_secs();
    if seconds >= 60 {
        format!("{}m {:02}s", seconds / 60, seconds % 60)
    } else {
        format!("{seconds}s")
    }
}

fn print_help() {
    println!("ostudy developer console");
    println!();
    println!("Usage: ./scripts/ostudy-dev [workflow] [--plain]");
    println!();
    println!("With no workflow, an interactive Linux TUI opens. Workflows:");
    for workflow in WORKFLOWS {
        println!("  {:<14} {}", workflow.key, workflow.title);
    }
    println!();
    println!("--plain runs the same maintained repository wrapper without the TUI.");
}
