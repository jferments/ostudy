use std::path::Path;
use std::process::Command;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkflowKind {
    Setup,
    Build,
    CheckChanged,
    CheckStatic,
    CheckFast,
    CheckFull,
    Run,
    Desktop,
}

#[derive(Clone, Copy, Debug)]
pub struct Workflow {
    pub kind: WorkflowKind,
    pub key: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub scope: &'static str,
    pub total_steps: Option<usize>,
}

pub const WORKFLOWS: [Workflow; 8] = [
    Workflow {
        kind: WorkflowKind::Run,
        key: "run",
        title: "Run current build",
        description: "Verify provenance, launch the staged desktop app, and retain application logs.",
        scope: "run",
        total_steps: Some(1),
    },
    Workflow {
        kind: WorkflowKind::Build,
        key: "build",
        title: "Build desktop app",
        description: "Run the maintained standalone Linux debug build path.",
        scope: "build",
        total_steps: Some(5),
    },
    Workflow {
        kind: WorkflowKind::CheckChanged,
        key: "check-changed",
        title: "Check changed files",
        description: "Use the repository's impact-aware iteration check for the current diff.",
        scope: "changed",
        total_steps: None,
    },
    Workflow {
        kind: WorkflowKind::CheckFast,
        key: "check-fast",
        title: "Fast qualification",
        description: "Run static contracts, frontend tests, native checks, and focused regressions.",
        scope: "fast",
        total_steps: Some(13),
    },
    Workflow {
        kind: WorkflowKind::CheckFull,
        key: "check",
        title: "Full Linux qualification",
        description: "Run the authoritative Linux/shared build, tests, and native smoke suite.",
        scope: "full",
        total_steps: Some(9),
    },
    Workflow {
        kind: WorkflowKind::CheckStatic,
        key: "check-static",
        title: "Static qualification",
        description: "Verify manifests, repository contracts, type checking, formatting, and docs.",
        scope: "static",
        total_steps: Some(6),
    },
    Workflow {
        kind: WorkflowKind::Desktop,
        key: "desktop",
        title: "Desktop hot reload",
        description: "Start the maintained Tauri development server for interactive UI work.",
        scope: "desktop",
        total_steps: Some(1),
    },
    Workflow {
        kind: WorkflowKind::Setup,
        key: "setup",
        title: "Ubuntu/Debian setup",
        description: "Provision the pinned Linux development environment and initial build.",
        scope: "setup",
        total_steps: None,
    },
];

impl Workflow {
    pub fn from_key(key: &str) -> Option<Self> {
        WORKFLOWS.iter().copied().find(|workflow| workflow.key == key)
    }

    pub fn command(&self, root: &Path, console_events: bool) -> Command {
        let mut command = Command::new("bash");
        match self.kind {
            WorkflowKind::Setup => {
                command.arg(root.join("scripts/setup-ubuntu.sh"));
            }
            WorkflowKind::Build => {
                command.arg(root.join("scripts/build.sh"));
            }
            WorkflowKind::CheckChanged => {
                command.arg(root.join("scripts/check-changed.sh"));
            }
            WorkflowKind::CheckStatic => {
                command.arg(root.join("scripts/check-static.sh"));
            }
            WorkflowKind::CheckFast => {
                command.arg(root.join("scripts/check-fast.sh"));
            }
            WorkflowKind::CheckFull => {
                command.arg(root.join("scripts/check.sh"));
            }
            WorkflowKind::Run => {
                command.arg(root.join("run-app.sh"));
            }
            WorkflowKind::Desktop => {
                command.arg(root.join("scripts/dev.sh"));
            }
        }
        command.current_dir(root);
        command.env("NO_COLOR", "1");
        command.env("CLICOLOR", "0");
        command.env("FORCE_COLOR", "0");
        if console_events {
            command.env("OSTUDY_CONSOLE_EVENTS", "1");
        } else {
            command.env_remove("OSTUDY_CONSOLE_EVENTS");
        }
        command
    }

    pub fn shell_preview(&self) -> &'static str {
        match self.kind {
            WorkflowKind::Setup => "./scripts/setup-ubuntu.sh",
            WorkflowKind::Build => "./scripts/build.sh",
            WorkflowKind::CheckChanged => "./scripts/check-changed.sh",
            WorkflowKind::CheckStatic => "./scripts/check-static.sh",
            WorkflowKind::CheckFast => "./scripts/check-fast.sh",
            WorkflowKind::CheckFull => "./scripts/check.sh",
            WorkflowKind::Run => "./run-app.sh",
            WorkflowKind::Desktop => "./scripts/dev.sh",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_keys_are_unique() {
        for (index, workflow) in WORKFLOWS.iter().enumerate() {
            assert!(
                WORKFLOWS[index + 1..]
                    .iter()
                    .all(|candidate| candidate.key != workflow.key),
                "duplicate workflow key: {}",
                workflow.key
            );
        }
    }

    #[test]
    fn known_workflow_resolves() {
        assert_eq!(
            Workflow::from_key("check-fast").map(|workflow| workflow.kind),
            Some(WorkflowKind::CheckFast)
        );
        assert!(Workflow::from_key("not-a-workflow").is_none());
    }
}
