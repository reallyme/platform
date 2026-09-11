// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Minimal Docker Engine API client over the local Unix socket.
//!
//! The agent only needs a small, audited subset of the Docker API. Keeping this
//! adapter narrow avoids introducing a broad Docker-control surface while still
//! removing shell-like CLI use for container observation and simple actions.

use std::time::Duration;

use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::time::timeout;

use crate::error::{AgentResult, HephaestusAgentError, HephaestusAgentErrorReason};

pub(crate) const DOCKER_SOCKET_PATH: &str = "/var/run/docker.sock";
const MAX_DOCKER_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
const DOCKER_RESPONSE_READ_CHUNK_BYTES: usize = 8 * 1024;

/// One Docker container summary returned by the Engine API.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DockerContainerSummary {
    #[serde(rename = "Names", default)]
    names: Vec<String>,
    #[serde(rename = "Image", default)]
    image: String,
    #[serde(rename = "State", default)]
    state: String,
    #[serde(rename = "Status", default)]
    status: String,
}

impl DockerContainerSummary {
    /// Returns the first container name without Docker's leading slash.
    pub fn primary_name(&self) -> Option<&str> {
        self.names
            .iter()
            .find_map(|name| name.strip_prefix('/').or(Some(name.as_str())))
    }

    /// Returns the image reference.
    pub fn image(&self) -> &str {
        self.image.as_str()
    }

    /// Returns the lifecycle state.
    pub fn state(&self) -> &str {
        self.state.as_str()
    }

    /// Returns the human-readable status.
    pub fn status(&self) -> &str {
        self.status.as_str()
    }
}

#[cfg(test)]
pub(crate) struct DockerContainerSummaryForTest(DockerContainerSummary);

#[cfg(test)]
impl DockerContainerSummaryForTest {
    pub(crate) fn new(names: Vec<String>, image: String, state: String, status: String) -> Self {
        Self(DockerContainerSummary {
            names,
            image,
            state,
            status,
        })
    }

    pub(crate) fn as_summary(&self) -> &DockerContainerSummary {
        &self.0
    }
}

/// Docker Engine version response.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DockerVersionResponse {
    #[serde(rename = "Version", default)]
    version: String,
}

impl DockerVersionResponse {
    /// Returns the Docker Engine version.
    pub fn version(&self) -> Option<&str> {
        non_empty(self.version.as_str())
    }
}

/// Docker image inspect response.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DockerImageInspectResponse {
    #[serde(rename = "RepoDigests", default)]
    repo_digests: Vec<String>,
}

impl DockerImageInspectResponse {
    /// Returns repository digests attached to the local image.
    pub fn repo_digests(&self) -> &[String] {
        self.repo_digests.as_slice()
    }
}

/// Lists all containers through the Docker Engine API.
pub async fn list_containers(
    command_timeout: Duration,
) -> AgentResult<Vec<DockerContainerSummary>> {
    let response = request("GET", "/containers/json?all=true", None, command_timeout).await?;
    serde_json::from_slice(response.body()).map_err(|_error| {
        HephaestusAgentError::new(HephaestusAgentErrorReason::CommandOutputInvalid)
    })
}

/// Returns Docker Engine version through the Docker Engine API.
pub async fn docker_version(command_timeout: Duration) -> AgentResult<Option<String>> {
    let response = request("GET", "/version", None, command_timeout).await?;
    let value: DockerVersionResponse =
        serde_json::from_slice(response.body()).map_err(|_error| {
            HephaestusAgentError::new(HephaestusAgentErrorReason::CommandOutputInvalid)
        })?;
    Ok(value.version().map(str::to_owned))
}

/// Returns local repository digests for an image reference.
pub async fn image_repo_digests(
    image_ref: &str,
    command_timeout: Duration,
) -> AgentResult<Vec<String>> {
    validate_path_fragment(image_ref)?;
    let encoded = url::form_urlencoded::byte_serialize(image_ref.as_bytes()).collect::<String>();
    let path = format!("/images/{encoded}/json");
    let response = request("GET", path.as_str(), None, command_timeout).await?;
    let value: DockerImageInspectResponse =
        serde_json::from_slice(response.body()).map_err(|_error| {
            HephaestusAgentError::new(HephaestusAgentErrorReason::CommandOutputInvalid)
        })?;
    Ok(value.repo_digests)
}

/// Pulls an image through the Docker Engine API.
pub async fn pull_image(image_ref: &str, command_timeout: Duration) -> AgentResult<()> {
    validate_path_fragment(image_ref)?;
    let path = format!(
        "/images/create?fromImage={}",
        url::form_urlencoded::byte_serialize(image_ref.as_bytes()).collect::<String>()
    );
    request("POST", path.as_str(), None, command_timeout)
        .await
        .map(|_response| ())
}

