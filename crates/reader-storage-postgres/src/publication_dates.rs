use reader_application::RepositoryError;
/// Feed/listing dates are authoritative for their own origin. Page metadata is
/// used only when that declaration is absent; candidates from different origins
/// are retained so conflicting publications are visible rather than guessed.
pub(crate) fn origin_publication(
    published: Option<String>,
    manifest: Option<&reader_ingest::ContentManifestPointer>,
) -> Result<Vec<reader_core::PublicationEvidence>, RepositoryError> {
    if let Some(raw) = published {
        reader_core::PublicationDate::try_from(raw.clone())
            .map_err(|e| RepositoryError::Storage(e.to_string()))?;
        Ok(vec![reader_core::PublicationEvidence {
            source: "feed / source".into(),
            raw,
        }])
    } else {
        Ok(manifest
            .and_then(|v| v.publication.clone())
            .unwrap_or_default())
    }
}
