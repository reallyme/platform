// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::BTreeMap;

use serde::Deserialize;

use super::service_endpoint::AppServiceEndpointSourceDocument;

/// Raw deserialized JSONC envelope.
///
/// This type intentionally remains private to the document module. Public app
/// code receives validated newtypes only, which keeps app config immutable and
/// fail-closed after construction.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawAppJsoncConfigDocument<TCustom> {
    #[serde(default)]
    pub(super) public_base_url: Option<String>,
    pub(super) cors: RawCorsConfig,
    #[serde(default)]
    pub(super) cookies: Option<RawCookieConfig>,
    pub(super) reflection_enabled: bool,
    pub(super) downstream: BTreeMap<String, RawDownstreamEndpointConfig>,
    #[serde(flatten)]
    pub(super) custom: TCustom,
}

#[derive(Deserialize)]
pub(super) struct RawCorsConfig {
    pub(super) allowed_origins: Vec<String>,
}

#[derive(Deserialize)]
pub(super) struct RawCookieConfig {
    #[serde(default)]
    pub(super) secure: Option<bool>,
    #[serde(default)]
    pub(super) domain: Option<String>,
    #[serde(default)]
    pub(super) same_site_policy: Option<String>,
}

pub(super) type RawDownstreamEndpointConfig = AppServiceEndpointSourceDocument;
