// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::app_cors_layer;
use crate::config::CorsConfig;
use reallyme_app_kit::AppJsoncConfigDocument;

#[test]
fn no_cors_is_the_default_safe_posture() {
    assert!(matches!(CorsConfig::no_cors(), CorsConfig::NoCors));
}

#[test]
fn app_cors_layer_is_none_when_no_origins_declared() {
    let document = AppJsoncConfigDocument::<reallyme_app_kit::NoAppCustomConfig>::from_jsonc_str(
        r#"{
            "cors": {"allowed_origins": []},
            "reflection_enabled": false,
            "downstream": {}
        }"#,
    )
    .expect("valid empty-cors fixture");
    assert!(app_cors_layer(document.cors()).is_none());
}

#[test]
fn app_cors_layer_is_some_when_origins_declared() {
    let document = AppJsoncConfigDocument::<reallyme_app_kit::NoAppCustomConfig>::from_jsonc_str(
        r#"{
            "cors": {"allowed_origins": ["https://app.reallyme.net"]},
            "reflection_enabled": false,
            "downstream": {}
        }"#,
    )
    .expect("valid single-origin fixture");
    assert!(app_cors_layer(document.cors()).is_some());
}
