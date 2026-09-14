// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! URL construction for fixed Typesense API operations.

use super::TypesenseClient;
use crate::typesense::{CollectionName, DocumentId, TypesenseEndpoint};

impl TypesenseClient {
    pub(super) fn collection_documents_search_url(
        &self,
        endpoint: &TypesenseEndpoint,
        collection: &CollectionName,
    ) -> String {
        format!(
            "{}/collections/{}/documents/search",
            endpoint.as_str(),
            collection.as_str()
        )
    }

    pub(super) fn health_url(&self, endpoint: &TypesenseEndpoint) -> String {
        format!("{}/health", endpoint.as_str())
    }

    pub(super) fn collection_documents_url(
        &self,
        endpoint: &TypesenseEndpoint,
        collection: &CollectionName,
    ) -> String {
        format!(
            "{}/collections/{}/documents",
            endpoint.as_str(),
            collection.as_str()
        )
    }

    pub(super) fn collection_document_url(
        &self,
        endpoint: &TypesenseEndpoint,
        collection: &CollectionName,
        document_id: &DocumentId,
    ) -> String {
        format!(
            "{}/collections/{}/documents/{}",
            endpoint.as_str(),
            collection.as_str(),
            document_id.as_str()
        )
    }

    pub(super) fn collection_documents_import_url(
        &self,
        endpoint: &TypesenseEndpoint,
        collection: &CollectionName,
    ) -> String {
        format!(
            "{}/collections/{}/documents/import",
            endpoint.as_str(),
            collection.as_str()
        )
    }

    pub(super) fn multi_search_url(&self, endpoint: &TypesenseEndpoint) -> String {
        format!("{}/multi_search", endpoint.as_str())
    }

    pub(super) fn collections_url(&self, endpoint: &TypesenseEndpoint) -> String {
        format!("{}/collections", endpoint.as_str())
    }

    pub(super) fn collection_url(
        &self,
        endpoint: &TypesenseEndpoint,
        collection: &CollectionName,
    ) -> String {
        format!("{}/collections/{}", endpoint.as_str(), collection.as_str())
    }

    pub(super) fn collection_alias_url(
        &self,
        endpoint: &TypesenseEndpoint,
        alias: &CollectionName,
    ) -> String {
        format!("{}/aliases/{}", endpoint.as_str(), alias.as_str())
    }
}
