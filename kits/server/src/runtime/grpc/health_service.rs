// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Standard gRPC health behavior for registered and unknown service names.

use std::pin::Pin;
use std::sync::Arc;

use futures_util::{Stream, StreamExt, stream};
use tonic::{Request, Response, Status};
use tonic_health::pb::HealthCheckRequest;
use tonic_health::pb::HealthCheckResponse;
use tonic_health::pb::health_check_response::ServingStatus;
use tonic_health::pb::health_server::Health;
use tonic_health::server::HealthService;

/// Tonic-health returns NOT_FOUND for an unknown Watch, while the health
/// protocol requires an open stream starting with SERVICE_UNKNOWN.
#[derive(Clone)]
pub(super) struct RuntimeHealthService {
    inner: Arc<HealthService>,
}

impl RuntimeHealthService {
    pub(super) fn new(inner: HealthService) -> Self {
        Self {
            inner: Arc::new(inner),
        }
    }
}

#[tonic::async_trait]
impl Health for RuntimeHealthService {
    async fn check(
        &self,
        request: Request<HealthCheckRequest>,
    ) -> Result<Response<HealthCheckResponse>, Status> {
        self.inner.check(request).await
    }

    type WatchStream =
        Pin<Box<dyn Stream<Item = Result<HealthCheckResponse, Status>> + Send + 'static>>;

    async fn watch(
        &self,
        request: Request<HealthCheckRequest>,
    ) -> Result<Response<Self::WatchStream>, Status> {
        match self.inner.watch(request).await {
            Ok(response) => Ok(response.map(|stream| Box::pin(stream) as Self::WatchStream)),
            Err(status) if status.code() == tonic::Code::NotFound => {
                let unknown = HealthCheckResponse {
                    status: ServingStatus::ServiceUnknown as i32,
                };
                // Service registration is fixed before the listener starts.
                // Keep the unknown stream open until the client or server
                // closes it, as required by the gRPC health protocol.
                let stream = stream::once(async move { Ok(unknown) }).chain(stream::pending());
                Ok(Response::new(Box::pin(stream)))
            }
            Err(status) => Err(status),
        }
    }
}

#[cfg(test)]
#[path = "health_service_tests.rs"]
mod tests;
