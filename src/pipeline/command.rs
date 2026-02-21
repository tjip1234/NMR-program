/// NMRPipe command abstraction
///
/// Wraps subprocess calls to NMRPipe tools, captures arguments,
/// and integrates with the reproducibility log.

use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Result of executing an NMRPipe command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    pub success: bool,
    pub command_string: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
}

/// Builder for NMRPipe commands
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NmrPipeCommand {
    pub program: String,
    pub args: Vec<String>,
    pub working_dir: Option<PathBuf>,
    pub input_file: Option<PathBuf>,
    pub output_file: Option<PathBuf>,
    pub description: String,
}

impl NmrPipeCommand {
    pub fn new(program: &str) -> Self {
        Self {
            program: program.to_string(),
            args: Vec::new(),
            working_dir: None,
            input_file: None,
            output_file: None,
            description: String::new(),
        }
    }

    pub fn arg(mut self, arg: &str) -> Self {
        self.args.push(arg.to_string());
        self
    }

    pub fn args(mut self, args: &[&str]) -> Self {
        self.args.extend(args.iter().map(|s| s.to_string()));
        self
    }

    pub fn working_dir(mut self, dir: &Path) -> Self {
        self.working_dir = Some(dir.to_path_buf());
        self
    }

    pub fn input(mut self, path: &Path) -> Self {
        self.input_file = Some(path.to_path_buf());
        self
    }

    pub fn output(mut self, path: &Path) -> Self {
        self.output_file = Some(path.to_path_buf());
        self
    }

    pub fn describe(mut self, desc: &str) -> Self {
        self.description = desc.to_string();
        self
    }

    /// Build the command string for logging/display
    pub fn to_command_string(&self) -> String {
        let mut parts = vec![self.program.clone()];
        parts.extend(self.args.clone());
        parts.join(" ")
    }

    /// Build the shell script representation (for pipe chains)
    pub fn to_shell_script_line(&self) -> String {
        let cmd = self.to_command_string();
        if let (Some(input), Some(output)) = (&self.input_file, &self.output_file) {
            format!(
                "{} -in {} -out {}",
                cmd,
                input.display(),
                output.display()
            )
        } else {
            cmd
        }
    }

    /// Execute the command
    pub fn execute(&self) -> io::Result<CommandResult> {
        let mut cmd = Command::new(&self.program);
        cmd.args(&self.args);

        if let Some(dir) = &self.working_dir {
            cmd.current_dir(dir);
        }

        log::info!("Executing: {}", self.to_command_string());

        let output: Output = cmd.output()?;

        let result = CommandResult {
            success: output.status.success(),
            command_string: self.to_command_string(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code(),
        };

        if !result.success {
            log::warn!(
                "Command failed (exit {}): {}\nstderr: {}",
                result.exit_code.unwrap_or(-1),
                result.command_string,
                result.stderr
            );
        }

        Ok(result)
    }

    /// Execute with piped stdin/stdout (for NMRPipe pipeline chains)
    pub fn execute_piped(&self, stdin_data: Option<&[u8]>) -> io::Result<Vec<u8>> {
        use std::process::Stdio;

        let mut cmd = Command::new(&self.program);
        cmd.args(&self.args);

        if let Some(dir) = &self.working_dir {
            cmd.current_dir(dir);
        }

        if stdin_data.is_some() {
            cmd.stdin(Stdio::piped());
        }
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd.spawn()?;

        if let Some(data) = stdin_data {
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(data)?;
            }
        }

        let output = child.wait_with_output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            log::warn!("Piped command failed: {} | {}", self.to_command_string(), stderr);
        }

        Ok(output.stdout)
    }
}

/// Execute a pipeline of commands connected by pipes (like NMRPipe's | nmrPipe)
pub fn execute_pipeline(commands: &[NmrPipeCommand]) -> io::Result<CommandResult> {
    if commands.is_empty() {
        return Ok(CommandResult {
            success: false,
            command_string: String::new(),
            stdout: String::new(),
            stderr: "Empty pipeline".to_string(),
            exit_code: None,
        });
    }

    let full_cmd = commands
        .iter()
        .map(|c| c.to_command_string())
        .collect::<Vec<_>>()
        .join(" | ");

    // For simplicity, use shell to execute the pipe chain
    let shell_cmd = commands
        .iter()
        .map(|c| c.to_shell_script_line())
        .collect::<Vec<_>>()
        .join(" \\\n| ");

    let output = Command::new("sh")
        .arg("-c")
        .arg(&shell_cmd)
        .output()?;

    Ok(CommandResult {
        success: output.status.success(),
        command_string: full_cmd,
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code(),
    })
}

/// Check if NMRPipe is available on the system
pub fn check_nmrpipe_available() -> bool {
    Command::new("nmrPipe")
        .arg("-help")
        .output()
        .map(|o| o.status.success() || !o.stdout.is_empty() || !o.stderr.is_empty())
        .unwrap_or(false)
}

/// Check if a specific NMRPipe tool is available
pub fn check_tool_available(tool: &str) -> bool {
    Command::new("which")
        .arg(tool)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
