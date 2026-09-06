use std::env;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

use crate::parser::{ConsoleControl, EVENT_PREFIX, parse_console_control, strip_ansi};
use crate::workflow::Workflow;

#[derive(Clone, Copy, Debug)]
pub enum StreamKind {
    Stdout,
    Stderr,
}

#[derive(Debug)]
pub enum RunnerEvent {
    Line(StreamKind, String),
    Control(ConsoleControl),
    ReadError(String),
    StreamClosed(StreamKind),
}

pub struct RunningChild {
    pub child: Child,
    pub receiver: mpsc::Receiver<RunnerEvent>,
    pub closed_streams: usize,
    pub exit_code: Option<i32>,
    takeover_ack_path: PathBuf,
}

impl RunningChild {
    pub fn acknowledge_takeover(&self) -> io::Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.takeover_ack_path)?;
        file.write_all(b"ready\n")?;
        file.flush()
    }

    pub fn cleanup_takeover_ack(&self) {
        let _ = std::fs::remove_file(&self.takeover_ack_path);
    }
}

pub fn create_log_path(workflow: Workflow) -> io::Result<PathBuf> {
    let directory = log_directory();
    std::fs::create_dir_all(&directory)?;
    let epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    Ok(directory.join(format!(
        "{}-{epoch}-{}.log",
        workflow.key,
        std::process::id()
    )))
}

pub fn spawn_workflow(root: &Path, workflow: Workflow, log_path: &Path) -> io::Result<RunningChild> {
    let takeover_ack_path = log_path.with_extension("takeover-ready");
    let _ = std::fs::remove_file(&takeover_ack_path);

    let mut command = workflow.command(root, true);
    command
        .env("OSTUDY_CONSOLE_TAKEOVER_ACK", &takeover_ack_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    command.process_group(0);

    let mut child = command.spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("workflow stdout pipe was unavailable"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("workflow stderr pipe was unavailable"))?;

    let log = Arc::new(Mutex::new(
        OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(log_path)?,
    ));
    {
        let mut file = log
            .lock()
            .map_err(|_| io::Error::other("developer console log lock poisoned"))?;
        writeln!(file, "# ostudy developer console")?;
        writeln!(file, "# workflow: {}", workflow.shell_preview())?;
        writeln!(file, "# raw child output follows; terminal color escapes removed")?;
    }

    let (sender, receiver) = mpsc::channel();
    spawn_reader(stdout, StreamKind::Stdout, sender.clone(), Arc::clone(&log));
    spawn_reader(stderr, StreamKind::Stderr, sender, log);

    Ok(RunningChild {
        child,
        receiver,
        closed_streams: 0,
        exit_code: None,
        takeover_ack_path,
    })
}

pub fn terminate_process_group(child: &mut Child) {
    #[cfg(unix)]
    {
        let pid = child.id().to_string();
        let status = std::process::Command::new("kill")
            .arg("-TERM")
            .arg(format!("-{pid}"))
            .status();
        if status.as_ref().is_ok_and(|status| status.success()) {
            return;
        }
    }
    let _ = child.kill();
}

fn spawn_reader<R>(
    reader: R,
    kind: StreamKind,
    sender: mpsc::Sender<RunnerEvent>,
    log: Arc<Mutex<File>>,
) where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let reader = BufReader::new(reader);
        for line in reader.lines() {
            match line {
                Ok(line) => {
                    let clean = strip_ansi(&line).into_owned();
                    if let Some(control) = parse_console_control(&clean) {
                        if sender.send(RunnerEvent::Control(control)).is_err() {
                            return;
                        }
                        continue;
                    }
                    if !clean.trim_start().starts_with(EVENT_PREFIX)
                        && let Ok(mut file) = log.lock()
                    {
                        let stream = match kind {
                            StreamKind::Stdout => "stdout",
                            StreamKind::Stderr => "stderr",
                        };
                        let _ = writeln!(file, "[{stream}] {clean}");
                    }
                    if sender.send(RunnerEvent::Line(kind, clean)).is_err() {
                        return;
                    }
                }
                Err(error) => {
                    let _ = sender.send(RunnerEvent::ReadError(error.to_string()));
                    break;
                }
            }
        }
        let _ = sender.send(RunnerEvent::StreamClosed(kind));
    });
}

fn log_directory() -> PathBuf {
    if let Some(path) = env::var_os("OSTUDY_DEV_LOG_DIR") {
        return PathBuf::from(path);
    }
    if let Some(path) = env::var_os("XDG_STATE_HOME") {
        return PathBuf::from(path).join("ostudy/dev-console");
    }
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".local/state/ostudy/dev-console")
}
