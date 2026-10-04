// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum RequestKind {
    Health,
    Collection,
    Search,
    CreateCollection,
    GetCollection,
    UpdateCollection,
    DeleteCollection,
    UpsertAlias,
    DeleteAlias,
    UpsertDocument,
    GetDocument,
    DeleteDocument,
}

impl RequestKind {
    pub(super) const fn is_replay_safe(self) -> bool {
        matches!(
            self,
            Self::Health | Self::Search | Self::GetCollection | Self::GetDocument
        )
    }

    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Health => "health",
            Self::Collection => "collection",
            Self::Search => "search",
            Self::CreateCollection => "create_collection",
            Self::GetCollection => "get_collection",
            Self::UpdateCollection => "update_collection",
            Self::DeleteCollection => "delete_collection",
            Self::UpsertAlias => "upsert_alias",
            Self::DeleteAlias => "delete_alias",
            Self::UpsertDocument => "upsert_document",
            Self::GetDocument => "get_document",
            Self::DeleteDocument => "delete_document",
        }
    }
}
