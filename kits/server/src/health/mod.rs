// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Readiness and liveness state primitives.
//!
//! # Examples
//!
//! ```rust
//! use reallyme_server_kit::health::{
//!     LivenessState, Readiness, ReadinessState, liveness_check, readiness_check,
//! };
//!
//! let readiness = Readiness::new();
//! assert!(!readiness.is_ready());
//! assert_eq!(readiness.state(), ReadinessState::NotReady);
//!
//! readiness.mark_ready();
//! assert!(readiness.is_ready());
//!
//! readiness.mark_not_ready();
//! assert!(!readiness.is_ready());
//!
//! assert_eq!(LivenessState::Live, LivenessState::Live);
//! assert_eq!(liveness_check().http_status_code(), axum::http::StatusCode::OK);
//! assert_eq!(
//!     readiness_check(&readiness).http_status_code(),
//!     axum::http::StatusCode::SERVICE_UNAVAILABLE
//! );
//! ```

mod check;
mod liveness;
mod readiness;
mod response;

pub use check::{liveness_check, readiness_check};
pub use liveness::LivenessState;
pub use readiness::{Readiness, ReadinessState, ReadinessWatchError, ReadinessWatcher};
pub use response::{GrpcServingStatus, HealthCheckKind, HealthResponse, HealthStatus};
