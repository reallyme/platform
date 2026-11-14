// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fs;
use std::path::Path;
use std::process::{Child, Command as Process, Output, Stdio};
use std::time::{Duration, Instant};

use foundationdb::{Database, options::TransactionOption, tenant::TenantManagement};

use crate::{
    DeleteRecord, DeleteRecordState, FdbConfig, FdbContext, FoundationDbTenantName,
    LockedDeleteRecord, admin, persist_delete_intent,
};

const NAME: &str = "kit-durable-delete-test";
const SCHEMA: &[u8] = b"__meta/v1/schema_version";
const CREATED_AT: &[u8] = b"__meta/v1/created_at";
const TIMEOUT: Duration = Duration::from_secs(20);
const CHILD_TEST: &str = "tests::live::crash_fixture_process";
const FDB_NOT_COMMITTED: i32 = 1020;

fn connect() -> FdbContext {
    let config = FdbConfig::from_env().expect("live FoundationDB config");
    // SAFETY: Each test process initializes the client once. The parent drops
    // its connector normally and the child intentionally simulates a crash.
    #[allow(unsafe_code)]
    unsafe { FdbContext::connect(&config) }.expect("live FoundationDB connector")
}

fn raw_database() -> Database {
    let config = FdbConfig::from_env().expect("live FoundationDB config");
    Database::new(config.fdb_cluster_file()).expect("raw database for fixtures")
}

fn tenant_name() -> FoundationDbTenantName {
    FoundationDbTenantName::new(NAME).expect("valid fixed tenant name")
}

async fn clear_metadata(database: &Database) {
    let handle = database
        .open_tenant(NAME.as_bytes())
        .expect("fixture tenant");
    let trx = handle.create_trx().expect("clear transaction");
    trx.clear(SCHEMA);
    trx.clear(CREATED_AT);
    trx.commit().await.expect("commit metadata clear");
}

#[tokio::test]
#[ignore = "subprocess fixture for the live crash regression"]
async fn crash_fixture_process() {
    let path = std::env::var_os("TEST_DELETE_RECORD").expect("fixture record path");
    let phase = std::env::var("TEST_DELETE_CRASH_PHASE").expect("fixture crash phase");
    assert!(matches!(phase.as_str(), "intent" | "clear" | "delete"));
    let ready = std::env::var_os("TEST_DELETE_READY").expect("fixture readiness file");
    let connector = connect();
    let file = LockedDeleteRecord::open(Path::new(&path)).expect("lock record");
    let record = persist_delete_intent(&connector, tenant_name(), &file)
        .await
        .expect("persist real CLI intent");
    if phase == "clear" {
        clear_metadata(&raw_database()).await;
    } else if phase == "delete" {
        admin::delete_recorded_tenant(&connector, tenant_name(), record.original_id)
            .await
            .expect("commit real CLI deletion");
    }
    fs::write(ready, b"ready").expect("signal selected crash point");
    // Keep the lock and connection alive until the parent kills this process.
    std::future::pending::<()>().await;
}

struct ChildProcess(Child);

impl Drop for ChildProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

