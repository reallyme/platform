// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounds time spent blocked while writing an HTTP connection.

use std::future::Future;
use std::io::{self, IoSlice};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::time::{Sleep, sleep};

use super::activity::ConnectionActivity;

pub(super) struct WriteProgressIo<T> {
    inner: T,
    stall_timeout: Duration,
    pending_deadline: Option<(PendingOperation, Pin<Box<Sleep>>)>,
    activity: Option<ConnectionActivity>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PendingOperation {
    Write,
    Flush,
    Shutdown,
}

impl<T> WriteProgressIo<T> {
    pub(super) fn new(inner: T, stall_timeout: Duration) -> Self {
        Self {
            inner,
            stall_timeout,
            pending_deadline: None,
            activity: None,
        }
    }

    pub(super) fn with_activity(mut self, activity: ConnectionActivity) -> Self {
        self.activity = Some(activity);
        self
    }

    fn timed_out(&mut self, cx: &mut Context<'_>) -> bool {
        self.pending_deadline
            .as_mut()
            .is_some_and(|(_, deadline)| deadline.as_mut().poll(cx).is_ready())
    }

    fn on_pending<U>(
        &mut self,
        cx: &mut Context<'_>,
        operation: PendingOperation,
    ) -> Poll<io::Result<U>> {
        let deadline = self
            .pending_deadline
            .get_or_insert_with(|| (operation, Box::pin(sleep(self.stall_timeout))));
        if deadline.1.as_mut().poll(cx).is_ready() {
            Poll::Ready(Err(io::ErrorKind::TimedOut.into()))
        } else {
            Poll::Pending
        }
    }

    fn clear_completed_operation(&mut self, operation: PendingOperation) {
        if self
            .pending_deadline
            .as_ref()
            .is_some_and(|(pending, _)| *pending == operation)
        {
            self.pending_deadline = None;
        }
    }
}

impl<T: AsyncRead + Unpin> AsyncRead for WriteProgressIo<T> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        let before = buffer.filled().len();
        let result = Pin::new(&mut this.inner).poll_read(cx, buffer);
        if let Poll::Ready(Ok(())) = &result
            && let Some(activity) = &this.activity
            && let Some(read) = buffer.filled().len().checked_sub(before)
        {
            activity.record_read_progress(read);
        }
        result
    }
}

impl<T: AsyncWrite + Unpin> AsyncWrite for WriteProgressIo<T> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        if this.timed_out(cx) {
            return Poll::Ready(Err(io::ErrorKind::TimedOut.into()));
        }
        match Pin::new(&mut this.inner).poll_write(cx, buffer) {
            Poll::Pending => this.on_pending(cx, PendingOperation::Write),
            Poll::Ready(Ok(written)) if written > 0 || buffer.is_empty() => {
                this.pending_deadline = None;
                if written > 0
                    && let Some(activity) = &this.activity
                {
                    activity.record_write_progress(written);
                }
                Poll::Ready(Ok(written))
            }
            Poll::Ready(Ok(_)) => Poll::Ready(Err(io::ErrorKind::WriteZero.into())),
            Poll::Ready(Err(error)) => Poll::Ready(Err(error)),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if this.timed_out(cx) {
            return Poll::Ready(Err(io::ErrorKind::TimedOut.into()));
        }
        match Pin::new(&mut this.inner).poll_flush(cx) {
            Poll::Pending => this.on_pending(cx, PendingOperation::Flush),
            Poll::Ready(Ok(())) => {
                this.clear_completed_operation(PendingOperation::Flush);
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(error)) => Poll::Ready(Err(error)),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if this.timed_out(cx) {
            return Poll::Ready(Err(io::ErrorKind::TimedOut.into()));
        }
        match Pin::new(&mut this.inner).poll_shutdown(cx) {
            Poll::Pending => this.on_pending(cx, PendingOperation::Shutdown),
            Poll::Ready(Ok(())) => {
                this.clear_completed_operation(PendingOperation::Shutdown);
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(error)) => Poll::Ready(Err(error)),
        }
    }

    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffers: &[IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        if this.timed_out(cx) {
            return Poll::Ready(Err(io::ErrorKind::TimedOut.into()));
        }
        match Pin::new(&mut this.inner).poll_write_vectored(cx, buffers) {
            Poll::Pending => this.on_pending(cx, PendingOperation::Write),
            Poll::Ready(Ok(written)) if written > 0 || buffers.iter().all(|buf| buf.is_empty()) => {
                this.pending_deadline = None;
                if written > 0
                    && let Some(activity) = &this.activity
                {
                    activity.record_write_progress(written);
                }
                Poll::Ready(Ok(written))
            }
            Poll::Ready(Ok(_)) => Poll::Ready(Err(io::ErrorKind::WriteZero.into())),
            Poll::Ready(Err(error)) => Poll::Ready(Err(error)),
        }
    }
}

#[cfg(test)]
#[path = "write_progress_tests.rs"]
mod tests;
