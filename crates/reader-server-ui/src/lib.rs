//! Compile-time embedded frontend assets.

#[derive(Clone, Copy, Debug)]
pub struct Asset {
    pub path: &'static str,
    pub content_type: &'static str,
    pub bytes: &'static [u8],
}

include!(concat!(env!("OUT_DIR"), "/assets.rs"));

pub fn asset(request_path: &str) -> Option<Asset> {
    let normalized = if request_path == "/" {
        "/index.html"
    } else {
        request_path
    };
    ASSETS
        .iter()
        .copied()
        .find(|asset| asset.path == normalized)
        .or_else(|| {
            let is_api = normalized == "/api" || normalized.starts_with("/api/");
            let has_extension = normalized
                .rsplit('/')
                .next()
                .is_some_and(|part| part.contains('.'));
            if !is_api && !has_extension {
                ASSETS
                    .iter()
                    .copied()
                    .find(|asset| asset.path == "/index.html")
            } else {
                None
            }
        })
}

pub fn cache_control(asset: Asset) -> &'static str {
    if asset.path == "/index.html" {
        "no-cache"
    } else {
        "public, max-age=31536000, immutable"
    }
}

#[cfg(test)]
mod tests;
