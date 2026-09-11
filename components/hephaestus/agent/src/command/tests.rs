// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{MAX_OUTPUT_BYTES, output, read_output};
use crate::error::HephaestusAgentErrorReason;
use std::time::Duration;
use tokio::process::Command;

#[tokio::test]
async fn bounds_output_at_the_exact_limit() {
    let maximum = usize::try_from(MAX_OUTPUT_BYTES).expect("test bound");
    let bytes = vec![b'x'; maximum];
    assert_eq!(
        read_output(bytes.as_slice()).await.expect("bounded output"),
        bytes
    );
    let oversized = vec![b'x'; maximum + 1];
    let error = read_output(oversized.as_slice())
        .await
        .expect_err("oversized output");
    assert_eq!(
        error.reason(),
        HephaestusAgentErrorReason::CommandOutputInvalid
    );
}

#[cfg(unix)]
#[tokio::test]
async fn captures_stdout_stderr_and_exit_status() {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "printf out; printf err >&2; exit 7"]);
    let result = output(&mut command).await.expect("command output");
    assert_eq!(result.stdout, b"out");
    assert_eq!(result.stderr, b"err");
    assert_eq!(result.status.code(), Some(7));
}

#[tokio::test]
async fn classifies_missing_executable_without_exposing_io_details() {
    let mut command = Command::new("/reallyme-test/missing-executable");
    let error = output(&mut command)
        .await
        .expect_err("missing executable should fail");
    assert_eq!(
        error.reason(),
        HephaestusAgentErrorReason::CommandUnavailable
    );
}

#[cfg(unix)]
#[tokio::test]
async fn cancelled_command_cannot_perform_a_late_write() {
    let directory = tempfile::tempdir().expect("test directory");
    let marker = directory.path().join("late-write");
    let mut command = Command::new("/bin/sh");
    // The shell is the child under test. The fixed script receives the fixture
    // path as an argument, avoiding any shell interpolation of path contents.
    command
        .args(["-c", "sleep 0.2; printf late > \"$1\"", "test"])
        .arg(&marker);
    assert!(
        tokio::time::timeout(Duration::from_millis(50), output(&mut command))
            .await
            .is_err()
    );
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!marker.exists());
}
