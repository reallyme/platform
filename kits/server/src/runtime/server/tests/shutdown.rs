// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use super::fixtures::{
    send_sigterm, spawn_stdout_lock_regression_worker, stdout_lock_regression_action,
    stdout_lock_regression_port, stdout_lock_regression_runtime, terminate_child_for_cleanup,
    unused_local_port, wait_for_child_output, wait_for_http_response,
};

#[cfg(unix)]
#[test]
fn second_process_signal_forces_a_stuck_drain_to_exit() {
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};
    use std::sync::mpsc;

    let executable = std::env::current_exe().expect("test executable should resolve");
    let mut child = Command::new(executable)
        .arg("--exact")
        .arg("runtime::server::tests::shutdown::signal_escalation_subprocess_worker")
        .arg("--nocapture")
        .env_clear()
        .env("REALLYME_TEST_SIGNAL_ESCALATION", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("signal regression worker should launch");
    let stdout = child.stdout.take().expect("worker stdout should be piped");
    let (ready_sender, ready_receiver) = mpsc::channel();
    let _reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if line.contains("server process ready") {
                let _ = ready_sender.send(());
                return;
            }
        }
    });
    if ready_receiver.recv_timeout(Duration::from_secs(5)).is_err() {
        let _ = child.kill();
        let output = child
            .wait_with_output()
            .expect("signal worker output should be readable");
        panic!(
            "signal regression worker did not reach readiness; stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    send_sigterm(&mut child);
    std::thread::sleep(Duration::from_millis(100));
    send_sigterm(&mut child);
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let status = loop {
        match child.try_wait().expect("worker status should be readable") {
            Some(status) => break status,
            None if std::time::Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(20));
            }
            None => {
                let _ = child.kill();
                panic!("second signal did not terminate the worker");
            }
        }
    };
    assert_eq!(status.code(), Some(1));
}

#[cfg(unix)]
#[test]
fn signal_escalation_subprocess_worker() {
    if std::env::var_os("REALLYME_TEST_SIGNAL_ESCALATION").is_none() {
        return;
    }
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("worker runtime should build")
        .block_on(async {
            let server_name = crate::startup::ServerName::new("signal-escalation")
                .expect("valid fixture server name");
            let runtime = crate::runtime::ServerRuntime::builder()
                .server_name(server_name.clone())
                .observability_config(super::fixtures::observability_config())
                .build_info(crate::version::BuildInfo::new(
                    server_name,
                    env!("CARGO_PKG_VERSION"),
                ))
                .readiness(crate::health::Readiness::new())
                .shutdown_timeout(
                    crate::task::ShutdownTimeout::new(Duration::from_secs(1))
                        .expect("valid fixture shutdown timeout"),
                )
                .build()
                .expect("signal worker runtime should build");
            runtime
                .run()
                .await
                .expect("a second signal should exit first");
        });
}

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
