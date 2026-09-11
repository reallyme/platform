// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use axum::Router;
use reallyme_app_kit::{AppCorsConfig, AppMetadata};

use crate::http::app_cors_layer;
use crate::startup::StartupError;
use crate::task::ShutdownToken;

use super::background::RuntimeBackgroundTask;
use super::cleanup::RuntimeCleanupHook;
use super::error::{RuntimeAppCompositionErrorReason, ServerRuntimeError};
use super::startup_check::RuntimeStartupCheck;
use std::sync::Arc;

const MAX_APP_HTTP_MOUNT_PATH_BYTES: usize = 128;

/// Validated name for a logical app hosted by a server runtime.
///
/// An app is a logical service unit inside one server process. App names flow
/// into startup diagnostics and future composition policy, so they use the same
/// conservative DNS-label-style shape as server and task names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppName(String);

impl AppName {
    /// Constructs a validated app name.
    pub fn new(value: impl Into<String>) -> Result<Self, StartupError> {
        let value = value.into();

        if value.is_empty() {
            return Err(StartupError::EmptyAppName);
        }

        if value.len() > 63 {
            return Err(StartupError::AppNameTooLong);
        }

        if value.starts_with('-') || value.ends_with('-') {
            return Err(StartupError::InvalidAppNameBoundary);
        }

        if value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return Ok(Self(value));
        }

        Err(StartupError::InvalidAppName)
    }

    /// Returns the validated app name.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Validated HTTP mount point for an app router.
///
/// Mount paths are server-composition concerns rather than app behavior. The
/// root mount `/` merges the app router directly into the process router.
/// Non-root mounts are static absolute path prefixes such as `/api` or
/// `/internal-admin`; query strings, fragments, whitespace, and wildcard-like
/// patterns are rejected so route composition remains deterministic and
/// reviewable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppHttpMountPath(String);

impl AppHttpMountPath {
    /// Constructs a validated app HTTP mount path.
    pub fn new(value: impl Into<String>) -> Result<Self, StartupError> {
        let value = value.into();

        if value.is_empty() {
            return Err(StartupError::EmptyAppHttpMountPath);
        }

        if value.len() > MAX_APP_HTTP_MOUNT_PATH_BYTES {
            return Err(StartupError::AppHttpMountPathTooLong);
        }

        if value == "/" {
            return Ok(Self(value));
        }

        if !value.starts_with('/')
            || value.ends_with('/')
            || value.contains("//")
            || value.contains('?')
            || value.contains('#')
            || value.chars().any(char::is_whitespace)
        {
            return Err(StartupError::InvalidAppHttpMountPath);
        }

        if value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'/' | b'-' | b'_')
        }) {
            return Ok(Self(value));
        }

        Err(StartupError::InvalidAppHttpMountPath)
    }

    /// Returns the root mount path.
    pub fn root() -> Self {
        Self("/".to_owned())
    }

    /// Returns the validated mount path.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    fn is_root(&self) -> bool {
        self.0 == "/"
    }
}

/// Opaque handle to a runtime app for typed dependency declarations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeAppHandle {
    name: AppName,
}

impl RuntimeAppHandle {
    /// Returns the app name represented by this handle.
    pub fn name(&self) -> &AppName {
        &self.name
    }
}

/// Required dependency on another app hosted by the same runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeAppDependency {
    name: AppName,
}

impl RuntimeAppDependency {
    /// Creates a dependency from an app handle.
    pub fn from_handle(handle: &RuntimeAppHandle) -> Self {
        Self {
            name: handle.name.clone(),
        }
    }

    /// Creates a dependency from a validated app name.
    pub fn from_app_name(name: AppName) -> Self {
        Self { name }
    }

    /// Returns the required dependency app name.
    pub fn name(&self) -> &AppName {
        &self.name
    }
}

/// Logical app hosted by a [`crate::runtime::ServerRuntime`].
///
/// The server runtime owns process lifecycle, listeners, standard layers,
/// health/readiness/version/metrics routes, shutdown, and task supervision.
/// Apps own behavior: HTTP routes, startup checks, and app-specific background
/// tasks. This mirrors Pingora's framework/app separation while preserving the
/// Axum/Tonic stack used by ReallyMe.
pub struct RuntimeApp {
    name: AppName,
    http_mount: AppHttpMountPath,
    http_router: Router,
    http_cors: Option<AppCorsConfig>,
    dependencies: Vec<RuntimeAppDependency>,
    websocket_shutdown_consumers: Vec<Arc<dyn Fn(ShutdownToken) + Send + Sync + 'static>>,
    startup_checks: Vec<RuntimeStartupCheck>,
    background_tasks: Vec<RuntimeBackgroundTask>,
    cleanup_hooks: Vec<RuntimeCleanupHook>,
}

