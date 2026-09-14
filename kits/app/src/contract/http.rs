// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

/// Standard posture for HTTP DTOs derived from app contracts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppHttpDtoConvention {
    /// HTTP DTOs are compatibility/adaptation DTOs mapped from canonical
    /// proto-shaped requests/responses.
    CompatibilityAdapter,
}

/// Host-neutral descriptor for an app-owned HTTP compatibility route.
///
/// API gateway and app adapters can use this descriptor to document which HTTP
/// paths expose a contract capability without treating HTTP/JSON as the
/// canonical internal model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppHttpRouteContract {
    route_template: &'static str,
    convention: AppHttpDtoConvention,
}

impl AppHttpRouteContract {
    /// Constructs a static HTTP route contract descriptor.
    pub const fn new(route_template: &'static str) -> Self {
        Self {
            route_template,
            convention: AppHttpDtoConvention::CompatibilityAdapter,
        }
    }

    /// Returns the route template.
    pub const fn route_template(self) -> &'static str {
        self.route_template
    }

    /// Returns the DTO convention for this route.
    pub const fn convention(self) -> AppHttpDtoConvention {
        self.convention
    }
}
