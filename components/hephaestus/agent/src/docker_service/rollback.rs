// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::PathBuf;
use std::time::Duration;

use super::file_ops::{
    copy_dir_recursive, path_exists, remove_dir_if_exists, rollback_dir, secret_dir, service_dir,
    set_file_permissions,
};
use super::run_compose;
use crate::error::{AgentResult, HephaestusAgentError, HephaestusAgentErrorReason};

pub(super) struct DeploymentRollback {
    service_id: String,
    service_dir_existed: bool,
    secret_dir_existed: bool,
    backup_root: PathBuf,
    backup_service_dir: PathBuf,
    backup_secret_dir: PathBuf,
}

impl DeploymentRollback {
    pub(super) async fn capture(service_id: &str) -> AgentResult<Self> {
        let backup_root = rollback_dir(service_id);
        let backup_service_dir = backup_root.join("service");
        let backup_secret_dir = backup_root.join("secrets");
        remove_dir_if_exists(backup_root.as_path()).await?;
        tokio::fs::create_dir_all(backup_root.as_path())
            .await
            .map_err(|_error| {
                HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed)
            })?;
        set_file_permissions(backup_root.as_path(), 0o700).await?;

        let current_service_dir = service_dir(service_id);
        let current_secret_dir = secret_dir(service_id);
        let service_dir_existed = path_exists(current_service_dir.as_path()).await;
        let mut secret_dir_existed = path_exists(current_secret_dir.as_path()).await;
        if service_dir_existed {
            copy_dir_recursive(current_service_dir.as_path(), backup_service_dir.as_path()).await?;
        }
        if secret_dir_existed {
            match copy_dir_recursive(current_secret_dir.as_path(), backup_secret_dir.as_path())
                .await
            {
                Ok(()) => {}
                Err(_error) => {
                    // Older or partially applied deployments can leave secret
                    // files owned by the container user without root-group
                    // read permission. The agent intentionally runs without
                    // DAC override, so those files may be unreadable during
                    // rollback capture. Treat that backup as absent rather
                    // than blocking the repair path: the incoming action
                    // resolves fresh secrets from Hephaestus before rewriting
                    // the secret directory.
                    secret_dir_existed = false;
                    remove_dir_if_exists(backup_secret_dir.as_path()).await?;
                }
            }
        }

        Ok(Self {
            service_id: service_id.to_owned(),
            service_dir_existed,
            secret_dir_existed,
            backup_root,
            backup_service_dir,
            backup_secret_dir,
        })
    }

    pub(super) async fn restore(&self, command_timeout: Duration) -> AgentResult<()> {
        let current_service_dir = service_dir(self.service_id.as_str());
        let current_secret_dir = secret_dir(self.service_id.as_str());
        if path_exists(current_service_dir.join("compose.yml").as_path()).await {
            run_compose(
                current_service_dir.as_path(),
                ["down", "--remove-orphans"],
                command_timeout,
            )
            .await?;
        }
        remove_dir_if_exists(current_service_dir.as_path()).await?;
        remove_dir_if_exists(current_secret_dir.as_path()).await?;

        if self.service_dir_existed {
            copy_dir_recursive(
                self.backup_service_dir.as_path(),
                current_service_dir.as_path(),
            )
            .await?;
        }
        if self.secret_dir_existed {
            copy_dir_recursive(
                self.backup_secret_dir.as_path(),
                current_secret_dir.as_path(),
            )
            .await?;
        }
        if self.service_dir_existed
            && path_exists(current_service_dir.join("compose.yml").as_path()).await
        {
            run_compose(
                current_service_dir.as_path(),
                ["up", "-d", "--remove-orphans"],
                command_timeout,
            )
            .await?;
        }
        self.discard().await
    }

    pub(super) async fn discard(&self) -> AgentResult<()> {
        remove_dir_if_exists(self.backup_root.as_path()).await
    }
}