impl RuntimeApp {
    /// Creates a runtime app from its validated name and HTTP router.
    pub fn new(name: AppName, http_router: Router) -> Self {
        Self {
            name,
            http_mount: AppHttpMountPath::root(),
            http_router,
            http_cors: None,
            dependencies: Vec::new(),
            websocket_shutdown_consumers: Vec::new(),
            startup_checks: Vec::new(),
            background_tasks: Vec::new(),
            cleanup_hooks: Vec::new(),
        }
    }

    /// Creates a runtime app from host-neutral app metadata and an HTTP router.
    ///
    /// This helper is the native-server bridge for app-kit descriptors. It
    /// keeps every app from repeating app-name validation and conversion while
    /// still requiring apps to provide their own behavior router.
    pub fn from_app_metadata(
        metadata: &AppMetadata,
        http_router: Router,
    ) -> Result<Self, StartupError> {
        Ok(Self::new(
            AppName::new(metadata.name().as_str())?,
            http_router,
        ))
    }

    /// Returns the validated app name.
    pub fn name(&self) -> &AppName {
        &self.name
    }

    /// Returns a typed handle that other apps can depend on.
    pub fn handle(&self) -> RuntimeAppHandle {
        RuntimeAppHandle {
            name: self.name.clone(),
        }
    }

    /// Sets the server-composition HTTP mount path for this app.
    pub fn with_http_mount(mut self, value: AppHttpMountPath) -> Self {
        self.http_mount = value;
        self
    }

    /// Sets the per-app CORS policy applied to this mount only.
    ///
    /// The server runtime wraps this app's router with a [`tower_http`] CORS
    /// layer derived from the [`AppCorsConfig`] before nesting. Apps that share
    /// a listener can therefore advertise different allowed origins without
    /// colliding at the listener-level CORS policy.
    pub fn with_cors(mut self, value: AppCorsConfig) -> Self {
        self.http_cors = Some(value);
        self
    }

    /// Adds a required app dependency.
    ///
    /// Dependencies are resolved before listener binding. Unknown dependency
    /// names, duplicate dependencies, and cycles fail closed before readiness.
    pub fn with_dependency(mut self, value: RuntimeAppDependency) -> Self {
        self.dependencies.push(value);
        self
    }

    /// Registers a callback receiving the process shutdown token.
    pub fn with_websocket_shutdown_consumer(
        mut self,
        consumer: impl Fn(ShutdownToken) + Send + Sync + 'static,
    ) -> Self {
        self.websocket_shutdown_consumers.push(Arc::new(consumer));
        self
    }

    /// Adds an app-specific startup check that must pass before readiness.
    pub fn with_startup_check(mut self, value: RuntimeStartupCheck) -> Self {
        self.startup_checks.push(value);
        self
    }

    /// Adds an app-specific background task supervised by the server runtime.
    pub fn with_background_task(mut self, value: RuntimeBackgroundTask) -> Self {
        self.background_tasks.push(value);
        self
    }

    /// Adds an app-owned cleanup hook executed during runtime shutdown.
    pub fn with_cleanup_hook(mut self, value: RuntimeCleanupHook) -> Self {
        self.cleanup_hooks.push(value);
        self
    }

    pub(crate) fn into_parts(self) -> RuntimeAppLocalParts {
        RuntimeAppLocalParts {
            http_mount: self.http_mount,
            http_router: self.http_router,
            http_cors: self.http_cors,
            websocket_shutdown_consumers: self.websocket_shutdown_consumers,
            startup_checks: self.startup_checks,
            background_tasks: self.background_tasks,
            cleanup_hooks: self.cleanup_hooks,
        }
    }
}

pub(crate) struct RuntimeAppLocalParts {
    pub(crate) http_mount: AppHttpMountPath,
    pub(crate) http_router: Router,
    pub(crate) http_cors: Option<AppCorsConfig>,
    pub(crate) websocket_shutdown_consumers:
        Vec<Arc<dyn Fn(ShutdownToken) + Send + Sync + 'static>>,
    pub(crate) startup_checks: Vec<RuntimeStartupCheck>,
    pub(crate) background_tasks: Vec<RuntimeBackgroundTask>,
    pub(crate) cleanup_hooks: Vec<RuntimeCleanupHook>,
}

pub(crate) struct RuntimeAppParts {
    pub(crate) http_router: Router,
    pub(crate) ordered_app_names: Vec<AppName>,
    pub(crate) websocket_shutdown_consumers:
        Vec<Arc<dyn Fn(ShutdownToken) + Send + Sync + 'static>>,
    pub(crate) startup_checks: Vec<RuntimeStartupCheck>,
    pub(crate) background_tasks: Vec<RuntimeBackgroundTask>,
    pub(crate) cleanup_hooks: Vec<RuntimeAppCleanup>,
}

