// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Reusable server runtime orchestration.
//!
//! This module owns the generic mechanics of running a server process:
//! observability bootstrap, listener binding, operational HTTP routes, gRPC
//! health serving, readiness transitions, shutdown handling, and managed task
//! supervision. App crates provide what they do: app state, app routes, gRPC
//! handlers, startup dependencies, and app-specific background tasks.

mod app;
mod app_health;
mod background;
mod cleanup;
pub(crate) mod connection_guard;
mod critical;
mod error;
#[cfg(feature = "tonic-grpc")]
mod grpc;
#[cfg(feature = "tonic-grpc")]
mod grpc_idle;
mod http;
mod phase;
pub(crate) mod rate_limit;
mod readiness_drain;
mod server;
mod startup_check;
mod termination;

pub use app::{AppHttpMountPath, AppName, RuntimeApp, RuntimeAppDependency, RuntimeAppHandle};
pub use background::RuntimeBackgroundTask;
pub use cleanup::RuntimeCleanupHook;
pub use critical::{
    CriticalTaskReadinessTimeout, CriticalTaskReadinessTimeoutError,
    CriticalTaskReadinessTimeoutErrorReason, CriticalTaskReadySignal, CriticalTaskReadySignalError,
    CriticalTaskReadySignalErrorReason, RuntimeCriticalTask, RuntimeCriticalTaskFailureReason,
    RuntimeCriticalTaskFailureStage,
};
pub use error::{
    OperationalFailureSource, RetryDisposition, RuntimeAppAdapterError,
    RuntimeAppCleanupErrorReason, RuntimeAppCompositionErrorReason,
    RuntimeListenerCompositionErrorReason, ServerRuntimeError, ServerRuntimeErrorKind,
    ServerRuntimeRequiredField, ServerRuntimeTransport,
};
#[cfg(feature = "tonic-grpc")]
pub use grpc::{
    GrpcAppRoutes, GrpcAppRoutesError, GrpcAppRoutesErrorReason, GrpcMethodPolicy, GrpcServerSpec,
    GrpcTransportTimeouts, GrpcTransportTimeoutsError, GrpcTransportTimeoutsErrorReason,
};
pub use http::{
    HttpIpv6SourcePrefixError, HttpIpv6SourcePrefixErrorReason, HttpRateLimitScope,
    HttpRateLimitTierPolicy, HttpRateLimitTierPolicyError, HttpRateLimitTierPolicyErrorReason,
    HttpServerSpec,
};
pub use phase::{
    ServerRuntimePhase, ServerRuntimePhaseReporter, ServerRuntimePhaseWatchError,
    ServerRuntimePhaseWatcher,
};
pub(crate) use rate_limit::{RateLimitDecision, RateLimitRegistry, RateLimitSourceIdentity};
pub use readiness_drain::{
    ReadinessDrainDelay, ReadinessDrainDelayError, ReadinessDrainDelayErrorReason,
};
pub use server::{ServerRuntime, ServerRuntimeBuilder};
pub use startup_check::{
    RuntimeStartupCheck, StartupCheckTimeout, StartupCheckTimeoutError,
    StartupCheckTimeoutErrorReason,
};
