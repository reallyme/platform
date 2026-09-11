// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Host-neutral app config contracts.

mod document;
mod jsonc;
mod source;
mod validate;

pub use document::{
    AppBaseUrl, AppConfigDocumentError, AppConfigDocumentErrorReason, AppCookieConfig,
    AppCookieDomain, AppCookieSameSitePolicy, AppCorsConfig, AppDownstreamBaseUrl,
    AppDownstreamConfig, AppDownstreamEndpointConfig, AppJsoncConfigDocument,
    AppServiceEndpointResolutionError, AppServiceEndpointResolutionErrorReason,
    AppServiceEndpointResolver, AppServiceEndpointScheme, AppServiceEndpointSelection,
    AppServiceEndpointSource, AppServiceEndpointSourceDocument, AppServiceEndpointUrl,
    AppServiceLocatedEndpoints, AppServiceLocator, AppServiceLocatorDocument,
    AppServiceLocatorProvider, AppServiceStaticEndpoints, NoAppCustomConfig,
    TailscaleServiceLocator, parse_app_jsonc_config_document,
};
pub use jsonc::{
    AppConfigParseError, AppConfigParseErrorReason, parse_jsonc_config, strip_jsonc_comments,
};
pub use source::{AppConfigFormat, AppConfigProfile, AppConfigSource};
pub use validate::AppConfig;
