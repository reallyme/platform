// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use tokio::signal;

use super::error::{ShutdownError, ShutdownSignalKind};
use super::reason::ShutdownReason;

/// Waits for an operating-system shutdown signal and returns when services
/// should begin graceful termination.
///
/// Returning the reason allows services to emit structured audit logs while
/// still keeping the signal handling policy centralized.
pub async fn shutdown_signal() -> Result<ShutdownReason, ShutdownError> {
    #[cfg(unix)]
    let mut terminate_stream = signal::unix::signal(signal::unix::SignalKind::terminate())
        .map_err(|_| ShutdownError::ListenerInstallFailed {
            kind: ShutdownSignalKind::Sigterm,
        })?;

    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .map(|_| ShutdownReason::CtrlC)
            .map_err(|_| ShutdownError::ListenerInstallFailed {
                kind: ShutdownSignalKind::CtrlC,
            })
    };

    #[cfg(unix)]
    let terminate = async {
        match terminate_stream.recv().await {
            Some(_) => Ok(ShutdownReason::Sigterm),
            None => Ok(ShutdownReason::Unknown),
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<Result<ShutdownReason, ShutdownError>>();

    tokio::select! {
        result = ctrl_c => result,
        result = terminate => result,
    }
}
