// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Validated service locator selectors and schemes.

use super::validation::validate_identifier;
use super::{
    AppConfigDocumentError, AppConfigDocumentErrorReason, AppServiceLocatorDocument,
    MAX_LOCATOR_TAGS,
};

/// Validated service locator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppServiceLocator {
    /// Tailscale service locator.
    Tailscale(TailscaleServiceLocator),
}

impl AppServiceLocator {
    /// Builds a validated service locator from a raw config document.
    pub fn from_document(
        document: AppServiceLocatorDocument,
    ) -> Result<Self, AppConfigDocumentError> {
        match document.mode.as_str() {
            "tailscale" | "tailscale_service" => Ok(Self::Tailscale(
                TailscaleServiceLocator::from_document(document)?,
            )),
            _ => Err(AppConfigDocumentError::new(
                AppConfigDocumentErrorReason::InvalidLocatorMode,
            )),
        }
    }

    /// Returns the locator provider token.
    pub const fn provider(&self) -> AppServiceLocatorProvider {
        match self {
            Self::Tailscale(_) => AppServiceLocatorProvider::Tailscale,
        }
    }

    /// Returns Tailscale locator metadata when applicable.
    pub const fn tailscale(&self) -> Option<&TailscaleServiceLocator> {
        match self {
            Self::Tailscale(locator) => Some(locator),
        }
    }
}

/// Supported service-locator providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppServiceLocatorProvider {
    /// Tailscale Services MagicDNS.
    Tailscale,
}

impl AppServiceLocatorProvider {
    /// Returns the stable provider token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tailscale => "tailscale_service",
        }
    }
}

/// Tailscale Service locator selector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TailscaleServiceLocator {
    service: String,
    tags: Vec<String>,
    port: Option<u16>,
    scheme: Option<AppServiceEndpointScheme>,
}

impl TailscaleServiceLocator {
    fn from_document(document: AppServiceLocatorDocument) -> Result<Self, AppConfigDocumentError> {
        validate_identifier(document.service.as_str())?;
        if document.tags.len() > MAX_LOCATOR_TAGS {
            return Err(AppConfigDocumentError::new(
                AppConfigDocumentErrorReason::InvalidLocatorTag,
            ));
        }
        for tag in &document.tags {
            validate_identifier(tag.as_str())?;
        }
        let scheme = document
            .scheme
            .as_deref()
            .map(AppServiceEndpointScheme::parse)
            .transpose()?;
        if document.port == Some(0) {
            return Err(AppConfigDocumentError::new(
                AppConfigDocumentErrorReason::InvalidServiceEndpointSource,
            ));
        }

        Ok(Self {
            service: document.service,
            tags: document.tags,
            port: document.port,
            scheme,
        })
    }

    /// Returns the Tailscale Service token.
    pub fn service(&self) -> &str {
        self.service.as_str()
    }

    /// Returns configured Tailscale Service metadata tags.
    pub fn tags(&self) -> &[String] {
        self.tags.as_slice()
    }

    /// Returns the configured endpoint port, if present.
    pub const fn port(&self) -> Option<u16> {
        self.port
    }

    /// Returns the configured endpoint scheme, if present.
    pub const fn scheme(&self) -> Option<AppServiceEndpointScheme> {
        self.scheme
    }
}

/// Supported URL schemes for Tailscale Service endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppServiceEndpointScheme {
    /// HTTPS over the tailnet.
    Https,
}

impl AppServiceEndpointScheme {
    fn parse(value: &str) -> Result<Self, AppConfigDocumentError> {
        match value {
            "https" => Ok(Self::Https),
            _ => Err(AppConfigDocumentError::new(
                AppConfigDocumentErrorReason::InvalidServiceEndpointSource,
            )),
        }
    }

    /// Returns the URL scheme token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Https => "https",
        }
    }
}
