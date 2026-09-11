// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use super::fixtures::{
    send_sigterm, spawn_stdout_lock_regression_worker, stdout_lock_regression_action,
    stdout_lock_regression_port, stdout_lock_regression_runtime, terminate_child_for_cleanup,
    unused_local_port, wait_for_child_output, wait_for_http_response,
};

#[test]
fn startup_banner_stdout_lock_is_not_held_across_runtime_lifetime() {
    let Some(port) = unused_local_port() else {
        return;
    };
    let mut child = spawn_stdout_lock_regression_worker(port);

    let response = match wait_for_http_response(port, "/hello", Duration::from_secs(5)) {
        Ok(response) => response,
        Err(error) => {
            terminate_child_for_cleanup(&mut child);
            let output = wait_for_child_output(child, Duration::from_secs(5));
            panic!(
                "runtime worker did not serve request: {error}\nstdout:\n{}\nstderr:\n{}",
                output.stdout, output.stderr
            );
        }
    };

    assert!(
        response.starts_with("HTTP/1.1 200 OK"),
        "unexpected response: {response}"
    );
    assert!(
        response.contains("hello from runtime stdout lock regression"),
        "missing response body: {response}"
    );

    send_sigterm(&mut child);
    let output = wait_for_child_output(child, Duration::from_secs(5));

    assert!(
        matches!(output.status, Some(status) if status.success()),
        "runtime worker did not shut down cleanly\nstatus: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        output.stdout,
        output.stderr,
    );
    assert!(
        output.stdout.contains("shutdown completed"),
        "shutdown completion log missing\nstdout:\n{}\nstderr:\n{}",
        output.stdout,
        output.stderr,
    );
}

#[test]
fn stdout_lock_regression_subprocess_worker() {
    let Some(action) = stdout_lock_regression_action() else {
        return;
    };
    assert_eq!(action.to_string_lossy(), "run-runtime");
    let port = stdout_lock_regression_port();

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("worker tokio runtime should build")
        .block_on(async move {
            let runtime = stdout_lock_regression_runtime(port)
                .expect("worker runtime should build from valid fixtures");
            runtime.run().await.expect("worker runtime should run");
        });
}
