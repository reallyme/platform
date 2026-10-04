// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::WriteProgressIo;

struct NeverWritable;

impl tokio::io::AsyncWrite for NeverWritable {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        _buffer: &[u8],
    ) -> Poll<io::Result<usize>> {
        Poll::Pending
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

#[tokio::test]
async fn stalled_write_times_out_without_a_reader() {
    let (writer, _reader) = tokio::io::duplex(1);
    let mut writer = WriteProgressIo::new(writer, Duration::from_millis(20));
    writer.write_all(b"a").await.expect("first byte fits");
    let error = writer
        .write_all(b"b")
        .await
        .expect_err("full pipe should stop making write progress");
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
}

#[tokio::test]
async fn successful_write_clears_the_previous_stall_deadline() {
    let (writer, mut reader) = tokio::io::duplex(1);
    let mut writer = WriteProgressIo::new(writer, Duration::from_millis(100));
    writer.write_all(b"a").await.expect("first byte fits");
    let read = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(20)).await;
        let mut bytes = [0_u8; 3];
        reader
            .read_exact(&mut bytes[..1])
            .await
            .expect("first byte");
        reader
            .read_exact(&mut bytes[1..2])
            .await
            .expect("second byte");
        tokio::time::sleep(Duration::from_millis(150)).await;
        reader
            .read_exact(&mut bytes[2..])
            .await
            .expect("third byte");
        bytes
    });
    writer.write_all(b"b").await.expect("write resumes");
    tokio::time::sleep(Duration::from_millis(130)).await;
    writer
        .write_all(b"c")
        .await
        .expect("prior stall deadline was cleared");
    assert_eq!(read.await.expect("read joins"), *b"abc");
}

#[tokio::test]
async fn an_immediate_flush_cannot_reset_a_stalled_write() {
    let mut writer = WriteProgressIo::new(NeverWritable, Duration::from_millis(20));
    assert!(
        tokio::time::timeout(Duration::from_millis(5), writer.write_all(b"x"))
            .await
            .is_err()
    );
    writer.flush().await.expect("empty flush succeeds");
    tokio::time::sleep(Duration::from_millis(30)).await;
    let error = writer
        .write_all(b"x")
        .await
        .expect_err("write remains stalled");
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
}
