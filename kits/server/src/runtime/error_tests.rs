// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{
    OperationalFailureSource, RetryDisposition, RuntimeListenerBindErrorReason,
    RuntimeListenerCompositionErrorReason, ServerRuntimeError,
};

#[test]
fn listener_bind_has_typed_operational_retry_classification() {
    let error = ServerRuntimeError::ListenerBind {
        transport: super::ServerRuntimeTransport::Http,
        bind_address: "127.0.0.1:18082"
            .parse()
            .expect("test bind address should parse"),
        reason: RuntimeListenerBindErrorReason::AddressInUse,
    };

    assert_eq!(
        error.operational_source(),
        OperationalFailureSource::Transport
    );
    assert_eq!(error.retry_disposition(), RetryDisposition::Retryable);
}

#[test]
fn composition_errors_require_configuration_change() {
    let error = ServerRuntimeError::ListenerComposition {
        reason: RuntimeListenerCompositionErrorReason::DuplicateListenerPort,
    };

    assert_eq!(
        error.operational_source(),
        OperationalFailureSource::Configuration
    );
    assert_eq!(
        error.retry_disposition(),
        RetryDisposition::RetryAfterConfigurationChange
    );
}
