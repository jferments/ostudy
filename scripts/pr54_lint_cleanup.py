#!/usr/bin/env python3
"""One-shot cleanup for PR #54 real-provider resilience changes."""
from pathlib import Path


def replace_once(path: str, old: str, new: str, label: str) -> None:
    p = Path(path)
    value = p.read_text()
    if new in value:
        return
    if old not in value:
        raise SystemExit(f"{label}: anchor not found in {path}")
    p.write_text(value.replace(old, new, 1))


def remove_once(path: str, old: str, label: str) -> None:
    p = Path(path)
    value = p.read_text()
    if old not in value:
        return
    p.write_text(value.replace(old, "", 1))


# The ordinary JSON request path does not use streaming progress deadlines.
replace_once(
    "src-tauri/src/ai/gateway.rs",
    '''        let openrouter = is_openrouter_url(&url);
        let progress_timeout_seconds = if openrouter {
            OPENROUTER_STREAM_PROGRESS_TIMEOUT_SECONDS
        } else {
            DEFAULT_STREAM_PROGRESS_TIMEOUT_SECONDS
        };
        let progress_timeout = Duration::from_secs(progress_timeout_seconds);
        let mut request = self
            .client''',
    '''        let openrouter = is_openrouter_url(&url);
        let mut request = self
            .client''',
    "non-streaming progress-timeout residue",
)

# document_id is already the registry key; the task-local copy existed only to
# synthesize the removed OpenRouter sticky session_id.
replace_once(
    "src-tauri/src/task_cancellation.rs",
    '''struct ActiveDocumentTask {
    document_id: String,
    signal: CancellationSignal,
    progress: Arc<Mutex<DocumentTaskProgress>>,
}

impl ActiveDocumentTask {
    fn new(document_id: &str, started_at: i64) -> Self {
        Self {
            document_id: document_id.to_string(),
            signal: CancellationSignal::new(),
            progress: Arc::new(Mutex::new(DocumentTaskProgress::new(started_at))),
        }
    }
}''',
    '''struct ActiveDocumentTask {
    signal: CancellationSignal,
    progress: Arc<Mutex<DocumentTaskProgress>>,
}

impl ActiveDocumentTask {
    fn new(started_at: i64) -> Self {
        Self {
            signal: CancellationSignal::new(),
            progress: Arc::new(Mutex::new(DocumentTaskProgress::new(started_at))),
        }
    }
}''',
    "remove redundant task-local document id",
)
replace_once(
    "src-tauri/src/task_cancellation.rs",
    '        let task = ActiveDocumentTask::new(document_id, unix_time_millis()?);',
    '        let task = ActiveDocumentTask::new(unix_time_millis()?);',
    "active document task constructor",
)
remove_once(
    "src-tauri/src/task_cancellation.rs",
    '''pub(crate) fn current_document_id() -> Option<String> {
    CURRENT_DOCUMENT_TASK
        .try_with(|task| task.document_id.clone())
        .ok()
}

''',
    "obsolete current document id accessor",
)

# Keep the progress API lint-clean by grouping the start/end window identity as
# one conceptual range instead of adding an eighth positional argument.
replace_once(
    "src-tauri/src/task_cancellation.rs",
    '''    set_current_task_phase_range(
        phase,
        current_window,
        current_window,
        completed_windows,
        total_windows,
        current_page_start,
        current_page_end,
        current_partition,
    );''',
    '''    set_current_task_phase_range(
        phase,
        (current_window, current_window),
        completed_windows,
        total_windows,
        current_page_start,
        current_page_end,
        current_partition,
    );''',
    "single-window phase range call",
)
replace_once(
    "src-tauri/src/task_cancellation.rs",
    '''pub(crate) fn set_current_task_phase_range(
    phase: &str,
    current_window: Option<usize>,
    current_window_end: Option<usize>,
    completed_windows: usize,
    total_windows: usize,
    current_page_start: Option<i64>,
    current_page_end: Option<i64>,
    current_partition: Option<usize>,
) {
    let _ = CURRENT_DOCUMENT_TASK.try_with(|task| {
        if let Ok(mut progress) = task.progress.lock() {
            progress.phase = phase.chars().take(80).collect();
            progress.current_window = current_window;
            progress.current_window_end = current_window_end;''',
    '''pub(crate) fn set_current_task_phase_range(
    phase: &str,
    current_windows: (Option<usize>, Option<usize>),
    completed_windows: usize,
    total_windows: usize,
    current_page_start: Option<i64>,
    current_page_end: Option<i64>,
    current_partition: Option<usize>,
) {
    let (current_window, current_window_end) = current_windows;
    let _ = CURRENT_DOCUMENT_TASK.try_with(|task| {
        if let Ok(mut progress) = task.progress.lock() {
            progress.phase = phase.chars().take(80).collect();
            progress.current_window = current_window;
            progress.current_window_end = current_window_end;''',
    "group concurrent window range",
)

replace_once(
    "src-tauri/src/ai/document_intelligence_adapter.rs",
    '''        set_current_task_phase_range(
            "outline",
            Some(batch[0] + 1),
            batch.last().map(|index| *index + 1),
            completed,''',
    '''        set_current_task_phase_range(
            "outline",
            (Some(batch[0] + 1), batch.last().map(|index| *index + 1)),
            completed,''',
    "outline progress tuple",
)
replace_once(
    "src-tauri/src/document_generation.rs",
    '''        set_current_task_phase_range(
            "generating",
            Some(batch[0] + 1),
            batch.last().map(|position| *position + 1),
            completed,''',
    '''        set_current_task_phase_range(
            "generating",
            (
                Some(batch[0] + 1),
                batch.last().map(|position| *position + 1),
            ),
            completed,''',
    "generation progress tuple",
)

print("PR #54 focused lint cleanup applied.")
