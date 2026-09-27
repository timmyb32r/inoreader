//! Seed composition responsibilities.
use super::*;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SeedManifest {
    pub(super) schema_version: u64,
    pub(super) owner_account_id: uuid::Uuid,
    pub(super) workspace_id: uuid::Uuid,
    pub(super) items: Vec<SeedItem>,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SeedItem {
    pub(super) source_inventory_id: String,
    pub(super) idempotency_key: String,
    pub(super) selected: bool,
    pub(super) status: String,
    pub(super) configuration: serde_json::Value,
    pub(super) note: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SourceInventory {
    pub(super) schema_version: u64,
    pub(super) expected_source_count: usize,
    pub(super) origin: serde_json::Value,
    pub(super) sources: Vec<InventorySource>,
    pub(super) pdf_inventory: serde_json::Value,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct InventorySource {
    pub(super) id: String,
    pub(super) configuration: serde_json::Value,
    pub(super) adapter_kind: String,
    pub(super) historical_observations: serde_json::Value,
    pub(super) fixture_coverage: String,
}

pub(super) type PreparedSeed = (String, Subscription, SeedSource, serde_json::Value);

pub(super) fn load_seed(
    path: &std::path::Path,
) -> Result<SeedManifest, Box<dyn std::error::Error>> {
    let raw = std::fs::read(path)?;
    let value: SeedManifest = serde_json::from_slice(&raw)?;
    if value.schema_version != 1 {
        return Err("unsupported seed schema_version".into());
    }
    if value.items.is_empty() {
        return Err("seed manifest has no items".into());
    }
    let mut ids = std::collections::HashSet::new();
    let mut keys = std::collections::HashSet::new();
    for item in &value.items {
        if item.source_inventory_id.is_empty()
            || item.idempotency_key != format!("personal_feed:{}", item.source_inventory_id)
        {
            return Err("seed item has invalid identity or idempotency key".into());
        }
        if !ids.insert(&item.source_inventory_id) || !keys.insert(&item.idempotency_key) {
            return Err("seed manifest contains duplicate identities".into());
        }
        if item.selected && item.status != "reviewed" {
            return Err("selected seed item is not reviewed".into());
        }
        if contains_secret_field(&item.configuration) {
            return Err("seed configuration contains a forbidden secret-like field".into());
        }
        if !matches!(item.status.as_str(), "reviewed" | "unresolved" | "disabled") {
            return Err("seed item has invalid status".into());
        }
        let _ = &item.note;
    }
    Ok(value)
}

fn contains_secret_field(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(values) => values.iter().any(|(key, value)| {
            ["password", "secret", "token", "credential", "private_key"]
                .iter()
                .any(|part| key.to_lowercase().contains(part))
                || contains_secret_field(value)
        }),
        serde_json::Value::Array(values) => values.iter().any(contains_secret_field),
        _ => false,
    }
}

pub(super) fn seed_values(
    seed: SeedManifest,
) -> Result<Vec<PreparedSeed>, Box<dyn std::error::Error>> {
    let mut values = Vec::new();
    for item in seed.items.into_iter().filter(|v| v.selected) {
        let config = item
            .configuration
            .as_object()
            .ok_or("seed configuration must be an object")?;
        let name = config
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or("seed configuration requires name")?;
        let raw_url = config
            .get("feed_url")
            .or_else(|| config.get("url"))
            .and_then(|v| v.as_str())
            .ok_or("seed configuration requires url or feed_url")?;
        let url = Url::parse(raw_url)?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err("seed URL must use HTTP or HTTPS".into());
        }
        let kind = if config.get("feed_url").is_some() {
            SeedSource::Feed
        } else {
            SeedSource::Imported
        };
        let identity = format!(
            "seed/{}/{}/{}",
            seed.owner_account_id, seed.workspace_id, item.idempotency_key
        );
        let id = SubscriptionId::from_uuid(uuid::Uuid::new_v5(
            &uuid::Uuid::NAMESPACE_OID,
            identity.as_bytes(),
        ));
        values.push((
            item.idempotency_key,
            Subscription::new(
                id,
                WorkspaceId::from_uuid(seed.workspace_id),
                url,
                name.to_owned(),
            ),
            kind,
            serde_json::Value::Object(config.clone()),
        ));
    }
    Ok(values)
}
