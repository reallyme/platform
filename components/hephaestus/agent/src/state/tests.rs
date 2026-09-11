// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::error::HephaestusAgentErrorReason;
use reallyme_hephaestus_domain::HephaestusAgentRuntimeToken;

use super::AgentTokenStore;

#[tokio::test]
async fn writes_and_reads_runtime_token() {
    let dir = tempfile::tempdir().expect("tempdir");
    let bootstrap_path = dir.path().join("bootstrap-token");
    let store = AgentTokenStore::new(dir.path(), bootstrap_path.as_path()).expect("store");
    let token = HephaestusAgentRuntimeToken::new("runtime-token-1234").expect("token");

    store.write_runtime_token(&token).await.expect("write");
    let loaded = store
        .read_runtime_token()
        .await
        .expect("read")
        .expect("present");

    assert!(loaded.constant_time_eq(&token));
}

#[tokio::test]
async fn writes_runtime_token_with_restrictive_permissions() {
    let dir = tempfile::tempdir().expect("tempdir");
    let bootstrap_path = dir.path().join("bootstrap-token");
    let store = AgentTokenStore::new(dir.path(), bootstrap_path.as_path()).expect("store");
    let token = HephaestusAgentRuntimeToken::new("runtime-token-1234").expect("token");

    store.write_runtime_token(&token).await.expect("write");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let metadata = tokio::fs::metadata(dir.path().join("runtime-token").as_path())
            .await
            .expect("metadata");
        assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
    }
}

#[tokio::test]
async fn rejects_stub_runtime_token_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let bootstrap_path = dir.path().join("bootstrap-token");
    let store = AgentTokenStore::new(dir.path(), bootstrap_path.as_path()).expect("store");
    tokio::fs::write(dir.path().join("runtime-token").as_path(), b"stub\n")
        .await
        .expect("runtime token write");

    let loaded = store.read_runtime_token().await;

    assert_eq!(
        loaded.err().map(|error| error.reason()),
        Some(HephaestusAgentErrorReason::RuntimeTokenUnavailable)
    );
}

#[tokio::test]
async fn creates_and_reuses_identity_key() {
    let dir = tempfile::tempdir().expect("tempdir");
    let bootstrap_path = dir.path().join("bootstrap-token");
    let store = AgentTokenStore::new(dir.path(), bootstrap_path.as_path()).expect("store");

    let first = store
        .load_or_create_identity_key()
        .await
        .expect("first key");
    let second = store
        .load_or_create_identity_key()
        .await
        .expect("second key");

    assert_eq!(first.public_key_bytes(), second.public_key_bytes());
}

#[tokio::test]
async fn concurrent_identity_creation_does_not_clobber_winner() {
    let dir = tempfile::tempdir().expect("tempdir");
    let bootstrap_path = dir.path().join("bootstrap-token");
    let store = AgentTokenStore::new(dir.path(), bootstrap_path.as_path()).expect("store");
    let mut tasks = Vec::new();
    for _index in 0..8 {
        let task_store = store.clone();
        tasks.push(tokio::spawn(async move {
            task_store.load_or_create_identity_key().await
        }));
    }

    let mut keys = Vec::new();
    for task in tasks {
        keys.push(task.await.expect("identity task").expect("identity key"));
    }

    let first = keys.first().expect("at least one identity key");
    for key in keys.iter().skip(1) {
        assert_eq!(first.public_key_bytes(), key.public_key_bytes());
    }
}

#[tokio::test]
async fn writes_and_reads_observed_generation() {
    let dir = tempfile::tempdir().expect("tempdir");
    let bootstrap_path = dir.path().join("bootstrap-token");
    let store = AgentTokenStore::new(dir.path(), bootstrap_path.as_path()).expect("store");

    store
        .write_observed_generation(1_779_540_157)
        .await
        .expect("generation write");
    let loaded = store
        .read_observed_generation()
        .await
        .expect("generation read");

    assert_eq!(loaded, Some(1_779_540_157));
}

