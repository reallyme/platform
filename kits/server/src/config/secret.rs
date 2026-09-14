// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fmt;

use subtle::ConstantTimeEq;
use zeroize::{Zeroize, Zeroizing};

/// Generic secret wrapper that redacts debug output and zeroizes memory on
/// drop.
///
/// This is intended for configuration values such as API keys, tokens, and
/// passwords loaded at service startup. The wrapper prevents accidental
/// disclosure through `Debug` output while also reducing memory remanence risk.
///
/// `Secret<T>` intentionally does not implement `Clone`, `PartialEq`, or `Eq`.
/// Cloning duplicates secret material in memory, and direct equality checks can
/// encourage casual secret comparisons in higher-level code. If a future use
/// case needs either behavior, it should be added deliberately with explicit
/// review of the memory and API tradeoffs.
///
/// # Allocation Notes
///
/// This wrapper zeroizes the bytes held by the final `T` allocation. If callers
/// build secret material via mutable `String` growth (for example, starting from a
/// small allocation and repeatedly `push_str`), temporary growth buffers are
/// dropped before wrapping and are not controlled by this type. Build the final
/// secret in one allocation (for example, pre-sized from the source length) and
/// pass that ready value directly to `SecretString::new`.
///
/// # Examples
///
/// ```rust
/// use reallyme_server_kit::config::SecretString;
///
/// let token = SecretString::new("top-secret-token".to_owned());
///
/// assert_eq!(format!("{token:?}"), "Secret([REDACTED])");
/// assert_eq!(token.expose_secret().as_str(), "top-secret-token");
/// ```
///
/// Secrets intentionally cannot be cloned.
///
/// ```compile_fail
/// use reallyme_server_kit::config::SecretString;
///
/// let token = SecretString::new("top-secret-token".to_owned());
/// let _copy = token.clone();
/// ```
///
/// Secrets intentionally cannot be compared directly.
///
/// ```compile_fail
/// use reallyme_server_kit::config::SecretString;
///
/// let first = SecretString::new("top-secret-token".to_owned());
/// let second = SecretString::new("top-secret-token".to_owned());
///
/// assert_eq!(first, second);
/// ```
pub struct Secret<T>(Zeroizing<T>)
where
    T: Zeroize;

impl<T> Secret<T>
where
    T: Zeroize,
{
    /// Wraps a secret value.
    pub fn new(value: T) -> Self {
        Self(Zeroizing::new(value))
    }

    /// Exposes the secret by shared reference for explicit consumers.
    pub fn expose_secret(&self) -> &T {
        &self.0
    }

    /// Compares a candidate secret to this secret using constant-time equality.
    ///
    /// Secret-bearing newtypes in this crate provide this method to prevent
    /// callers from defaulting to byte-by-byte `==` on sensitive material.
    pub fn constant_time_eq(&self, candidate: &str) -> bool
    where
        T: AsRef<str>,
    {
        candidate
            .as_bytes()
            .ct_eq(self.0.as_ref().as_bytes())
            .into()
    }
}

impl<T> fmt::Debug for Secret<T>
where
    T: Zeroize,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Secret([REDACTED])")
    }
}

/// Secret string convenience alias.
pub type SecretString = Secret<String>;

#[cfg(test)]
#[path = "secret_tests.rs"]
mod tests;
