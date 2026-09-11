// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{AgentIdentity, decode_signing_key, encode_signing_key, generate_signing_key};

#[test]
fn generated_identity_round_trips_private_key_and_public_fingerprint() {
    let signing_key = generate_signing_key().expect("key generation succeeds");
    let encoded = encode_signing_key(&signing_key);
    let decoded = decode_signing_key(encoded.as_str()).expect("key decodes");

    let first = AgentIdentity::from_signing_key(&signing_key).expect("identity builds");
    let second = AgentIdentity::from_signing_key(&decoded).expect("identity rebuilds");

    assert_eq!(first.public_key(), second.public_key());
    assert_eq!(
        first.public_key_fingerprint(),
        second.public_key_fingerprint()
    );
}
