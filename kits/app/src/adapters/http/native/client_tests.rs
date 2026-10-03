// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use zeroize::ZeroizeOnDrop;

use super::{BoundedHttpsClient, hardened_builder, zeroizing_body};
use crate::HttpsOrigin;

fn requires_zeroize_on_drop<T: ZeroizeOnDrop>() {}

#[test]
fn request_owner_has_a_zeroize_on_drop_contract() {
    requires_zeroize_on_drop::<zeroize::Zeroizing<Vec<u8>>>();
    let body = zeroizing_body(b"sensitive body").expect("body allocation");
    assert_eq!(body.as_bytes(), Some(b"sensitive body".as_slice()));
}

#[test]
fn client_debug_output_does_not_expose_its_origin() {
    let origin = HttpsOrigin::try_from("https://private-service.example/").expect("valid origin");
    let client = BoundedHttpsClient::new(origin).expect("client");
    let output = format!("{client:?}");
    assert!(!output.contains("private-service"));
    assert!(output.contains("redacted"));
}

#[test]
fn hardened_client_builder_is_constructible() {
    hardened_builder().build().expect("hardened client");
}
