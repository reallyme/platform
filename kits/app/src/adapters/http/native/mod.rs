// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded native HTTPS transport for sensitive application adapters.

mod client;
mod error;
mod origin;
mod request;
mod response;

pub use client::BoundedHttpsClient;
pub use error::{HttpsDispatchOutcome, HttpsTransportError, HttpsTransportErrorReason};
pub use origin::HttpsOrigin;
pub use request::{BoundedHttpsRequest, CapturedResponseHeader, HttpsExchangeLimits, HttpsMethod};
pub use response::BoundedHttpsResponse;
