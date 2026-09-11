// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Node-local asymmetric identity for first-boot enrollment.

use reallyme_codec::base64url::{base64url_to_bytes, bytes_to_base64url};
use reallyme_crypto::{ed25519, sha2};
use reallyme_hephaestus_domain::{HephaestusAgentPublicKey, HephaestusAgentPublicKeyFingerprint};
use zeroize::{Zeroize, Zeroizing};

use crate::error::{AgentResult, HephaestusAgentError, HephaestusAgentErrorReason};

const ED25519_SECRET_KEY_BYTES: usize = 32;

/// Node-local Ed25519 signing seed plus its derived public key.
#[derive(Clone)]
pub struct AgentSigningKey {
    public_key: Vec<u8>,
    private_seed: Zeroizing<Vec<u8>>,
}

impl AgentSigningKey {
    fn new(public_key: Vec<u8>, private_seed: Zeroizing<Vec<u8>>) -> AgentResult<Self> {
        if public_key.len() != ED25519_SECRET_KEY_BYTES
            || private_seed.len() != ED25519_SECRET_KEY_BYTES
        {
            return Err(HephaestusAgentError::new(
                HephaestusAgentErrorReason::IdentityKeyUnavailable,
            ));
        }
        Ok(Self {
            public_key,
            private_seed,
        })
    }

    /// Borrows the public Ed25519 verification key bytes.
    pub fn public_key_bytes(&self) -> &[u8] {
        self.public_key.as_slice()
    }

    fn private_seed_bytes(&self) -> &[u8] {
        self.private_seed.as_slice()
    }
}

impl std::fmt::Debug for AgentSigningKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AgentSigningKey")
            .field("public_key_len", &self.public_key.len())
            .finish_non_exhaustive()
    }
}

/// Node-generated Ed25519 enrollment identity.
#[derive(Debug, Clone)]
pub struct AgentIdentity {
    public_key: HephaestusAgentPublicKey,
    public_key_fingerprint: HephaestusAgentPublicKeyFingerprint,
}

impl AgentIdentity {
    /// Constructs a public enrollment identity from a signing key.
    pub fn from_signing_key(signing_key: &AgentSigningKey) -> AgentResult<Self> {
        let public_key = bytes_to_base64url(signing_key.public_key_bytes());
        let public_key = HephaestusAgentPublicKey::new(public_key).map_err(|_error| {
            HephaestusAgentError::new(HephaestusAgentErrorReason::IdentityKeyUnavailable)
        })?;
        let public_key_fingerprint =
            HephaestusAgentPublicKeyFingerprint::new(hex_sha256(public_key.as_str().as_bytes()))
                .map_err(|_error| {
                    HephaestusAgentError::new(HephaestusAgentErrorReason::CryptoUnavailable)
                })?;
        Ok(Self {
            public_key,
            public_key_fingerprint,
        })
    }

    /// Returns the encoded public key.
    pub const fn public_key(&self) -> &HephaestusAgentPublicKey {
        &self.public_key
    }

    /// Returns the public-key fingerprint.
    pub const fn public_key_fingerprint(&self) -> &HephaestusAgentPublicKeyFingerprint {
        &self.public_key_fingerprint
    }
}

/// Generates a new Ed25519 signing key from the OS CSPRNG.
pub fn generate_signing_key() -> AgentResult<AgentSigningKey> {
    let (public_key, private_seed) = ed25519::generate_ed25519_keypair().map_err(|_error| {
        HephaestusAgentError::new(HephaestusAgentErrorReason::IdentityKeyUnavailable)
    })?;
    AgentSigningKey::new(public_key, private_seed)
}

/// Encodes the private signing seed for local storage.
pub fn encode_signing_key(signing_key: &AgentSigningKey) -> String {
    bytes_to_base64url(signing_key.private_seed_bytes())
}

/// Signs the shared first-boot registration payload with the node private key.
pub fn sign_registration_payload(
    signing_key: &AgentSigningKey,
    payload: &str,
) -> AgentResult<String> {
    let signature = ed25519::sign_ed25519(signing_key.private_seed_bytes(), payload.as_bytes())
        .map_err(|_error| {
            HephaestusAgentError::new(HephaestusAgentErrorReason::CryptoUnavailable)
        })?;
    Ok(bytes_to_base64url(signature.as_slice()))
}

/// Decodes a locally stored private signing seed.
pub fn decode_signing_key(value: &str) -> AgentResult<AgentSigningKey> {
    let mut decoded = base64url_to_bytes(value).map_err(|_error| {
        HephaestusAgentError::new(HephaestusAgentErrorReason::IdentityKeyUnavailable)
    })?;
    if decoded.len() != ED25519_SECRET_KEY_BYTES {
        decoded.zeroize();
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::IdentityKeyUnavailable,
        ));
    }
    let mut secret = [0u8; ED25519_SECRET_KEY_BYTES];
    secret.copy_from_slice(decoded.as_slice());
    decoded.zeroize();
    let keypair = ed25519::generate_ed25519_keypair_from_seed(&secret).map_err(|_error| {
        HephaestusAgentError::new(HephaestusAgentErrorReason::IdentityKeyUnavailable)
    });
    secret.zeroize();
    let (public_key, private_seed) = keypair?;
    AgentSigningKey::new(public_key, private_seed)
}

fn hex_sha256(value: &[u8]) -> String {
    let digest = sha2::digest(value);
    let mut output = String::with_capacity(sha2::SHA2_256_DIGEST_LENGTH * 2);
    for byte in digest.as_bytes() {
        let high = usize::from(byte >> 4);
        let low = usize::from(byte & 0x0F);
        output.push(char::from(b"0123456789abcdef"[high]));
        output.push(char::from(b"0123456789abcdef"[low]));
    }
    output
}

#[cfg(test)]
mod tests;
