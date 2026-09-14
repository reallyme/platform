// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

/// App-kit field that failed validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppKitField {
    /// App name.
    AppName,
    /// App version.
    AppVersion,
    /// App config source name.
    ConfigSource,
    /// App lifecycle item name.
    LifecycleName,
    /// App metric namespace.
    MetricNamespace,
    /// App metric name.
    MetricName,
    /// App permission name.
    PermissionName,
    /// App capability name.
    CapabilityName,
    /// App port name.
    PortName,
    /// App contract name.
    ContractName,
    /// App dependency binding.
    DependencyBinding,
    /// App error code.
    ErrorCode,
}
