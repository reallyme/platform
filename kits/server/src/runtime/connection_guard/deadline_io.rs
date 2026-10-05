// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Enforces the first-request deadline before a protocol request is parsed.

use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::time::{Sleep, sleep};

use super::FirstRequestTracker;

pub(super) struct DeadlineIo<T> {
    pub(super) inner: T,
    first_request_deadline: Pin<Box<Sleep>>,
    first_request: FirstRequestTracker,
}

impl<T> DeadlineIo<T> {
    pub(super) fn new(inner: T, first_request: Duration, tracker: FirstRequestTracker) -> Self {
        Self {
            inner,
            first_request_deadline: Box::pin(sleep(first_request)),
            first_request: tracker,
        }
    }

    fn timed_out(&mut self, cx: &mut Context<'_>) -> bool {
        !self.first_request.was_seen() && self.first_request_deadline.as_mut().poll(cx).is_ready()
    }
}

impl<T: AsyncRead + Unpin> AsyncRead for DeadlineIo<T> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if this.timed_out(cx) {
            return Poll::Ready(Err(io::Error::from(io::ErrorKind::TimedOut)));
        }
        Pin::new(&mut this.inner).poll_read(cx, buf)
    }
}

impl<T: AsyncWrite + Unpin> AsyncWrite for DeadlineIo<T> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        if this.timed_out(cx) {
            return Poll::Ready(Err(io::Error::from(io::ErrorKind::TimedOut)));
        }
        Pin::new(&mut this.inner).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if this.timed_out(cx) {
            return Poll::Ready(Err(io::Error::from(io::ErrorKind::TimedOut)));
        }
        Pin::new(&mut this.inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}
