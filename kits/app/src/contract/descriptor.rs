// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::AppContractName;

/// Host-neutral descriptor for an app-owned contract crate.
///
/// App contracts are the stable boundary for DTOs, generated protobuf/Connect
/// types, route traits, and downstream-port traits. The descriptor is metadata
/// only; app-kit does not generate code, bind listeners, or implement RPCs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppContractDescriptor {
    name: AppContractName,
    proto_package: &'static str,
    services: &'static [AppContractService],
}

impl AppContractDescriptor {
    /// Constructs a contract descriptor from static contract metadata.
    pub const fn new(
        name: AppContractName,
        proto_package: &'static str,
        services: &'static [AppContractService],
    ) -> Self {
        Self {
            name,
            proto_package,
            services,
        }
    }

    /// Returns the contract name.
    pub const fn name(&self) -> &AppContractName {
        &self.name
    }

    /// Returns the canonical protobuf package for this contract.
    pub const fn proto_package(&self) -> &'static str {
        self.proto_package
    }

    /// Returns declared protobuf/Connect services contained by this app contract.
    pub const fn services(&self) -> &'static [AppContractService] {
        self.services
    }
}

/// Static descriptor for one protobuf/Connect service declared by an app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppContractService {
    service_name: &'static str,
}

impl AppContractService {
    /// Constructs a service descriptor from its fully-qualified protobuf name.
    pub const fn new(service_name: &'static str) -> Self {
        Self { service_name }
    }

    /// Returns the fully-qualified protobuf service name.
    pub const fn service_name(self) -> &'static str {
        self.service_name
    }
}
