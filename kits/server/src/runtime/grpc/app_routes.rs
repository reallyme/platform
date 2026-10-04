// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! gRPC route composition that retains the names required by health checks.

use std::convert::Infallible;

use axum::response::IntoResponse;
use thiserror::Error;
use tonic::body::Body;
use tonic::codegen::http::Request;
use tonic::server::NamedService;
use tonic::service::Routes;
use tower::Service;

const MAX_REGISTERED_SERVICES: usize = 64;
const MAX_SERVICE_NAME_BYTES: usize = 255;
const RUNTIME_HEALTH_SERVICE_NAME: &str = "grpc.health.v1.Health";

/// Stable reasons a gRPC route set cannot be composed safely.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrpcAppRoutesErrorReason {
    /// A service name is not a canonical protobuf service identifier.
    InvalidServiceName,
    /// The service name is owned by the runtime health implementation.
    ReservedServiceName,
    /// A service name was registered twice.
    DuplicateServiceName,
    /// The route set exceeded its reviewed service count.
    TooManyServices,
}

/// Typed gRPC route composition failure.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("invalid gRPC app routes")]
pub struct GrpcAppRoutesError {
    reason: GrpcAppRoutesErrorReason,
}

impl GrpcAppRoutesError {
    /// Returns the bounded reason without exposing service names.
    pub const fn reason(self) -> GrpcAppRoutesErrorReason {
        self.reason
    }
}

/// App-provided tonic routes with a complete, bounded health-service registry.
pub struct GrpcAppRoutes {
    routes: Routes,
    service_names: Vec<&'static str>,
}

impl GrpcAppRoutes {
    /// Creates an empty route set for hosts that currently serve health only.
    pub fn empty() -> Self {
        Self {
            routes: Routes::default(),
            service_names: Vec::new(),
        }
    }

    /// Creates a route set with its first named service.
    pub fn new<S>(service: S) -> Result<Self, GrpcAppRoutesError>
    where
        S: Service<Request<Body>, Error = Infallible>
            + NamedService
            + Clone
            + Send
            + Sync
            + 'static,
        S::Response: IntoResponse,
        S::Future: Send + 'static,
    {
        Self::empty().add_service(service)
    }

    /// Adds a service and registers its canonical name for gRPC health.
    pub fn add_service<S>(mut self, service: S) -> Result<Self, GrpcAppRoutesError>
    where
        S: Service<Request<Body>, Error = Infallible>
            + NamedService
            + Clone
            + Send
            + Sync
            + 'static,
        S::Response: IntoResponse,
        S::Future: Send + 'static,
    {
        let name = S::NAME;
        if !valid_service_name(name) {
            return Err(GrpcAppRoutesError {
                reason: GrpcAppRoutesErrorReason::InvalidServiceName,
            });
        }
        if name == RUNTIME_HEALTH_SERVICE_NAME {
            return Err(GrpcAppRoutesError {
                reason: GrpcAppRoutesErrorReason::ReservedServiceName,
            });
        }
        if self.service_names.contains(&name) {
            return Err(GrpcAppRoutesError {
                reason: GrpcAppRoutesErrorReason::DuplicateServiceName,
            });
        }
        if self.service_names.len() >= MAX_REGISTERED_SERVICES {
            return Err(GrpcAppRoutesError {
                reason: GrpcAppRoutesErrorReason::TooManyServices,
            });
        }
        self.routes = self.routes.add_service(service);
        self.service_names.push(name);
        Ok(self)
    }

    pub(super) fn into_parts(self) -> (Routes, Vec<&'static str>) {
        (self.routes, self.service_names)
    }
}

fn valid_service_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    if bytes.is_empty() || bytes.len() > MAX_SERVICE_NAME_BYTES {
        return false;
    }
    let mut segment_start = true;
    for byte in bytes {
        if *byte == b'.' {
            if segment_start {
                return false;
            }
            segment_start = true;
            continue;
        }
        let valid = if segment_start {
            byte.is_ascii_alphabetic() || *byte == b'_'
        } else {
            byte.is_ascii_alphanumeric() || *byte == b'_'
        };
        if !valid {
            return false;
        }
        segment_start = false;
    }
    !segment_start
}

#[cfg(test)]
#[path = "app_routes_tests.rs"]
mod tests;
