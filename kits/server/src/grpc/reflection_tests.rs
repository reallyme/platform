// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{GrpcReflectionMode, reflection_builder};
use crate::config::ServiceEnvironment;
use crate::grpc::GrpcReflectionError;

#[test]
fn reflection_is_disabled_by_default() {
    let builder = reflection_builder(GrpcReflectionMode::Disabled, ServiceEnvironment::Prod)
        .expect("disabled reflection should be allowed");

    assert!(builder.is_none());
}

#[test]
fn development_only_reflection_is_rejected_in_production() {
    let builder = reflection_builder(
        GrpcReflectionMode::DevelopmentOnly,
        ServiceEnvironment::Prod,
    );

    assert!(matches!(
        builder,
        Err(GrpcReflectionError::ReflectionNotAllowedInEnvironment {
            service_environment: ServiceEnvironment::Prod,
        })
    ));
}

#[test]
fn development_only_reflection_is_rejected_in_staging() {
    let builder = reflection_builder(
        GrpcReflectionMode::DevelopmentOnly,
        ServiceEnvironment::Staging,
    );

    assert!(matches!(
        builder,
        Err(GrpcReflectionError::ReflectionNotAllowedInEnvironment {
            service_environment: ServiceEnvironment::Staging,
        })
    ));
}

#[test]
fn development_only_reflection_is_allowed_in_local_environment() {
    let builder = reflection_builder(
        GrpcReflectionMode::DevelopmentOnly,
        ServiceEnvironment::Local,
    )
    .expect("development reflection should be allowed locally");

    assert!(builder.is_some());
}
