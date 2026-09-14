// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Standard host-neutral app JSONC config document.

mod cookies;
mod cors;
mod downstream;
mod error;
mod model;
mod raw;
mod service_endpoint;
mod url;

pub use cookies::{AppCookieConfig, AppCookieDomain, AppCookieSameSitePolicy};
pub use cors::AppCorsConfig;
pub use downstream::{AppDownstreamConfig, AppDownstreamEndpointConfig};
pub use error::{AppConfigDocumentError, AppConfigDocumentErrorReason};
pub use model::{AppJsoncConfigDocument, NoAppCustomConfig, parse_app_jsonc_config_document};
pub use service_endpoint::{
    AppServiceEndpointResolutionError, AppServiceEndpointResolutionErrorReason,
    AppServiceEndpointResolver, AppServiceEndpointScheme, AppServiceEndpointSelection,
    AppServiceEndpointSource, AppServiceEndpointSourceDocument, AppServiceEndpointUrl,
    AppServiceLocatedEndpoints, AppServiceLocator, AppServiceLocatorDocument,
    AppServiceLocatorProvider, AppServiceStaticEndpoints, TailscaleServiceLocator,
};
pub use url::{AppBaseUrl, AppDownstreamBaseUrl};

#[cfg(test)]
mod tests;