pub(crate) struct RuntimeAppCleanup {
    pub(crate) app_name: AppName,
    pub(crate) hook: RuntimeCleanupHook,
}

pub(crate) fn collect_apps(apps: Vec<RuntimeApp>) -> Result<RuntimeAppParts, ServerRuntimeError> {
    validate_runtime_apps(apps.as_slice())?;

    let apps = order_runtime_apps(apps)?;

    let mut http_router = Router::new();
    let mut ordered_app_names = Vec::with_capacity(apps.len());
    let mut websocket_shutdown_consumers = Vec::new();
    let mut startup_checks = Vec::new();
    let mut background_tasks = Vec::new();
    let mut cleanup_hooks = Vec::new();

    for app in apps {
        let app_name = app.name.clone();
        ordered_app_names.push(app_name.clone());
        let parts = app.into_parts();
        let app_router = match parts.http_cors.as_ref().and_then(app_cors_layer) {
            Some(layer) => parts.http_router.layer(layer),
            None => parts.http_router,
        };
        http_router = if parts.http_mount.is_root() {
            http_router.merge(app_router)
        } else {
            http_router.nest(parts.http_mount.as_str(), app_router)
        };
        startup_checks.extend(parts.startup_checks);
        websocket_shutdown_consumers.extend(parts.websocket_shutdown_consumers);
        background_tasks.extend(parts.background_tasks);
        cleanup_hooks.extend(
            parts
                .cleanup_hooks
                .into_iter()
                .map(|hook| RuntimeAppCleanup {
                    app_name: app_name.clone(),
                    hook,
                }),
        );
    }

    Ok(RuntimeAppParts {
        http_router,
        ordered_app_names,
        websocket_shutdown_consumers,
        startup_checks,
        background_tasks,
        cleanup_hooks,
    })
}

pub(crate) fn validate_runtime_apps(apps: &[RuntimeApp]) -> Result<(), ServerRuntimeError> {
    for (left_index, left) in apps.iter().enumerate() {
        for right in apps.iter().skip(left_index + 1) {
            if left.name == right.name {
                return Err(ServerRuntimeError::AppComposition {
                    reason: RuntimeAppCompositionErrorReason::DuplicateAppName,
                });
            }

            if left.http_mount == right.http_mount {
                return Err(ServerRuntimeError::AppComposition {
                    reason: RuntimeAppCompositionErrorReason::DuplicateHttpMount,
                });
            }
        }

        validate_app_dependencies(left, apps)?;
    }

    Ok(())
}

fn validate_app_dependencies(
    app: &RuntimeApp,
    apps: &[RuntimeApp],
) -> Result<(), ServerRuntimeError> {
    for (left_index, left) in app.dependencies.iter().enumerate() {
        if find_app_index(apps, &left.name).is_none() {
            return Err(ServerRuntimeError::AppComposition {
                reason: RuntimeAppCompositionErrorReason::UnknownDependency,
            });
        }

        for right in app.dependencies.iter().skip(left_index + 1) {
            if left == right {
                return Err(ServerRuntimeError::AppComposition {
                    reason: RuntimeAppCompositionErrorReason::DuplicateDependency,
                });
            }
        }
    }

    Ok(())
}

fn order_runtime_apps(apps: Vec<RuntimeApp>) -> Result<Vec<RuntimeApp>, ServerRuntimeError> {
    let mut emitted = vec![false; apps.len()];
    let mut ordered_indexes = Vec::with_capacity(apps.len());

    while ordered_indexes.len() < apps.len() {
        let mut progressed = false;

        for (index, app) in apps.iter().enumerate() {
            if emitted[index] {
                continue;
            }

            let dependencies_ready =
                app.dependencies.iter().all(|dependency| {
                    match find_app_index(&apps, &dependency.name) {
                        Some(dependency_index) => emitted[dependency_index],
                        None => false,
                    }
                });

            if dependencies_ready {
                emitted[index] = true;
                ordered_indexes.push(index);
                progressed = true;
            }
        }

        if !progressed {
            return Err(ServerRuntimeError::AppComposition {
                reason: RuntimeAppCompositionErrorReason::DependencyCycle,
            });
        }
    }

    let mut app_slots = apps.into_iter().map(Some).collect::<Vec<_>>();
    let mut ordered_apps = Vec::with_capacity(app_slots.len());

    for index in ordered_indexes {
        if let Some(app) = app_slots[index].take() {
            ordered_apps.push(app);
        }
    }

    Ok(ordered_apps)
}

fn find_app_index(apps: &[RuntimeApp], name: &AppName) -> Option<usize> {
    apps.iter().position(|app| app.name == *name)
}

#[cfg(test)]
mod tests;
