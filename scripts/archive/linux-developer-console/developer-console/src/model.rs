use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::parser::{ParsedLine, TestResult, TestStatus};
use crate::workflow::Workflow;

const RECENT_TEST_LIMIT: usize = 8;
const RECENT_OUTPUT_LIMIT: usize = 14;
const ERROR_LIMIT: usize = 80;
const ERROR_CONTEXT_LINES: usize = 6;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunStatus {
    Menu,
    Running,
    Passed,
    Failed,
    Cancelled,
}

pub struct AppModel {
    pub status: RunStatus,
    pub selected_index: usize,
    pub workflow: Option<Workflow>,
    pub started_at: Option<Instant>,
    pub finished_at: Option<Instant>,
    pub stage_index: usize,
    pub current_stage: String,
    pub nested_stage: Option<String>,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub recent_tests: VecDeque<TestResult>,
    pub recent_output: VecDeque<String>,
    pub errors: VecDeque<String>,
    pub log_path: Option<PathBuf>,
    pub child_pid: Option<u32>,
    pub exit_code: Option<i32>,
    pub show_errors: bool,
    error_context_remaining: usize,
}

impl Default for AppModel {
    fn default() -> Self {
        Self {
            status: RunStatus::Menu,
            selected_index: 0,
            workflow: None,
            started_at: None,
            finished_at: None,
            stage_index: 0,
            current_stage: "Choose a workflow".to_owned(),
            nested_stage: None,
            passed: 0,
            failed: 0,
            skipped: 0,
            recent_tests: VecDeque::new(),
            recent_output: VecDeque::new(),
            errors: VecDeque::new(),
            log_path: None,
            child_pid: None,
            exit_code: None,
            show_errors: false,
            error_context_remaining: 0,
        }
    }
}

impl AppModel {
    pub fn begin(&mut self, workflow: Workflow, log_path: PathBuf, child_pid: u32) {
        self.status = RunStatus::Running;
        self.workflow = Some(workflow);
        self.started_at = Some(Instant::now());
        self.finished_at = None;
        self.stage_index = 0;
        self.current_stage = "Starting workflow".to_owned();
        self.nested_stage = None;
        self.passed = 0;
        self.failed = 0;
        self.skipped = 0;
        self.recent_tests.clear();
        self.recent_output.clear();
        self.errors.clear();
        self.log_path = Some(log_path);
        self.child_pid = Some(child_pid);
        self.exit_code = None;
        self.show_errors = false;
        self.error_context_remaining = 0;
    }

    pub fn apply(&mut self, parsed: ParsedLine) {
        match parsed {
            ParsedLine::Stage { scope, name } => {
                if self.workflow.is_some_and(|workflow| workflow.scope == scope) {
                    self.stage_index += 1;
                    self.current_stage = name;
                    self.nested_stage = None;
                } else {
                    self.nested_stage = Some(format!("{scope} › {name}"));
                }
            }
            ParsedLine::Test(test) => {
                match test.status {
                    TestStatus::Passed => self.passed += test.count,
                    TestStatus::Failed => {
                        self.failed += test.count;
                        self.push_error(format!("FAILED: {}", test.name));
                    }
                    TestStatus::Skipped => self.skipped += test.count,
                }
                self.recent_tests.push_back(test);
                while self.recent_tests.len() > RECENT_TEST_LIMIT {
                    self.recent_tests.pop_front();
                }
            }
            ParsedLine::Error(line) => {
                self.push_error(line.clone());
                self.push_output(line);
                self.error_context_remaining = ERROR_CONTEXT_LINES;
            }
            ParsedLine::Output(line) => {
                if self.error_context_remaining > 0 {
                    self.push_error(line.clone());
                    self.error_context_remaining -= 1;
                }
                self.push_output(line);
            }
            ParsedLine::Noise => {}
        }
    }

    pub fn finish(&mut self, exit_code: i32) {
        self.exit_code = Some(exit_code);
        self.status = if exit_code == 0 {
            RunStatus::Passed
        } else {
            RunStatus::Failed
        };
        if exit_code != 0 {
            self.show_errors = true;
        }
        if exit_code == 0
            && let Some(total_steps) = self.workflow.and_then(|workflow| workflow.total_steps)
        {
            self.stage_index = total_steps;
        }
        self.finished_at = Some(Instant::now());
        self.child_pid = None;
    }

    pub fn cancel(&mut self) {
        self.status = RunStatus::Cancelled;
        self.finished_at = Some(Instant::now());
        self.child_pid = None;
        self.exit_code = Some(130);
    }

    pub fn reset_to_menu(&mut self) {
        let selected_index = self.selected_index;
        *self = Self::default();
        self.selected_index = selected_index;
    }

    pub fn elapsed(&self) -> Duration {
        let Some(started) = self.started_at else {
            return Duration::ZERO;
        };
        self.finished_at.unwrap_or_else(Instant::now) - started
    }

    pub fn total_tests(&self) -> usize {
        self.passed + self.failed + self.skipped
    }

    pub fn progress_ratio(&self) -> Option<f64> {
        let total = self.workflow?.total_steps?;
        if total == 0 {
            return Some(0.0);
        }
        Some((self.stage_index as f64 / total as f64).clamp(0.0, 1.0))
    }

    fn push_output(&mut self, line: String) {
        self.recent_output.push_back(line);
        while self.recent_output.len() > RECENT_OUTPUT_LIMIT {
            self.recent_output.pop_front();
        }
    }

    fn push_error(&mut self, line: String) {
        self.errors.push_back(line);
        while self.errors.len() > ERROR_LIMIT {
            self.errors.pop_front();
        }
    }
}
