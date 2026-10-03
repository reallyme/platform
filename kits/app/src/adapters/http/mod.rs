// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! HTTP adapter conventions.

mod conventions;
#[cfg(feature = "native-http")]
mod native;

pub use conventions::HttpAdapterConventions;
#[cfg(feature = "native-http")]
pub use native::{
    BoundedHttpsClient, BoundedHttpsRequest, BoundedHttpsResponse, CapturedResponseHeader,
    HttpsDispatchOutcome, HttpsExchangeLimits, HttpsMethod, HttpsOrigin, HttpsTransportError,
    HttpsTransportErrorReason,
};
