// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded subprocess output and cancellation at the OS adapter boundary.

use crate::error::{AgentResult, HephaestusAgentError, HephaestusAgentErrorReason};
use std::io::ErrorKind;
use std::process::{Output, Stdio};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;
use zeroize::Zeroizing;

const MAX_OUTPUT_BYTES: u64 = 4 * 1024 * 1024;

pub(crate) async fn output(command: &mut Command) -> AgentResult<Output> {
    // Tokio otherwise leaves children running when a timeout drops their future.
    command
        .kill_on_drop(true)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|error| {
        if error.kind() == ErrorKind::NotFound {
            HephaestusAgentError::new(HephaestusAgentErrorReason::CommandUnavailable)
        } else {
            command_failed()
        }
    })?;
    let stdout = child.stdout.take().ok_or_else(command_failed)?;
    let stderr = child.stderr.take().ok_or_else(command_failed)?;
    let (stdout, stderr, status) =
        tokio::try_join!(read_output(stdout), read_output(stderr), async {
            child.wait().await.map_err(|_| command_failed())
        })?;
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

async fn read_output(reader: impl AsyncRead + Unpin) -> AgentResult<Vec<u8>> {
    let limit = MAX_OUTPUT_BYTES.checked_add(1).ok_or_else(command_failed)?;
    let mut bytes = Zeroizing::new(Vec::new());
    reader
        .take(limit)
        .read_to_end(&mut bytes)
        .await
        .map_err(|_| command_failed())?;
    let length = u64::try_from(bytes.len()).map_err(|_| command_failed())?;
    if length > MAX_OUTPUT_BYTES {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::CommandOutputInvalid,
        ));
    }
    Ok(std::mem::take(&mut *bytes))
}

fn command_failed() -> HephaestusAgentError {
    HephaestusAgentError::new(HephaestusAgentErrorReason::CommandFailed)
}

#[cfg(test)]
mod tests;
