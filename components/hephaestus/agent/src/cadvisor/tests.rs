// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{MAX_CADVISOR_METRICS_BYTES, collect_cadvisor_metrics, parse_cadvisor_metrics};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::Duration;
use url::Url;

#[test]
fn parses_core_cadvisor_signals() {
    let snapshot = parse_cadvisor_metrics(
        r#"
# HELP container_cpu_usage_seconds_total CPU
container_cpu_usage_seconds_total{id="/docker/abc"} 1
container_memory_usage_bytes{id="/docker/abc"} 2
"#,
    );

    assert!(snapshot.has_cpu_metrics());
    assert!(snapshot.has_memory_metrics());
    assert_eq!(snapshot.container_metric_lines(), 2);
}

#[tokio::test]
async fn rejects_cadvisor_metrics_body_above_limit_while_streaming() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap_or_else(|error| panic!("listener binds: {error:?}"));
    let address = listener
        .local_addr()
        .unwrap_or_else(|error| panic!("listener address: {error:?}"));
    let server = tokio::spawn(async move {
        let (mut stream, _peer) = listener
            .accept()
            .await
            .unwrap_or_else(|error| panic!("accept request: {error:?}"));
        let mut request = [0u8; 512];
        let _bytes_read = stream
            .read(&mut request)
            .await
            .unwrap_or_else(|error| panic!("read request: {error:?}"));
        let content_length = MAX_CADVISOR_METRICS_BYTES
            .checked_add(1)
            .unwrap_or_else(|| panic!("test size does not overflow"));
        let header = format!("HTTP/1.1 200 OK\r\nContent-Length: {content_length}\r\n\r\n");
        stream
            .write_all(header.as_bytes())
            .await
            .unwrap_or_else(|error| panic!("write response header: {error:?}"));
        stream
            .write_all(&vec![b'a'; content_length])
            .await
            .unwrap_or_else(|error| panic!("write response body: {error:?}"));
    });
    let url = Url::parse(format!("http://{address}/metrics").as_str())
        .unwrap_or_else(|error| panic!("valid url: {error:?}"));

    let result = collect_cadvisor_metrics(&url, Duration::from_secs(5)).await;

    assert!(result.is_err());
    server
        .await
        .unwrap_or_else(|error| panic!("server joins: {error:?}"));
}
