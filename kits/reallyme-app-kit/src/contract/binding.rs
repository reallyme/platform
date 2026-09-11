// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde::{Deserialize, Serialize};

use crate::config::AppDownstreamBaseUrl;
use crate::metadata::AppName;
use crate::ports::AppPortName;
use crate::{AppKitError, AppKitErrorReason, AppKitField};

/// Binding key for one app downstream port.
///
/// This models config keys such as `api.handle` without carrying arbitrary
/// strings through the runtime. The source app and port name are validated
/// independently and remain low-cardinality deployment metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppDependencyBindingKey {
    source_app: AppName,
    port_name: AppPortName,
}

impl AppDependencyBindingKey {
    /// Constructs a binding key from typed app and port names.
    pub const fn new(source_app: AppName, port_name: AppPortName) -> Self {
        Self {
            source_app,
            port_name,
        }
    }

    /// Parses a dotted dependency key, for example `api.handle`.
    pub fn parse(value: &str) -> Result<Self, AppKitError> {
        let Some((source_app, port_name)) = value.split_once('.') else {
            return Err(AppKitError::new(
                AppKitField::DependencyBinding,
                AppKitErrorReason::InvalidCharacter,
            ));
        };

        if port_name.contains('.') {
            return Err(AppKitError::new(
                AppKitField::DependencyBinding,
                AppKitErrorReason::InvalidCharacter,
            ));
        }

        Ok(Self {
            source_app: AppName::new(source_app)?,
            port_name: AppPortName::new(port_name)?,
        })
    }

    /// Returns the app that owns the downstream port.
    pub const fn source_app(&self) -> &AppName {
        &self.source_app
    }

    /// Returns the downstream port name.
    pub const fn port_name(&self) -> &AppPortName {
        &self.port_name
    }
}

/// Host-selected dependency binding mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppDependencyBindingMode {
    /// The dependency is hosted in the same server process and should be
    /// injected as an in-memory typed port adapter.
    InProcess,
    /// The dependency is hosted remotely and is eligible for a generated
    /// Connect client adapter.
    ///
    /// This mode only validates the host-level deployment decision. Concrete
    /// remote client injection belongs in a server/app adapter once a real
    /// downstream contract crate exists; the app-kit does not pretend to own or
    /// implement network clients.
    RemoteConnect,
}

/// Validated target for a dependency binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppDependencyBindingTarget {
    /// In-process target app.
    InProcess {
        /// Enabled app that implements the downstream port in the same process.
        target_app: AppName,
    },
    /// Remote Connect endpoint.
    RemoteConnect {
        /// Base URL for the remote Connect endpoint.
        base_url: AppDownstreamBaseUrl,
    },
}

/// Validated host-level app dependency binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppDependencyBinding {
    key: AppDependencyBindingKey,
    target: AppDependencyBindingTarget,
}

impl AppDependencyBinding {
    /// Constructs an in-process dependency binding.
    pub const fn in_process(key: AppDependencyBindingKey, target_app: AppName) -> Self {
        Self {
            key,
            target: AppDependencyBindingTarget::InProcess { target_app },
        }
    }

    /// Constructs a remote Connect dependency binding.
    pub const fn remote_connect(
        key: AppDependencyBindingKey,
        base_url: AppDownstreamBaseUrl,
    ) -> Self {
        Self {
            key,
            target: AppDependencyBindingTarget::RemoteConnect { base_url },
        }
    }

    /// Returns the dependency key.
    pub const fn key(&self) -> &AppDependencyBindingKey {
        &self.key
    }

    /// Returns the target binding.
    pub const fn target(&self) -> &AppDependencyBindingTarget {
        &self.target
    }

    /// Returns the host-selected binding mode.
    pub const fn mode(&self) -> AppDependencyBindingMode {
        match self.target {
            AppDependencyBindingTarget::InProcess { .. } => AppDependencyBindingMode::InProcess,
            AppDependencyBindingTarget::RemoteConnect { .. } => {
                AppDependencyBindingMode::RemoteConnect
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AppDependencyBindingKey, AppDependencyBindingMode};
    use crate::{AppKitError, AppKitErrorReason, AppKitField};

    #[test]
    fn parses_dependency_binding_keys() {
        let key = AppDependencyBindingKey::parse("api.handle").expect("valid dependency key");

        assert_eq!(key.source_app().as_str(), "api");
        assert_eq!(key.port_name().as_str(), "handle");
    }

    #[test]
    fn rejects_malformed_dependency_binding_keys() {
        assert_eq!(
            AppDependencyBindingKey::parse("api.handle.extra"),
            Err(AppKitError::new(
                AppKitField::DependencyBinding,
                AppKitErrorReason::InvalidCharacter,
            )),
        );
    }

    #[test]
    fn dependency_binding_modes_are_stable() {
        assert_eq!(
            AppDependencyBindingMode::InProcess,
            AppDependencyBindingMode::InProcess,
        );
    }
}