/// Restarts a container through the Docker Engine API.
pub async fn restart_container(container_name: &str, command_timeout: Duration) -> AgentResult<()> {
    validate_path_fragment(container_name)?;
    let path = format!("/containers/{container_name}/restart");
    request("POST", path.as_str(), None, command_timeout)
        .await
        .map(|_response| ())
}

/// Stops a container through the Docker Engine API.
pub async fn stop_container(container_name: &str, command_timeout: Duration) -> AgentResult<()> {
    validate_path_fragment(container_name)?;
    let path = format!("/containers/{container_name}/stop");
    request("POST", path.as_str(), None, command_timeout)
        .await
        .map(|_response| ())
}

struct DockerResponse {
    status_code: u16,
    body: Vec<u8>,
}

impl DockerResponse {
    fn body(&self) -> &[u8] {
        self.body.as_slice()
    }
}

async fn request(
    method: &str,
    path: &str,
    body: Option<&[u8]>,
    command_timeout: Duration,
) -> AgentResult<DockerResponse> {
    validate_method(method)?;
    validate_path(path)?;
    let body = body.unwrap_or_default();
    let mut request = Vec::new();
    request.extend_from_slice(method.as_bytes());
    request.extend_from_slice(b" ");
    request.extend_from_slice(path.as_bytes());
    request.extend_from_slice(b" HTTP/1.1\r\nHost: docker\r\nConnection: close\r\n");
    request.extend_from_slice(b"Content-Length: ");
    request.extend_from_slice(body.len().to_string().as_bytes());
    request.extend_from_slice(b"\r\n\r\n");
    request.extend_from_slice(body);

    let response = timeout(command_timeout, send_request(request))
        .await
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::CommandFailed))??;
    if !(200..=299).contains(&response.status_code) {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::CommandFailed,
        ));
    }
    Ok(response)
}

async fn send_request(request: Vec<u8>) -> AgentResult<DockerResponse> {
    let mut stream = UnixStream::connect(DOCKER_SOCKET_PATH)
        .await
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::CommandFailed))?;
    stream
        .write_all(request.as_slice())
        .await
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::CommandFailed))?;
    let response = read_bounded_response(&mut stream).await?;
    parse_response(response.as_slice())
}

async fn read_bounded_response(stream: &mut UnixStream) -> AgentResult<Vec<u8>> {
    let mut response = Vec::new();
    let mut chunk = [0u8; DOCKER_RESPONSE_READ_CHUNK_BYTES];
    loop {
        let bytes_read = stream.read(chunk.as_mut_slice()).await.map_err(|_error| {
            HephaestusAgentError::new(HephaestusAgentErrorReason::CommandOutputInvalid)
        })?;
        if bytes_read == 0 {
            return Ok(response);
        }
        append_response_chunk(&mut response, &chunk[..bytes_read])?;
    }
}

fn append_response_chunk(response: &mut Vec<u8>, chunk: &[u8]) -> AgentResult<()> {
    // We intentionally cap accumulated response size on each append. A single chunk
    // can overrun the target by up to `DOCKER_RESPONSE_READ_CHUNK_BYTES - 1` before the
    // next guard, which is bounded and acceptable for this polling API.
    let next_len = response
        .len()
        .checked_add(chunk.len())
        .ok_or_else(|| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidNumber))?;
    if next_len > MAX_DOCKER_RESPONSE_BYTES {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::CommandOutputInvalid,
        ));
    }
    response.extend_from_slice(chunk);
    Ok(())
}

fn parse_response(response: &[u8]) -> AgentResult<DockerResponse> {
    let Some(header_end) = find_header_end(response) else {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::CommandOutputInvalid,
        ));
    };
    let header_bytes = &response[..header_end];
    let body_start = header_end
        .checked_add(4)
        .ok_or_else(|| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidNumber))?;
    let headers = std::str::from_utf8(header_bytes).map_err(|_error| {
        HephaestusAgentError::new(HephaestusAgentErrorReason::CommandOutputInvalid)
    })?;
    let status_code = parse_status_code(headers)?;
    let raw_body = &response[body_start..];
    let body = if has_chunked_transfer_encoding(headers) {
        decode_chunked_body(raw_body)?
    } else {
        raw_body.to_vec()
    };
    Ok(DockerResponse { status_code, body })
}

fn has_chunked_transfer_encoding(headers: &str) -> bool {
    headers.lines().skip(1).any(|line| {
        let Some((name, value)) = line.split_once(':') else {
            return false;
        };
        name.trim().eq_ignore_ascii_case("transfer-encoding")
            && value
                .split(',')
                .any(|encoding| encoding.trim().eq_ignore_ascii_case("chunked"))
    })
}

