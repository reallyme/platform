// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::{Path, PathBuf};

use secrecy::ExposeSecret;
use tokio::io::AsyncWriteExt as _;

use super::{
    CONFIG_DIR_NAME, ETC_SECRET_ROOT, ETC_SERVICE_ROOT, LOG_SERVICE_ROOT, RenderedConfigFile,
    ResolvedSecretFile, VAR_SERVICE_ROOT,
};
use crate::error::{AgentResult, HephaestusAgentError, HephaestusAgentErrorReason};

pub(super) fn service_dir(service_id: &str) -> PathBuf {
    Path::new(ETC_SERVICE_ROOT).join(service_id)
}

pub(super) fn config_dir(service_id: &str) -> PathBuf {
    service_dir(service_id).join(CONFIG_DIR_NAME)
}

pub(super) fn secret_dir(service_id: &str) -> PathBuf {
    Path::new(ETC_SECRET_ROOT).join(service_id)
}

pub(super) fn data_dir(service_id: &str) -> PathBuf {
    Path::new(VAR_SERVICE_ROOT).join(production_data_dir_name(service_id))
}

pub(super) fn docker_auth_config_dir(service_id: &str) -> PathBuf {
    Path::new(VAR_SERVICE_ROOT)
        .join("hephaestus-agent")
        .join("docker-auth")
        .join(service_id)
}

pub(super) fn rollback_dir(service_id: &str) -> PathBuf {
    Path::new(VAR_SERVICE_ROOT)
        .join("hephaestus-agent")
        .join("rollback")
        .join(service_id)
}

pub(super) fn log_dir(service_id: &str) -> PathBuf {
    Path::new(LOG_SERVICE_ROOT).join(service_id)
}

fn production_data_dir_name(service_id: &str) -> &str {
    match service_id {
        // These names intentionally mirror the production infrastructure
        // compose/Ansible defaults. The controller may use a stable internal
        // service id while the host data path remains the workload's canonical
        // operational path.
        "nats" | "reallyme-nats" => "nats",
        "typesense" | "reallyme-typesense" => "typesense",
        _ => service_id,
    }
}

pub(super) async fn write_file_atomic(path: &Path, contents: &[u8]) -> AgentResult<()> {
    write_atomic(path, contents, 0o600).await
}

pub(super) async fn path_exists(path: &Path) -> bool {
    tokio::fs::metadata(path).await.is_ok()
}

pub(super) async fn remove_dir_if_exists(path: &Path) -> AgentResult<()> {
    match tokio::fs::remove_dir_all(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_error) => Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::FileDeleteFailed,
        )),
    }
}

pub(super) async fn copy_dir_recursive(source: &Path, destination: &Path) -> AgentResult<()> {
    let source = source.to_path_buf();
    let destination = destination.to_path_buf();
    tokio::task::spawn_blocking(move || {
        copy_dir_recursive_sync(source.as_path(), destination.as_path())
    })
    .await
    .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::CommandFailed))?
}

fn copy_dir_recursive_sync(source: &Path, destination: &Path) -> AgentResult<()> {
    let metadata = std::fs::symlink_metadata(source)
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::FileReadFailed))?;
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidPath,
        ));
    }

    if file_type.is_dir() {
        std::fs::create_dir_all(destination).map_err(|_error| {
            HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed)
        })?;
        std::fs::set_permissions(destination, metadata.permissions()).map_err(|_error| {
            HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed)
        })?;
        let entries = std::fs::read_dir(source).map_err(|_error| {
            HephaestusAgentError::new(HephaestusAgentErrorReason::FileReadFailed)
        })?;
        for entry in entries {
            let entry = entry.map_err(|_error| {
                HephaestusAgentError::new(HephaestusAgentErrorReason::FileReadFailed)
            })?;
            let file_name = entry.file_name();
            copy_dir_recursive_sync(
                entry.path().as_path(),
                destination.join(file_name).as_path(),
            )?;
        }
        Ok(())
    } else if file_type.is_file() {
        let parent = destination
            .parent()
            .ok_or_else(|| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidPath))?;
        std::fs::create_dir_all(parent).map_err(|_error| {
            HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed)
        })?;
        std::fs::copy(source, destination).map_err(|_error| {
            HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed)
        })?;
        std::fs::set_permissions(destination, metadata.permissions()).map_err(|_error| {
            HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed)
        })
    } else {
        Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidPath,
        ))
    }
}

pub(super) async fn write_rendered_config_files(
    config_dir: &Path,
    files: &[RenderedConfigFile],
) -> AgentResult<()> {
    for file in files {
        let path = config_dir.join(file.relative_path.as_path());
        create_parent_dir(path.as_path()).await?;
        write_config_file_atomic(path.as_path(), file.contents.as_bytes(), file.executable).await?;
    }
    Ok(())
}

pub(super) async fn write_rendered_secret_files(
    secret_dir: &Path,
    files: &[ResolvedSecretFile],
) -> AgentResult<()> {
    for file in files {
        let path = secret_dir.join(file.relative_path.as_path());
        create_parent_dir(path.as_path()).await?;
        write_secret_file_atomic(path.as_path(), file.contents.expose_secret().as_bytes()).await?;
    }
    Ok(())
}

async fn create_parent_dir(path: &Path) -> AgentResult<()> {
    let parent = path
        .parent()
        .ok_or_else(|| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidPath))?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed))
}