#[tokio::test]
async fn rejects_observed_generation_above_policy_cap() {
    let dir = tempfile::tempdir().expect("tempdir");
    let bootstrap_path = dir.path().join("bootstrap-token");
    let store = AgentTokenStore::new(dir.path(), bootstrap_path.as_path()).expect("store");

    tokio::fs::write(
        dir.path().join("observed-generation").as_path(),
        (super::MAX_OBSERVED_GENERATION + 1).to_string().as_bytes(),
    )
    .await
    .expect("write huge observed generation");

    let loaded = store.read_observed_generation().await;

    assert_eq!(
        loaded.err().map(|error| error.reason()),
        Some(crate::error::HephaestusAgentErrorReason::InvalidConfig)
    );
}

#[tokio::test]
async fn deletes_bootstrap_token_after_registration() {
    let dir = tempfile::tempdir().expect("tempdir");
    let bootstrap_path = dir.path().join("bootstrap-token");
    tokio::fs::write(bootstrap_path.as_path(), b"bootstrap-token-1\n")
        .await
        .expect("bootstrap write");
    let store = AgentTokenStore::new(dir.path(), bootstrap_path.as_path()).expect("store");

    store
        .delete_bootstrap_token()
        .await
        .expect("bootstrap delete");

    assert!(!bootstrap_path.exists());
}

#[tokio::test]
async fn successful_registration_persistence_order_keeps_bootstrap_until_runtime_state_is_durable()
{
    let dir = tempfile::tempdir().expect("tempdir");
    let bootstrap_path = dir.path().join("bootstrap-token");
    tokio::fs::write(bootstrap_path.as_path(), b"bootstrap-token-1\n")
        .await
        .expect("bootstrap write");
    let store = AgentTokenStore::new(dir.path(), bootstrap_path.as_path()).expect("store");
    let token = HephaestusAgentRuntimeToken::new("runtime-token-1234").expect("token");

    store
        .write_runtime_token(&token)
        .await
        .expect("runtime write");
    assert!(
        bootstrap_path.exists(),
        "bootstrap token must remain until all registration state is durable"
    );

    store
        .write_observed_generation(1_779_575_254)
        .await
        .expect("generation write");
    assert!(
        bootstrap_path.exists(),
        "bootstrap token must remain after runtime token write until observed generation is durable"
    );

    store
        .delete_bootstrap_token()
        .await
        .expect("bootstrap delete");
    assert!(!bootstrap_path.exists());
}

#[tokio::test]
async fn failed_registration_state_write_keeps_bootstrap_token_for_retry() {
    let dir = tempfile::tempdir().expect("tempdir");
    let bootstrap_path = dir.path().join("bootstrap-token");
    tokio::fs::write(bootstrap_path.as_path(), b"bootstrap-token-1\n")
        .await
        .expect("bootstrap write");
    let store = AgentTokenStore::new(dir.path(), bootstrap_path.as_path()).expect("store");

    let invalid_token = HephaestusAgentRuntimeToken::new("short").expect("domain token");
    let result = store.write_runtime_token(&invalid_token).await;

    assert_eq!(
        result.err().map(|error| error.reason()),
        Some(HephaestusAgentErrorReason::RuntimeTokenUnavailable)
    );
    assert!(
        bootstrap_path.exists(),
        "bootstrap token must remain when runtime token validation/write fails"
    );
}

