// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use tokio::signal;

use super::error::{ShutdownError, ShutdownSignalKind};
use super::reason::ShutdownReason;

/// Installed streams retain notifications received during startup checks.
pub(crate) struct InstalledShutdownSignals {
    #[cfg(unix)]
    terminate: signal::unix::Signal,
    #[cfg(unix)]
    interrupt: signal::unix::Signal,
    #[cfg(windows)]
    ctrl_c: signal::windows::CtrlC,
    #[cfg(windows)]
    ctrl_break: signal::windows::CtrlBreak,
    #[cfg(windows)]
    ctrl_close: signal::windows::CtrlClose,
    #[cfg(windows)]
    ctrl_shutdown: signal::windows::CtrlShutdown,
}

pub(crate) fn install_shutdown_signal_listener() -> Result<InstalledShutdownSignals, ShutdownError>
{
    #[cfg(unix)]
    {
        let terminate =
            signal::unix::signal(signal::unix::SignalKind::terminate()).map_err(|_| {
                ShutdownError::ListenerInstallFailed {
                    kind: ShutdownSignalKind::Sigterm,
                }
            })?;
        let interrupt =
            signal::unix::signal(signal::unix::SignalKind::interrupt()).map_err(|_| {
                ShutdownError::ListenerInstallFailed {
                    kind: ShutdownSignalKind::CtrlC,
                }
            })?;
        Ok(InstalledShutdownSignals {
            terminate,
            interrupt,
        })
    }

    #[cfg(windows)]
    {
        let ctrl_c =
            signal::windows::ctrl_c().map_err(|_| ShutdownError::ListenerInstallFailed {
                kind: ShutdownSignalKind::CtrlC,
            })?;
        let ctrl_break =
            signal::windows::ctrl_break().map_err(|_| ShutdownError::ListenerInstallFailed {
                kind: ShutdownSignalKind::CtrlBreak,
            })?;
        let ctrl_close =
            signal::windows::ctrl_close().map_err(|_| ShutdownError::ListenerInstallFailed {
                kind: ShutdownSignalKind::CtrlClose,
            })?;
        let ctrl_shutdown =
            signal::windows::ctrl_shutdown().map_err(|_| ShutdownError::ListenerInstallFailed {
                kind: ShutdownSignalKind::CtrlShutdown,
            })?;
        Ok(InstalledShutdownSignals {
            ctrl_c,
            ctrl_break,
            ctrl_close,
            ctrl_shutdown,
        })
    }

    #[cfg(not(any(unix, windows)))]
    Ok(InstalledShutdownSignals {})
}

impl InstalledShutdownSignals {
    pub(crate) async fn wait_next(&mut self) -> Result<ShutdownReason, ShutdownError> {
        #[cfg(unix)]
        {
            tokio::select! {
                result = self.terminate.recv() => Ok(result.map_or(ShutdownReason::Unknown, |_| ShutdownReason::Sigterm)),
                result = self.interrupt.recv() => Ok(result.map_or(ShutdownReason::Unknown, |_| ShutdownReason::CtrlC)),
            }
        }

        #[cfg(windows)]
        {
            tokio::select! {
                result = self.ctrl_c.recv() => Ok(result.map_or(ShutdownReason::Unknown, |_| ShutdownReason::CtrlC)),
                result = self.ctrl_break.recv() => Ok(result.map_or(ShutdownReason::Unknown, |_| ShutdownReason::CtrlC)),
                _result = self.ctrl_close.recv() => Ok(ShutdownReason::Unknown),
                _result = self.ctrl_shutdown.recv() => Ok(ShutdownReason::Unknown),
            }
        }

        #[cfg(not(any(unix, windows)))]
        {
            signal::ctrl_c()
                .await
                .map(|_| ShutdownReason::CtrlC)
                .map_err(|_| ShutdownError::ListenerInstallFailed {
                    kind: ShutdownSignalKind::CtrlC,
                })
        }
    }
}

/// Waits for an operating-system shutdown signal.
pub async fn shutdown_signal() -> Result<ShutdownReason, ShutdownError> {
    install_shutdown_signal_listener()?.wait_next().await
}
