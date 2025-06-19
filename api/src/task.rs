use anyhow::bail;
use anyhow::Context as _;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;
use std::process::{Command, ExitStatus};
use tempdir::TempDir;

#[derive(Deserialize, Serialize, Debug)]
pub struct TaskRequest {
    pub cmdline: Vec<String>,
    pub environment: HashMap<String, String>,
}

#[derive(Debug)]
pub struct TaskResult {
    pub status: ExitStatus,
    pub output: Vec<String>,
}

pub fn execute(request: &TaskRequest) -> crate::Result<TaskResult> {
    if request.cmdline.is_empty() {
        bail!("Task command is empty")
    }

    let (mut recv, send) = std::io::pipe()?;

    let wd = TempDir::new("task")?;
    let mut child = Command::new(&request.cmdline[0])
        .args(&request.cmdline[1..])
        .envs(&request.environment)
        .current_dir(&wd)
        .stdout(send.try_clone()?)
        .stderr(send)
        .spawn()
        .with_context(|| {
            format!(
                "Failed to start command: {}",
                shlex::try_join(request.cmdline.iter().map(AsRef::as_ref))
                    .expect("command has nul byte")
            )
        })?;

    let mut output = Vec::new();
    recv.read_to_end(&mut output)?;

    let status = child.wait()?;

    Ok(TaskResult {
        status,
        output: String::from_utf8_lossy(&output)
            .lines()
            .map(String::from)
            .collect(),
    })
}

// These tests do make some basic assumptions about the command line tools available on the system.
// They will work on Linux and macOS, but maybe not on all Windows installations.
#[cfg(test)]
mod tests {
    use super::*;
    use assertor::*;

    #[test]
    fn can_execute_task() -> anyhow::Result<()> {
        let result = execute(&TaskRequest {
            cmdline: vec![String::from("echo"), String::from("Hello")],
            environment: HashMap::new(),
        })?;
        assert_eq!(result.status.code(), Some(0));
        assert_eq!(result.output, ["Hello"]);
        Ok(())
    }

    #[test]
    fn exit_code_is_reported() -> anyhow::Result<()> {
        let result = execute(&TaskRequest {
            cmdline: vec![String::from("false")],
            environment: HashMap::new(),
        })?;
        assert_eq!(result.status.code(), Some(1));
        Ok(())
    }

    #[test]
    fn cannot_run_empty_command() {
        let result = execute(&TaskRequest {
            cmdline: vec![],
            environment: HashMap::new(),
        });
        assert_that!(result)
            .err()
            .has_message("Task command is empty");
    }

    #[test]
    fn invalid_commands_are_caught() {
        let result = execute(&TaskRequest {
            cmdline: vec![String::from("not-a-real-command")],
            environment: HashMap::new(),
        });
        assert_that!(result)
            .err()
            .as_string()
            .contains("Failed to start command");
    }

    #[test]
    fn can_set_environment_variables() -> anyhow::Result<()> {
        let result = execute(&TaskRequest {
            cmdline: vec![String::from("env")],
            environment: HashMap::from([(String::from("key"), String::from("value"))]),
        })?;

        assert_that!(result.output).contains(String::from("key=value"));

        Ok(())
    }
}