async fn crash_after(path: &Path, phase: &str, ready: &Path) {
    let mut child = ChildProcess(
        Process::new(std::env::current_exe().expect("test binary"))
            .args(["--ignored", "--exact", CHILD_TEST])
            .env("TEST_DELETE_RECORD", path)
            .env("TEST_DELETE_CRASH_PHASE", phase)
            .env("TEST_DELETE_READY", ready)
            .spawn()
            .expect("spawn worker"),
    );
    let deadline = Instant::now()
        .checked_add(TIMEOUT)
        .expect("fixture deadline");
    while !ready.exists() {
        assert!(
            child.0.try_wait().expect("inspect worker").is_none(),
            "worker exited before reaching {phase}"
        );
        assert!(Instant::now() < deadline, "worker did not reach {phase}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let blocked = cli(&["resume-delete"], Some(path)).await;
    assert!(!blocked.status.success());
    assert!(String::from_utf8_lossy(&blocked.stderr).contains("holds the record lock"));
    child.0.kill().expect("kill worker without cleanup");
    assert!(!child.0.wait().expect("reap crashed worker").success());
    let file = LockedDeleteRecord::open(path).expect("lock released after crash");
    assert!(file.load().expect("read durable intent").state == DeleteRecordState::Pending);
}

async fn cli(args: &[&str], path: Option<&Path>) -> Output {
    let executable =
        std::env::var_os("FDB_TENANT_ADMIN_TEST_BIN").expect("path to built operator CLI");
    let mut command = Process::new(executable);
    command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(path) = path {
        command.arg(path);
    }
    let mut child = ChildProcess(command.spawn().expect("spawn CLI"));
    let deadline = Instant::now()
        .checked_add(TIMEOUT)
        .expect("fixture deadline");
    while child.0.try_wait().expect("poll CLI").is_none() {
        assert!(Instant::now() < deadline, "CLI invocation timed out");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    use std::io::Read;
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    child
        .0
        .stdout
        .take()
        .expect("CLI stdout")
        .read_to_end(&mut stdout)
        .expect("read CLI stdout");
    child
        .0
        .stderr
        .take()
        .expect("CLI stderr")
        .read_to_end(&mut stderr)
        .expect("read CLI stderr");
    Output {
        status: child.0.wait().expect("CLI exit status"),
        stdout,
        stderr,
    }
}

async fn expect_cli(args: &[&str], path: Option<&Path>, stdout: &[u8]) {
    let result = cli(args, path).await;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, stdout);
}

#[tokio::test]
#[ignore = "requires a live throwaway FoundationDB cluster and the built operator CLI"]
async fn durable_delete_survives_crashes_late_clear_and_name_reuse() {
    let connector = connect();
    let database = raw_database();
    let tenant = tenant_name();
    let directory = tempfile::tempdir().expect("record directory");
    let pending = directory.path().join("before-clear");
    admin::ensure_tenant(&connector, tenant)
        .await
        .expect("provision original");
    crash_after(&pending, "intent", &directory.path().join("intent-ready")).await;

    // Hold a real clear transaction with a non-snapshot read made before the
    // resumed CLI writes the same two values. Its eventual commit must fail.
    let handle = database
        .open_tenant(NAME.as_bytes())
        .expect("open original");
    let trx = handle.create_trx().expect("stage late clear");
    trx.set_option(TransactionOption::Timeout(10_000))
        .expect("bound clear transaction");
    let original_timestamp = trx
        .get(CREATED_AT, false)
        .await
        .expect("read creation time")
        .expect("creation time exists")
        .as_ref()
        .to_vec();
    assert!(trx.get(SCHEMA, false).await.expect("read schema").is_some());
    trx.clear(SCHEMA);
    trx.clear(CREATED_AT);
    expect_cli(&["resume-delete"], Some(&pending), b"metadata-safe\n").await;
    assert_eq!(
        trx.commit()
            .await
            .expect_err("pending clear conflicts")
            .code(),
        FDB_NOT_COMMITTED
    );
    let trx = handle.create_trx().expect("check preserved timestamp");
    assert_eq!(
        trx.get(CREATED_AT, false)
            .await
            .expect("read timestamp")
            .expect("timestamp remains")
            .as_ref(),
        &original_timestamp
    );

    let pending = directory.path().join("after-clear");
    crash_after(&pending, "clear", &directory.path().join("clear-ready")).await;
    assert!(connector.open_tenant(tenant).await.is_err());
    expect_cli(&["resume-delete"], Some(&pending), b"metadata-safe\n").await;
    connector
        .open_tenant(tenant)
        .await
        .expect("repair restores metadata");

    let pending = directory.path().join("after-delete");
    let original_id = admin::tenant_id(&connector, tenant)
        .await
        .expect("original lookup");
    crash_after(&pending, "delete", &directory.path().join("delete-ready")).await;
    TenantManagement::create_tenant(&database, NAME.as_bytes())
        .await
        .expect("create empty replacement");
    let replacement_id = admin::tenant_id(&connector, tenant)
        .await
        .expect("replacement lookup");
    assert_ne!(original_id, replacement_id);
    expect_cli(&["resume-delete"], Some(&pending), b"original-absent\n").await;
    assert_eq!(
        admin::tenant_id(&connector, tenant)
            .await
            .expect("replacement still present"),
        replacement_id
    );
    let replacement = database
        .open_tenant(NAME.as_bytes())
        .expect("open replacement");
    let read = replacement.create_trx().expect("read replacement");
    assert!(
        read.get(SCHEMA, false)
            .await
            .expect("read replacement schema")
            .is_none()
    );
    assert!(
        read.get(CREATED_AT, false)
            .await
            .expect("read replacement timestamp")
            .is_none()
    );
    expect_cli(&["resume-delete"], Some(&pending), b"original-absent\n").await;

    let wrong_cluster = directory.path().join("wrong-cluster");
    let uid = admin::tenant_admin_cluster_id(&connector)
        .await
        .expect("cluster UID");
    let mut other_uid = *uid.as_bytes();
    other_uid[0] = if other_uid[0] == b'0' { b'1' } else { b'0' };
    let locked = LockedDeleteRecord::open(&wrong_cluster).expect("lock wrong-cluster fixture");
    locked
        .create(DeleteRecord {
            cluster: admin::TenantAdminClusterId::from_bytes(&other_uid).expect("other valid UID"),
            tenant,
            original_id: replacement_id.expect("replacement ID"),
            state: DeleteRecordState::Pending,
        })
        .expect("persist record scoped to another cluster");
    drop(locked);
    let rejected = cli(&["resume-delete"], Some(&wrong_cluster)).await;
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("different FoundationDB cluster"));
    assert!(
        read.get(SCHEMA, false)
            .await
            .expect("read replacement schema")
            .is_none()
    );

    let incompatible = directory.path().join("incompatible-metadata");
    crash_after(
        &incompatible,
        "intent",
        &directory.path().join("incompatible-ready"),
    )
    .await;
    let write = replacement.create_trx().expect("write incomplete metadata");
    write.set(SCHEMA, &1_u32.to_le_bytes());
    write.commit().await.expect("commit incomplete metadata");
    let rejected = cli(&["resume-delete"], Some(&incompatible)).await;
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("record remains pending"));
    let locked = LockedDeleteRecord::open(&incompatible).expect("open incompatible record");
    assert!(locked.load().expect("load incompatible record").state == DeleteRecordState::Pending);
    let read = replacement.create_trx().expect("check partial metadata");
    assert!(
        read.get(SCHEMA, false)
            .await
            .expect("read schema")
            .is_some()
    );
    assert!(
        read.get(CREATED_AT, false)
            .await
            .expect("read missing timestamp")
            .is_none()
    );

    let complete = directory.path().join("completed");
    expect_cli(&["delete", NAME, "--record"], Some(&complete), b"").await;
    expect_cli(&["resume-delete"], Some(&complete), b"original-absent\n").await;
    assert_eq!(
        admin::tenant_id(&connector, tenant)
            .await
            .expect("replacement deleted"),
        None
    );
}