fn parse_status_code(headers: &str) -> AgentResult<u16> {
    let Some(status_line) = headers.lines().next() else {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::CommandOutputInvalid,
        ));
    };
    let Some(code) = status_line.split_whitespace().nth(1) else {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::CommandOutputInvalid,
        ));
    };
    code.parse::<u16>().map_err(|_error| {
        HephaestusAgentError::new(HephaestusAgentErrorReason::CommandOutputInvalid)
    })
}

fn decode_chunked_body(response_body: &[u8]) -> AgentResult<Vec<u8>> {
    let mut decoded = Vec::new();
    let mut cursor = 0usize;

    loop {
        if cursor >= response_body.len() {
            return Err(HephaestusAgentError::new(
                HephaestusAgentErrorReason::CommandOutputInvalid,
            ));
        }
        let line_end = find_crlf(&response_body[cursor..]).ok_or_else(|| {
            HephaestusAgentError::new(HephaestusAgentErrorReason::CommandOutputInvalid)
        })?;
        let size_line_end = cursor
            .checked_add(line_end)
            .ok_or_else(|| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidNumber))?;
        let size_line =
            std::str::from_utf8(&response_body[cursor..size_line_end]).map_err(|_error| {
                HephaestusAgentError::new(HephaestusAgentErrorReason::CommandOutputInvalid)
            })?;
        let Some(size_token) = size_line.split(';').next().map(str::trim) else {
            return Err(HephaestusAgentError::new(
                HephaestusAgentErrorReason::CommandOutputInvalid,
            ));
        };
        if size_token.is_empty() {
            return Err(HephaestusAgentError::new(
                HephaestusAgentErrorReason::CommandOutputInvalid,
            ));
        }
        let chunk_size = usize::from_str_radix(size_token, 16).map_err(|_error| {
            HephaestusAgentError::new(HephaestusAgentErrorReason::CommandOutputInvalid)
        })?;
        cursor = cursor
            .checked_add(line_end)
            .and_then(|value| value.checked_add(2))
            .ok_or_else(|| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidNumber))?;
        if chunk_size == 0 {
            return Ok(decoded);
        }
        let data_end = cursor
            .checked_add(chunk_size)
            .ok_or_else(|| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidNumber))?;
        let crlf_end = data_end
            .checked_add(2)
            .ok_or_else(|| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidNumber))?;
        if crlf_end > response_body.len() || response_body.get(data_end..crlf_end) != Some(b"\r\n")
        {
            return Err(HephaestusAgentError::new(
                HephaestusAgentErrorReason::CommandOutputInvalid,
            ));
        }
        append_response_chunk(&mut decoded, &response_body[cursor..data_end])?;
        cursor = crlf_end;
    }
}

fn find_header_end(response: &[u8]) -> Option<usize> {
    response.windows(4).position(|window| window == b"\r\n\r\n")
}

fn find_crlf(value: &[u8]) -> Option<usize> {
    value.windows(2).position(|window| window == b"\r\n")
}

fn validate_method(value: &str) -> AgentResult<()> {
    if matches!(value, "GET" | "POST" | "DELETE") {
        Ok(())
    } else {
        Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidConfig,
        ))
    }
}

fn validate_path(value: &str) -> AgentResult<()> {
    if value.starts_with('/')
        && value.len() <= 512
        && !value.contains("..")
        && percent_encoding_is_valid(value.as_bytes())
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(
                    byte,
                    b'/' | b'-' | b'_' | b'.' | b':' | b'?' | b'=' | b'&' | b'%' | b'@' | b'+'
                )
        })
    {
        Ok(())
    } else {
        Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidConfig,
        ))
    }
}

fn percent_encoding_is_valid(value: &[u8]) -> bool {
    let mut index = 0usize;
    while index < value.len() {
        if value[index] == b'%' {
            let Some(high_index) = index.checked_add(1) else {
                return false;
            };
            let Some(low_index) = index.checked_add(2) else {
                return false;
            };
            if low_index >= value.len()
                || !value[high_index].is_ascii_hexdigit()
                || !value[low_index].is_ascii_hexdigit()
            {
                return false;
            }
            index = match index.checked_add(3) {
                Some(next) => next,
                None => return false,
            };
        } else {
            index = match index.checked_add(1) {
                Some(next) => next,
                None => return false,
            };
        }
    }
    true
}

fn validate_path_fragment(value: &str) -> AgentResult<()> {
    if value.is_empty()
        || value.len() > 255
        || value.contains("..")
        || value.bytes().any(|byte| {
            !(byte.is_ascii_alphanumeric()
                || matches!(byte, b'-' | b'_' | b'.' | b':' | b'/' | b'@' | b'+'))
        })
    {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidConfig,
        ));
    }
    Ok(())
}

fn non_empty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

#[cfg(test)]
mod tests;
