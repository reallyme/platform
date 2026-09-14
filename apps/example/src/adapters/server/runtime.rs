// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use reallyme_app_kit::{
    AppBackgroundTaskDescriptor, AppCleanupHookDescriptor, AppLifecycleName,
    AppStartupCheckDescriptor,
};
use reallyme_server_kit::runtime::{
    RuntimeApp, RuntimeAppAdapterError, RuntimeBackgroundTask, RuntimeCleanupHook,
    RuntimeStartupCheck,
};
use reallyme_server_kit::task::{ShutdownToken, TaskExecutionError};

#[cfg(not(feature = "websocket"))]
use crate::adapters::http::router::router;
#[cfg(feature = "websocket")]
use crate::adapters::http::router::router_with_websocket_shutdown;
use crate::app::{
    EXAMPLE_BACKGROUND_TASK_NAME, EXAMPLE_CLEANUP_HOOK_NAME, EXAMPLE_STARTUP_CHECK_NAME,
    ExampleAppConfigDocument, context_from_config_document, example_app_metadata,
};
use crate::ports::ExamplePorts;

/// Builds the server-runtime adapter from a validated app JSONC config document.
///
/// Every lifecycle item declared by the
/// [`reallyme_app_kit::AppDescriptor`] is wired here: a startup readiness gate,
/// a long-running background task, and a shutdown cleanup hook. Each runtime
/// hook is constructed from the descriptor that carries its stable name, so the
/// contract advertised by the app stays in sync with what the runtime actually
/// executes.
pub fn runtime_app_from_config_document(
    document: &ExampleAppConfigDocument,
    ports: ExamplePorts,
) -> Result<RuntimeApp, RuntimeAppAdapterError> {
    let metadata = example_app_metadata()?;

    let startup_check_descriptor =
        AppStartupCheckDescriptor::new(AppLifecycleName::new(EXAMPLE_STARTUP_CHECK_NAME)?);
    let background_task_descriptor =
        AppBackgroundTaskDescriptor::new(AppLifecycleName::new(EXAMPLE_BACKGROUND_TASK_NAME)?);
    let cleanup_hook_descriptor =
        AppCleanupHookDescriptor::new(AppLifecycleName::new(EXAMPLE_CLEANUP_HOOK_NAME)?);

    #[cfg(feature = "websocket")]
    let (router, websocket_state) =
        router_with_websocket_shutdown(context_from_config_document(document, ports));
    #[cfg(not(feature = "websocket"))]
    let router = router(context_from_config_document(document, ports));

    let app = RuntimeApp::from_app_metadata(&metadata, router)?
        .with_cors(document.cors().clone())
        .with_startup_check(RuntimeStartupCheck::from_app_descriptor(
            &startup_check_descriptor,
            example_startup_check,
        )?)
        .with_background_task(RuntimeBackgroundTask::from_app_descriptor(
            &background_task_descriptor,
            example_background_task,
        )?)
        .with_cleanup_hook(RuntimeCleanupHook::from_app_descriptor(
            &cleanup_hook_descriptor,
            example_cleanup_hook,
        )?);

    #[cfg(feature = "websocket")]
    let app = app.with_websocket_shutdown_consumer(move |shutdown| {
        websocket_state.attach_process_shutdown_signal(shutdown)
    });

    Ok(app)
}

async fn example_startup_check() -> Result<(), TaskExecutionError> {
    Ok(())
}

async fn example_background_task(mut shutdown: ShutdownToken) -> Result<(), TaskExecutionError> {
    let _reason = shutdown.cancelled().await;
    Ok(())
}

async fn example_cleanup_hook() -> Result<(), TaskExecutionError> {
    Ok(())
}