#[tokio::test]
async fn removes_orphaned_identity_tmp_file_before_key_creation() {
    let dir = tempfile::tempdir().expect("tempdir");
    let bootstrap_path = dir.path().join("bootstrap-token");
    let store = AgentTokenStore::new(dir.path(), bootstrap_path.as_path()).expect("store");
    let stale_tmp_name = format!(
        "{}.{}.{}.tmp",
        "ed25519-private-key",
        std::process::id(),
        123456_u128
    );
    let stale_tmp_path = dir.path().join(&stale_tmp_name);

    tokio::fs::write(stale_tmp_path.as_path(), b"orphaned-seed")
        .await
        .expect("write stale tmp");
    tokio::fs::write(
        dir.path().join("ed25519-private-key.abc-123.tmp"),
        b"invalid",
    )
    .await
    .expect("write invalid tmp");

    store
        .load_or_create_identity_key()
        .await
        .expect("identity key");

    assert!(!stale_tmp_path.exists());
    assert!(
        dir.path().join("ed25519-private-key.abc-123.tmp").exists(),
        "non-matching tmp files should remain"
    );
}

#[test]
fn rejects_orphaned_identity_tmp_file_with_overlong_components() {
    let max_component_name = "9".repeat(21);
    let mut path = std::path::PathBuf::new();
    path.push(format!(
        "ed25519-private-key.{max_component_name}.{max_component_name}.tmp"
    ));

    let result =
        super::is_orphaned_identity_tmp_file(path.as_path()).expect("tmp filename validation");

    assert!(
        !result,
        "tmp filename components longer than u64::MAX width should be rejected"
    );
}

#[tokio::test]
async fn rejects_non_regular_and_oversized_secret_files() {
    let dir = tempfile::tempdir().expect("directory");
    assert!(super::read_secret_file(dir.path()).await.is_err());
    let path = dir.path().join("oversized");
    tokio::fs::write(&path, vec![b'x'; super::MAX_RUNTIME_TOKEN_BYTES + 1])
        .await
        .expect("fixture");
    assert!(super::read_secret_file(&path).await.is_err());
    tokio::fs::write(&path, [0xff, 0xfe])
        .await
        .expect("invalid UTF-8");
    assert!(super::read_secret_file(&path).await.is_err());
}

#[tokio::test]
async fn cleanup_cannot_remove_a_prepared_identity_key() {
    let directory = tempfile::tempdir().expect("directory");
    let store =
        AgentTokenStore::new(directory.path(), &directory.path().join("bootstrap")).expect("store");
    let temporary = super::prepare_identity_key_file(directory.path(), b"seed")
        .await
        .expect("prepared key");
    store
        .cleanup_orphaned_identity_key_tmp_files()
        .await
        .expect("cleanup");
    assert_eq!(
        tokio::fs::read(&temporary).await.expect("active key"),
        b"seed"
    );
    let path = temporary.to_path_buf();
    drop(temporary);
    assert!(!path.exists());
}

#[cfg(unix)]
#[tokio::test]
async fn secret_reader_rejects_a_fifo_without_waiting_for_a_writer() {
    let directory = tempfile::tempdir().expect("directory");
    let path = directory.path().join("fifo");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .expect("mkfifo")
            .success()
    );
    let mut read = Box::pin(super::read_secret_file(&path));
    let blocked = tokio::select! {
        result = &mut read => { assert!(result.is_err()); false },
        _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => true,
    };
    if blocked {
        // Unblock a regressed reader before failing so the runtime can shut down.
        let (_, writer) = tokio::join!(read, async {
            tokio::fs::OpenOptions::new().write(true).open(&path).await
        });
        drop(writer);
    }
    assert!(
        !blocked,
        "reader opened a FIFO before rejecting its file type"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn secret_reader_preserves_regular_file_symlink_support() {
    use secrecy::ExposeSecret;
    let directory = tempfile::tempdir().expect("directory");
    let target = directory.path().join("token");
    let link = directory.path().join("token-link");
    tokio::fs::write(&target, b"runtime-token-1234\r\n")
        .await
        .expect("token");
    std::os::unix::fs::symlink(&target, &link).expect("symlink");
    let token = super::read_secret_file(&link)
        .await
        .expect("symlinked token");
    assert_eq!(token.expose_secret(), "runtime-token-1234");
}
