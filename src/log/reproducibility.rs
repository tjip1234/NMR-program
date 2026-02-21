/// Reproducibility logging system
///
/// Every operation performed on NMR data is recorded with:
/// - Timestamp
/// - Operation description
/// - Exact NMRPipe command/flags used
/// - Parameter values
/// - Sequential order
///
/// The log can be exported as:
/// - Human-readable text
/// - JSON
/// - Executable shell script (to reproduce results independently)

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

/// A single log entry representing one operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Sequential operation number (1-based)
    pub sequence: usize,
    /// Timestamp when the operation was performed
    pub timestamp: DateTime<Local>,
    /// Human-readable operation name
    pub operation: String,
    /// Detailed description of what was done
    pub description: String,
    /// The exact NMRPipe command equivalent
    pub nmrpipe_command: String,
}

impl LogEntry {
    /// Format as human-readable text line
    pub fn to_text(&self) -> String {
        format!(
            "[{:03}] {} | {} | {}\n      Command: {}",
            self.sequence,
            self.timestamp.format("%Y-%m-%d %H:%M:%S"),
            self.operation,
            self.description,
            if self.nmrpipe_command.is_empty() {
                "(n/a)".to_string()
            } else {
                self.nmrpipe_command.clone()
            }
        )
    }

    /// Format as shell script line
    pub fn to_shell_line(&self) -> String {
        if self.nmrpipe_command.is_empty() || self.nmrpipe_command.starts_with('#') {
            format!("# Step {}: {} — {}", self.sequence, self.operation, self.description)
        } else {
            format!(
                "# Step {}: {} — {}\n{}",
                self.sequence, self.operation, self.description, self.nmrpipe_command
            )
        }
    }
}

/// The reproducibility log — records all operations in order
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReproLog {
    /// Session metadata
    pub session_id: String,
    pub session_start: DateTime<Local>,
    pub source_file: String,
    pub software_version: String,
    /// Ordered list of operations
    pub entries: Vec<LogEntry>,
}

