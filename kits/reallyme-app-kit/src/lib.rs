// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]

//! Host-neutral app conventions for ReallyMe app crates.
//!
//! `reallyme-app-kit` defines reusable contracts that app cores and adapters can
//! share without depending on a particular host runtime. It must not bind
//! listeners, own process lifecycle, initialize tracing, or mount routes.

pub mod adapters;
pub mod app;
pub mod auth;
pub mod config;
pub mod contract;
pub mod descriptor;
pub mod error;
pub mod health;
pub mod lifecycle;
pub mod metadata;
pub mod metrics;
mod name_validator;
pub mod ports;
pub mod registration;

/// Constructs an app version from the calling crate's Cargo metadata.
///
/// This macro intentionally resolves `CARGO_PKG_VERSION` in the caller's crate
/// rather than this crate's metadata.
#[macro_export]
macro_rules! app_version {
    () => {
        $crate::metadata::AppVersion::new(env!("CARGO_PKG_VERSION"))
    };
}

pub use adapters::{
    ConnectAdapterConventions, ConnectCodeGenerationWorkflow, ConnectJsonCompatibilityPolicy,
    ConnectProtocolEncoding, ConnectPublicEncodingPolicy, ConnectSchemaSource,
    GrpcAdapterConventions, HttpAdapterConventions, ServerAdapterConventions,
    WorkersAdapterConventions,
};
pub use app::{StandardAppContext, StandardAppCore, StandardAppState};
pub use auth::{AppCapability, AppCapabilityName, AppPermission, AppPermissionName};
pub use config::{
    AppBaseUrl, AppConfig, AppConfigDocumentError, AppConfigDocumentErrorReason, AppConfigFormat,
    AppConfigParseError, AppConfigParseErrorReason, AppConfigProfile, AppConfigSource,
    AppCookieConfig, AppCookieDomain, AppCookieSameSitePolicy, AppCorsConfig, AppDownstreamBaseUrl,
    AppDownstreamConfig, AppDownstreamEndpointConfig, AppJsoncConfigDocument,
    AppServiceEndpointResolutionError, AppServiceEndpointResolutionErrorReason,
    AppServiceEndpointResolver, AppServiceEndpointScheme, AppServiceEndpointSelection,
    AppServiceEndpointSource, AppServiceEndpointSourceDocument, AppServiceEndpointUrl,
    AppServiceLocatedEndpoints, AppServiceLocator, AppServiceLocatorDocument,
    AppServiceLocatorProvider, AppServiceStaticEndpoints, NoAppCustomConfig,
    TailscaleServiceLocator, parse_app_jsonc_config_document, parse_jsonc_config,
    strip_jsonc_comments,
};
pub use contract::{
    AppContractDescriptor, AppContractName, AppContractService, AppDependencyBinding,
    AppDependencyBindingKey, AppDependencyBindingMode, AppDependencyBindingTarget,
    AppHttpDtoConvention, AppHttpRouteContract,
};
pub use descriptor::{AppDescriptor, AppDescriptorSpec};
pub use error::{
    AppErrorCategory, AppErrorCode, AppErrorContract, AppErrorRetryDisposition, AppKitError,
    AppKitErrorReason, AppKitField,
};
pub use health::{
    AppHealthContribution, AppHealthContributor, AppHealthFuture, AppHealthStatus,
    ready_app_health_contribution,
};
pub use lifecycle::{
    AppBackgroundTask, AppBackgroundTaskDescriptor, AppBackgroundTaskFuture, AppCleanupFuture,
    AppCleanupHook, AppCleanupHookDescriptor, AppLifecycleError, AppLifecycleErrorKind,
    AppLifecycleName, AppStartupCheck, AppStartupCheckDescriptor, AppStartupCheckFuture,
    AppTaskShutdown,
};
pub use metadata::{AppMetadata, AppName, AppVersion};
pub use metrics::{
    AppMetricName, AppMetricNamespace, record_app_metric_counter, record_app_metric_counter_by_name,
};
pub use ports::{
    AppAdapterKind, AppDownstreamPortDescriptor, AppPortDescriptor, AppPortError, AppPortErrorKind,
    AppPortHealth, AppPortName, AppPortRetryPolicy, AppPortTimeout, AppPortTimeoutError,
};
pub use registration::{AppDependency, AppRegistration};
