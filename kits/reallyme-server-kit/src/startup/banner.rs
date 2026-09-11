// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Local-development startup banner helpers.
//!
//! A host-provided banner is intentionally treated as optional operator-facing
//! decoration rather than part of the structured logging contract:
//!
//! - disabled for JSON logs
//! - disabled for production by default
//! - allowed for local/dev plain-text startup only
//! - never mixed into structured JSON log streams
//!
//! Callers should decide whether to emit the banner before initializing JSON
//! logging, and should prefer the typed helper functions here rather than
//! open-coded environment checks.
//!
//! # Examples
//!
//! ```no_run
//! use std::io;
//! use std::time::Duration;
//!
//! use reallyme_server_kit::config::{
//!     LogFormat, MetricsIdleTimeout, ObservabilityConfig, ServiceEnvironment,
//! };
//! use reallyme_server_kit::observability::init_tracing;
//! use reallyme_server_kit::startup::{ServerName, StartupBanner, write_startup_banner};
//!
//! let server_name = ServerName::new("reallyme-api").expect("valid server name");
//! let observability = ObservabilityConfig::new(
//!     ServiceEnvironment::Local,
//!     LogFormat::PlainText,
//!     false,
//!     "reallyme_server_kit=info".to_owned(),
//!     MetricsIdleTimeout::new(Duration::from_secs(30)).expect("valid metrics timeout"),
//! )
//! .expect("valid observability config");
//!
//! let banner = StartupBanner::new("Example server");
//! let _ = write_startup_banner(
//!     &mut io::stdout(),
//!     banner,
//!     observability.service_environment(),
//!     observability.log_format(),
//! )
//! .expect("banner write should succeed");
//!
//! init_tracing(server_name, &observability).expect("tracing should initialize");
//! ```

use std::io::{self, Write};

use crate::config::{LogFormat, ServiceEnvironment};

/// Static, host-supplied startup banner content.
///
/// The runtime accepts only a `&'static str` so server composition keeps
/// ownership of the operator-facing text. It deliberately does not accept a
/// runtime-configured string: startup output must not become an unbounded or
/// secret-bearing configuration channel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StartupBanner {
    text: &'static str,
}

impl StartupBanner {
    /// Creates a startup banner from host-owned static text.
    pub const fn new(text: &'static str) -> Self {
        Self { text }
    }

    /// Returns the static text supplied by the server host.
    pub const fn text(self) -> &'static str {
        self.text
    }
}

impl Default for StartupBanner {
    fn default() -> Self {
        Self::new("")
    }
}

/// Returns whether the startup banner should be emitted.
///
/// The banner is intended only for interactive local or shared-development
/// startup flows that use plain-text logging. Production-like or JSON-log
/// environments should keep startup output fully structured.
pub const fn should_emit_startup_banner(
    banner: StartupBanner,
    service_environment: ServiceEnvironment,
    log_format: LogFormat,
) -> bool {
    if banner.text().is_empty() {
        return false;
    }

    match (service_environment, log_format) {
        (ServiceEnvironment::Local | ServiceEnvironment::Dev, LogFormat::PlainText) => true,
        (ServiceEnvironment::Staging | ServiceEnvironment::Prod, LogFormat::PlainText) => false,
        (_, LogFormat::Json) => false,
    }
}

/// Writes the startup banner to the provided writer when the environment/log
/// format policy allows it.
///
/// Returns `Ok(true)` only when the banner was actually emitted.
pub fn write_startup_banner<W>(
    writer: &mut W,
    banner: StartupBanner,
    service_environment: ServiceEnvironment,
    log_format: LogFormat,
) -> io::Result<bool>
where
    W: Write,
{
    if !should_emit_startup_banner(banner, service_environment, log_format) {
        return Ok(false);
    }

    writer.write_all(banner.text().as_bytes())?;
    writer.write_all(b"\n")?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::{StartupBanner, should_emit_startup_banner, write_startup_banner};
    use crate::config::{LogFormat, ServiceEnvironment};

    const TEST_BANNER: StartupBanner = StartupBanner::new("test banner");

    #[test]
    fn startup_banner_preserves_host_supplied_text() {
        assert_eq!(TEST_BANNER.text(), "test banner");
    }

    #[test]
    fn banner_is_enabled_only_for_local_or_dev_plain_text_logs() {
        assert!(should_emit_startup_banner(
            TEST_BANNER,
            ServiceEnvironment::Local,
            LogFormat::PlainText
        ));
        assert!(should_emit_startup_banner(
            TEST_BANNER,
            ServiceEnvironment::Dev,
            LogFormat::PlainText
        ));
        assert!(!should_emit_startup_banner(
            TEST_BANNER,
            ServiceEnvironment::Staging,
            LogFormat::PlainText
        ));
        assert!(!should_emit_startup_banner(
            TEST_BANNER,
            ServiceEnvironment::Prod,
            LogFormat::PlainText
        ));
        assert!(!should_emit_startup_banner(
            TEST_BANNER,
            ServiceEnvironment::Local,
            LogFormat::Json
        ));
        assert!(!should_emit_startup_banner(
            TEST_BANNER,
            ServiceEnvironment::Prod,
            LogFormat::Json
        ));
    }

    #[test]
    fn banner_writer_respects_policy() {
        let mut local_output = Vec::new();
        let emitted = write_startup_banner(
            &mut local_output,
            TEST_BANNER,
            ServiceEnvironment::Local,
            LogFormat::PlainText,
        )
        .expect("writing local banner should succeed");
        assert!(emitted);
        assert_eq!(
            String::from_utf8(local_output).expect("banner output should be utf-8"),
            "test banner\n"
        );

        let mut prod_output = Vec::new();
        let emitted = write_startup_banner(
            &mut prod_output,
            TEST_BANNER,
            ServiceEnvironment::Prod,
            LogFormat::Json,
        )
        .expect("writing production banner should succeed");
        assert!(!emitted);
        assert!(prod_output.is_empty());
    }

    #[test]
    fn empty_banner_is_never_emitted() {
        let mut output = Vec::new();
        let emitted = write_startup_banner(
            &mut output,
            StartupBanner::default(),
            ServiceEnvironment::Local,
            LogFormat::PlainText,
        )
        .expect("writing an empty banner should succeed");

        assert!(!emitted);
        assert!(output.is_empty());
    }
}