impl ReproLog {
    /// Create a new empty log
    pub fn new() -> Self {
        Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            session_start: Local::now(),
            source_file: String::new(),
            software_version: env!("CARGO_PKG_VERSION").to_string(),
            entries: Vec::new(),
        }
    }

    /// Set the source file for this session
    pub fn set_source(&mut self, source: &str) {
        self.source_file = source.to_string();
    }

    /// Add an operation to the log
    pub fn add_entry(&mut self, operation: &str, description: &str, nmrpipe_command: &str) {
        let seq = self.entries.len() + 1;
        self.entries.push(LogEntry {
            sequence: seq,
            timestamp: Local::now(),
            operation: operation.to_string(),
            description: description.to_string(),
            nmrpipe_command: nmrpipe_command.to_string(),
        });
        log::info!("[LOG {:03}] {} — {}", seq, operation, description);
    }

    /// Remove the last entry (for undo)
    pub fn pop_entry(&mut self) -> Option<LogEntry> {
        self.entries.pop()
    }

    /// Get the number of operations
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Export as human-readable text
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        out.push_str("═══════════════════════════════════════════════════════════════\n");
        out.push_str("  NMR Processing Reproducibility Log\n");
        out.push_str("═══════════════════════════════════════════════════════════════\n");
        out.push_str(&format!("  Session ID:  {}\n", self.session_id));
        out.push_str(&format!(
            "  Started:     {}\n",
            self.session_start.format("%Y-%m-%d %H:%M:%S")
        ));
        out.push_str(&format!("  Source:      {}\n", self.source_file));
        out.push_str(&format!("  Software:    NMR-GUI v{}\n", self.software_version));
        out.push_str(&format!("  Operations:  {}\n", self.entries.len()));
        out.push_str("───────────────────────────────────────────────────────────────\n\n");

        for entry in &self.entries {
            out.push_str(&entry.to_text());
            out.push_str("\n\n");
        }

        out.push_str("═══════════════════════════════════════════════════════════════\n");
        out.push_str(&format!(
            "  Log exported: {}\n",
            Local::now().format("%Y-%m-%d %H:%M:%S")
        ));
        out.push_str("═══════════════════════════════════════════════════════════════\n");
        out
    }

    /// Export as JSON
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|e| format!("JSON error: {}", e))
    }

    /// Export as executable shell script
    pub fn to_shell_script(&self) -> String {
        let mut out = String::new();
        out.push_str("#!/bin/bash\n");
        out.push_str("#\n");
        out.push_str("# NMR Processing Reproducibility Script\n");
        out.push_str(&format!("# Generated by NMR-GUI v{}\n", self.software_version));
        out.push_str(&format!(
            "# Session: {} ({})\n",
            self.session_id,
            self.session_start.format("%Y-%m-%d %H:%M:%S")
        ));
        out.push_str(&format!("# Source: {}\n", self.source_file));
        out.push_str("#\n");
        out.push_str("# This script reproduces the exact processing steps.\n");
        out.push_str("# Requirements: NMRPipe must be installed and in PATH.\n");
        out.push_str("#\n");
        out.push_str("set -euo pipefail\n\n");

        for entry in &self.entries {
            out.push_str(&entry.to_shell_line());
            out.push_str("\n\n");
        }

        out.push_str("echo \"Processing complete.\"\n");
        out
    }

    /// Save log as text file
    pub fn save_text(&self, path: &Path) -> io::Result<()> {
        std::fs::write(path, self.to_text())
    }

    /// Save log as JSON file
    pub fn save_json(&self, path: &Path) -> io::Result<()> {
        std::fs::write(path, self.to_json())
    }

    /// Save log as shell script
    pub fn save_script(&self, path: &Path) -> io::Result<()> {
        use std::os::unix::fs::PermissionsExt;
        std::fs::write(path, self.to_shell_script())?;
        // Make executable
        let perms = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(path, perms)?;
        Ok(())
    }
}

impl Default for ReproLog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_creation_and_entries() {
        let mut log = ReproLog::new();
        assert!(log.is_empty());

        log.add_entry("Test Op", "Did something", "nmrPipe -fn FT");
        assert_eq!(log.len(), 1);
        assert_eq!(log.entries[0].sequence, 1);
        assert_eq!(log.entries[0].operation, "Test Op");

        log.add_entry("Second Op", "Did more", "nmrPipe -fn PS -p0 45.0");
        assert_eq!(log.len(), 2);
        assert_eq!(log.entries[1].sequence, 2);
    }

    #[test]
    fn test_undo_pops_last() {
        let mut log = ReproLog::new();
        log.add_entry("Op1", "desc1", "cmd1");
        log.add_entry("Op2", "desc2", "cmd2");

        let popped = log.pop_entry().unwrap();
        assert_eq!(popped.operation, "Op2");
        assert_eq!(log.len(), 1);
    }

    #[test]
    fn test_text_export() {
        let mut log = ReproLog::new();
        log.set_source("test.fid");
        log.add_entry("FT", "Fourier Transform", "nmrPipe -fn FT -auto");
        let text = log.to_text();
        assert!(text.contains("Fourier Transform"));
        assert!(text.contains("nmrPipe -fn FT -auto"));
    }

    #[test]
    fn test_json_roundtrip() {
        let mut log = ReproLog::new();
        log.add_entry("Test", "test desc", "test cmd");
        let json = log.to_json();
        let parsed: ReproLog = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.entries.len(), 1);
    }

    #[test]
    fn test_shell_script_export() {
        let mut log = ReproLog::new();
        log.add_entry("FT", "FFT", "nmrPipe -fn FT -auto");
        let script = log.to_shell_script();
        assert!(script.starts_with("#!/bin/bash"));
        assert!(script.contains("nmrPipe -fn FT -auto"));
    }
}
