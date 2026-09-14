// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::cell::Cell;
use std::rc::Rc;

use zeroize::Zeroize;

use super::SecretString;
use crate::config::secret::Secret;

#[derive(Debug)]
struct TestSecretValue {
    zeroized: Rc<Cell<bool>>,
}

impl Zeroize for TestSecretValue {
    fn zeroize(&mut self) {
        self.zeroized.set(true);
    }
}

#[test]
fn debug_redacts_secret_value() {
    let secret = SecretString::new("top-secret-value".to_owned());
    let rendered = format!("{secret:?}");

    assert_eq!(rendered, "Secret([REDACTED])");
    assert!(!rendered.contains("top-secret-value"));
}

#[test]
fn secret_zeroizes_wrapped_value_on_drop() {
    let zeroized = Rc::new(Cell::new(false));

    {
        let secret = Secret::new(TestSecretValue {
            zeroized: Rc::clone(&zeroized),
        });

        assert!(!zeroized.get());
        let _ = secret.expose_secret();
    }

    assert!(zeroized.get());
}

#[test]
fn secret_string_constant_time_eq_covers_comparison_edges() {
    let secret = SecretString::new("top-secret-value".to_owned());

    assert!(secret.constant_time_eq("top-secret-value"));
    assert!(!secret.constant_time_eq("top-secret-other"));
    assert!(!secret.constant_time_eq("top-secret-value-extra"));
    assert!(SecretString::new(String::new()).constant_time_eq(""));
}
