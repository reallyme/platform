// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde::Serialize;

use super::{ImportBatch, ImportFailureReason, parse_import_response_jsonl};
use crate::typesense::{
    DocumentId, SearchDocument, TypesenseError, TypesenseRequestReason, error::TypesenseResult,
};

#[derive(Serialize)]
struct FixtureDocument {
    id: DocumentId,
    handle: String,
}

impl SearchDocument for FixtureDocument {
    fn document_id(&self) -> &DocumentId {
        &self.id
    }
}

#[test]
fn accepts_non_empty_import_batch() -> TypesenseResult<()> {
    let id = DocumentId::parse("profile:alice")?;
    let batch = ImportBatch::new(vec![FixtureDocument {
        id,
        handle: String::from("alice"),
    }]);

    assert!(matches!(batch, Ok(ref value) if value.documents().len() == 1));
    Ok(())
}

#[test]
fn classifies_typesense_update_missing_document_response() -> TypesenseResult<()> {
    let result = parse_import_response_jsonl(
        r#"{"code":404,"error":"Could not find a document with id: page_1","success":false}"#,
    )?;

    assert_eq!(result.lines.len(), 1);
    assert_eq!(
        result.lines[0].failure_reason,
        Some(ImportFailureReason::DocumentNotFound)
    );
    Ok(())
}

#[test]
fn rejects_empty_import_batch() {
    let batch = ImportBatch::<FixtureDocument>::new(Vec::new());

    assert!(matches!(
        batch,
        Err(TypesenseError::InvalidRequest {
            reason: TypesenseRequestReason::EmptyImportBatch
        })
    ));
}

#[test]
fn rejects_oversized_import_batch() -> TypesenseResult<()> {
    let documents = (0..=super::MAX_DOCUMENTS_PER_BATCH)
        .map(|index| {
            let id = DocumentId::parse(format!("document:{index}"))?;
            Ok(FixtureDocument {
                id,
                handle: String::from("fixture"),
            })
        })
        .collect::<TypesenseResult<Vec<_>>>()?;

    let batch = ImportBatch::new(documents);

    assert!(matches!(
        batch,
        Err(TypesenseError::InvalidRequest {
            reason: TypesenseRequestReason::ImportBatchTooLarge
        })
    ));
    Ok(())
}

#[test]
fn parses_jsonl_import_response() -> TypesenseResult<()> {
    let result = parse_import_response_jsonl(
        "{\"success\":true,\"id\":\"profile:alice\"}\n{\"success\":false,\"id\":\"profile:bob\",\"error\":\"Field `handle` is invalid\",\"document\":{\"id\":\"profile:bob\"}}\n",
    )?;

    assert_eq!(result.lines.len(), 2);
    assert!(result.lines[0].success);
    assert!(!result.lines[1].success);
    assert_eq!(
        result.lines[1].failure_reason,
        Some(ImportFailureReason::InvalidField)
    );
    assert_eq!(result.lines[1].failure_message_length, Some(25));
    assert!(result.lines[1].had_document_payload);
    Ok(())
}

#[test]
fn rejects_non_jsonl_array_payload() {
    let result = parse_import_response_jsonl(
        "[{\"success\":true,\"id\":\"profile:alice\"},{\"success\":false,\"id\":\"profile:bob\"}]",
    );

    assert!(matches!(
        result,
        Err(TypesenseError::Transport {
            reason: crate::typesense::TypesenseTransportReason::InvalidResponseBody
        })
    ));
}
