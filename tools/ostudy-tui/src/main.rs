mod model;
mod parser;
mod runner;
mod ui;
mod workflow;

use std::env;
use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::mpsc::TryRecvError;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::DefaultTerminal;

use model::{AppModel, RunStatus};
use parser::{ParsedLine, parse_line};
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
    log_path: Option<PathBuf>,
    errors: Vec<String>,
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

    let summary = ratatui::run(|terminal| run_tui(terminal, &cli.root, cli.workflow))?;
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

fn run_tui(
    terminal: &mut DefaultTerminal,
    root: &Path,
    initial_workflow: Option<Workflow>,
) -> io::Result<RunSummary> {
    let mut model = AppModel::default();
    let mut running: Option<RunningChild> = None;
    let mut exit_code = 0;

    if let Some(workflow) = initial_workflow {
        start_workflow(&mut model, &mut running, root, workflow)?;
    }

    loop {
        drain_runner(&mut model, &mut running)?;
        if model.status == RunStatus::Failed {
            exit_code = model.exit_code.unwrap_or(1);
        } else if model.status == RunStatus::Passed {
            exit_code = 0;
        } else if model.status == RunStatus::Cancelled {
            exit_code = 130;
        }

        terminal.draw(|frame| ui::draw(frame, &model))?;

        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
            && handle_key(key, &mut model, &mut running, root, &mut exit_code)?
        {
            break;
        }
    }

    if let Some(child) = running.as_mut() {
        terminate_process_group(&mut child.child);
        let _ = child.child.wait();
    }

    Ok(RunSummary {
        workflow: model.workflow,
        status: model.status,
        exit_code,
        log_path: model.log_path.clone(),
        errors: model.errors.iter().cloned().collect(),
    })
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

fn drain_runner(model: &mut AppModel, running: &mut Option<RunningChild>) -> io::Result<()> {
    let mut finished = None;
    {
        let Some(child) = running.as_mut() else {
            return Ok(());
        };
        loop {
            match child.receiver.try_recv() {
                Ok(RunnerEvent::Line(_kind, line)) => model.apply(parse_line(&line)),
                Ok(RunnerEvent::ReadError(error)) => {
                    model.apply(ParsedLine::Error(format!("console stream read error: {error}")));
                }
                Ok(RunnerEvent::StreamClosed(_kind)) => child.closed_streams += 1,
                Err(TryRecvError::Empty) => {
                    if child.exit_code.is_none()
                        && let Some(status) = child.child.try_wait()?
                    {
                        child.exit_code = Some(status.code().unwrap_or(1));
                    }
                    if child.closed_streams >= 2 {
                        finished = child.exit_code;
                    }
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
        model.finish(code);
        *running = None;
    }
    Ok(())
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
        RunStatus::Passed => eprintln!("✓ {} passed.", workflow.title),
        RunStatus::Failed => eprintln!("✗ {} failed (exit {}).", workflow.title, summary.exit_code),
        RunStatus::Cancelled => eprintln!("○ {} cancelled.", workflow.title),
        _ => return,
    }
    if let Some(log_path) = &summary.log_path {
        eprintln!("Full log: {}", log_path.display());
    }
    if summary.status == RunStatus::Failed && !summary.errors.is_empty() {
        eprintln!("\nCopy/paste error tail:");
        let start = summary.errors.len().saturating_sub(10);
        for line in &summary.errors[start..] {
            eprintln!("{line}");
        }
    }
}

fn print_help() {
    println!("ostudy developer console");
    println!();
    println!("Usage: ./ostudy-dev [workflow] [--plain]");
    println!();
    println!("With no workflow, an interactive Linux TUI opens. Workflows:");
    for workflow in WORKFLOWS {
        println!("  {:<14} {}", workflow.key, workflow.title);
    }
    println!();
    println!("--plain runs the same maintained repository wrapper without the TUI.");
}
