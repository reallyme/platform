// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::{Arc, Mutex};

use crate::runtime::RuntimeStartupCheck;
use crate::startup::TaskName;
use crate::task::TaskExecutionError;

pub(super) fn recording_startup_check(
    task_name: &'static str,
    marker: &'static str,
    observed: &Arc<Mutex<Vec<&'static str>>>,
) -> RuntimeStartupCheck {
    let observed = Arc::clone(observed);

    RuntimeStartupCheck::new(
        TaskName::new(task_name).expect("valid fixture task name"),
        move || async move {
            observed
                .lock()
                .expect("test mutex should not be poisoned")
                .push(marker);
            Ok::<(), TaskExecutionError>(())
        },
    )
}