async fn write_config_file_atomic(
    path: &Path,
    contents: &[u8],
    executable: bool,
) -> AgentResult<()> {
    write_atomic(path, contents, config_file_mode(executable)).await
}

pub(super) async fn write_secret_file_atomic(path: &Path, contents: &[u8]) -> AgentResult<()> {
    write_atomic(path, contents, 0o600).await
}

async fn write_atomic(path: &Path, contents: &[u8], mode: u32) -> AgentResult<()> {
    let parent = path
        .parent()
        .ok_or_else(|| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidPath))?
        .to_path_buf();
    let temporary = tokio::task::spawn_blocking(move || tempfile::NamedTempFile::new_in(parent))
        .await
        .map_err(|_| write_failed())?
        .map_err(|_| write_failed())?;
    let (file, temporary_path) = temporary.into_parts();
    let mut file = tokio::fs::File::from_std(file);
    file.write_all(contents).await.map_err(|_| write_failed())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(mode))
            .await
            .map_err(|_| write_failed())?;
    }
    #[cfg(not(unix))]
    let _ = mode;
    file.sync_all().await.map_err(|_| write_failed())?;
    drop(file);
    // Keep the commit separate from preparation: cancellation during writing or
    // syncing must drop the temporary file without publishing partial work.
    tokio::fs::rename(&temporary_path, path)
        .await
        .map_err(|_| write_failed())
}

fn write_failed() -> HephaestusAgentError {
    HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed)
}

fn config_file_mode(executable: bool) -> u32 {
    if executable { 0o755 } else { 0o644 }
}

#[cfg(unix)]
pub(super) async fn set_file_permissions(path: &Path, mode: u32) -> AgentResult<()> {
    use std::os::unix::fs::PermissionsExt;

    let permissions = std::fs::Permissions::from_mode(mode);
    tokio::fs::set_permissions(path, permissions)
        .await
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed))
}

#[cfg(not(unix))]
pub(super) async fn set_file_permissions(_path: &Path, _mode: u32) -> AgentResult<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{write_file_atomic, write_secret_file_atomic};

    #[test]
    fn cancellation_during_preparation_does_not_publish_later() {
        use std::future::{Future, poll_fn};
        use std::task::Poll;
        let directory = tempfile::tempdir().expect("directory");
        let destination = directory.path().join("config.json");
        std::fs::write(&destination, b"original").expect("original");
        let runtime = tokio::runtime::Builder::new_current_thread()
            .max_blocking_threads(1)
            .enable_all()
            .build()
            .expect("runtime");
        runtime.block_on(async {
            let (release, blocked) = std::sync::mpsc::channel();
            let (started, ready) = std::sync::mpsc::channel();
            let blocker = tokio::task::spawn_blocking(move || {
                started.send(()).expect("started");
                blocked.recv().expect("release");
            });
            ready.recv().expect("blocking pool occupied");
            let mut write = Box::pin(write_secret_file_atomic(&destination, b"replacement"));
            poll_fn(|cx| {
                assert!(write.as_mut().poll(cx).is_pending());
                Poll::Ready(())
            })
            .await;
            drop(write);
            release.send(()).expect("release blocking pool");
            blocker.await.expect("blocker");
            // With one blocking thread this barrier follows the cancelled work.
            tokio::task::spawn_blocking(|| ())
                .await
                .expect("drain queue");
        });
        assert_eq!(
            std::fs::read(&destination).expect("destination"),
            b"original"
        );
        assert_eq!(
            std::fs::read_dir(directory.path())
                .expect("directory")
                .count(),
            1
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn config_permissions_and_replacement_remain_compatible() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().expect("directory");
        let destination = directory.path().join("config");
        for (executable, expected_mode) in [(false, 0o644), (true, 0o755)] {
            super::write_config_file_atomic(&destination, b"config", executable)
                .await
                .expect("write configuration");
            assert_eq!(
                std::fs::metadata(&destination)
                    .expect("metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                expected_mode
            );
        }
    }

    #[tokio::test]
    async fn atomic_writes_preserve_neighboring_tmp_files() {
        let directory = tempfile::tempdir().expect("directory");
        let neighbor = directory.path().join("config.tmp");
        tokio::fs::write(&neighbor, b"keep")
            .await
            .expect("neighbor");
        let destination = directory.path().join("config.json");
        write_secret_file_atomic(&destination, b"secret")
            .await
            .expect("write secret");
        assert_eq!(
            tokio::fs::read(&neighbor).await.expect("read neighbor"),
            b"keep"
        );
        assert_eq!(
            tokio::fs::read(&destination).await.expect("read result"),
            b"secret"
        );
        write_secret_file_atomic(&neighbor, b"new")
            .await
            .expect("tmp destination");
        assert_eq!(tokio::fs::read(&neighbor).await.expect("read tmp"), b"new");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn atomic_write_does_not_follow_a_preexisting_temp_symlink() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let directory = tempfile::tempdir().expect("directory");
        let victim = directory.path().join("victim");
        tokio::fs::write(&victim, b"keep").await.expect("victim");
        symlink(&victim, directory.path().join("compose.tmp")).expect("symlink");
        let destination = directory.path().join("compose.yml");
        write_file_atomic(&destination, b"configuration")
            .await
            .expect("write");
        assert_eq!(
            tokio::fs::read(&victim).await.expect("victim unchanged"),
            b"keep"
        );
        assert_eq!(
            std::fs::metadata(destination)
                .expect("metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
}
