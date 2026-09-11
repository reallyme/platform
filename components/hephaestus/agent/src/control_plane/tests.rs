// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use buffa::bytes::BytesMut;
use buffa::{Message, SizeCache};
use reallyme_hephaestus_contract::generated::proto::reallyme::hephaestus::v1::{
    RegisterAgentRequest, ResolveAgentSecretResponse,
};
use reallyme_hephaestus_domain::HephaestusAgentRuntimeToken;
use secrecy::ExposeSecret;

use super::{SensitiveRegisterAgentRequest, SensitiveResolveAgentSecretResponse, bearer_header};

#[test]
fn bearer_header_is_marked_sensitive() {
    let token = HephaestusAgentRuntimeToken::new("runtime-token-1").expect("runtime token");

    let header = bearer_header(&token).expect("bearer header");

    assert!(header.is_sensitive());
}

#[test]
fn sensitive_register_request_zeroizes_bootstrap_token() {
    let mut request = SensitiveRegisterAgentRequest::new(RegisterAgentRequest {
        report: Default::default(),
        bootstrap_token: "bootstrap-token-1".to_owned(),
        agent_version: "0.1.0".to_owned(),
        agent_public_key: "public-key".to_owned(),
        agent_public_key_fingerprint: "fingerprint".to_owned(),
        agent_registration_signature: "signature".to_owned(),
        __buffa_unknown_fields: Default::default(),
    });

    request.zeroize_bootstrap_token();

    assert!(request.0.bootstrap_token.is_empty());
}

#[test]
fn sensitive_resolve_agent_secret_response_zeroizes_secret_value() {
    let response = SensitiveResolveAgentSecretResponse::new(ResolveAgentSecretResponse {
        secret_value: "resolved-secret".to_owned(),
        __buffa_unknown_fields: Default::default(),
    });

    let secret = response.into_secret_value();
    assert_eq!(secret.expose_secret(), "resolved-secret");
}

#[test]
fn sensitive_resolve_agent_secret_response_zeroizes_on_demand() {
    let mut response = SensitiveResolveAgentSecretResponse::new(ResolveAgentSecretResponse {
        secret_value: "resolved-secret".to_owned(),
        __buffa_unknown_fields: Default::default(),
    });

    response.zeroize_secret_value();

    assert!(response.0.secret_value.is_empty());
}

#[test]
fn sensitive_register_request_serialization_matches_base_request() {
    let request = RegisterAgentRequest {
        report: Default::default(),
        bootstrap_token: "bootstrap-token-1".to_owned(),
        agent_version: "0.1.0".to_owned(),
        agent_public_key: "public-key".to_owned(),
        agent_public_key_fingerprint: "fingerprint".to_owned(),
        agent_registration_signature: "signature".to_owned(),
        __buffa_unknown_fields: Default::default(),
    };

    let sensitive_request = SensitiveRegisterAgentRequest::new(request.clone());

    let request_json = serde_json::to_string(&request)
        .expect("register request json serialization should succeed");
    let sensitive_json = serde_json::to_string(&sensitive_request)
        .expect("sensitive register request json serialization should succeed");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&request_json)
            .expect("normalized request json should deserialize"),
        serde_json::from_str::<serde_json::Value>(&sensitive_json)
            .expect("normalized sensitive request json should deserialize"),
    );

    let mut request_cache = SizeCache::default();
    let mut request_frame = BytesMut::new();
    request.write_to(&mut request_cache, &mut request_frame);

    let mut sensitive_cache = SizeCache::default();
    let mut sensitive_frame = BytesMut::new();
    sensitive_request.write_to(&mut sensitive_cache, &mut sensitive_frame);

    assert_eq!(request_frame, sensitive_frame);
}
