use url::Url;

#[derive(Debug)]
pub(super) enum Error {
    DeclarationsForbidden,
    InvalidDocument,
    InvalidRoot,
    InvalidUrl,
    UnsupportedUrlScheme,
    InvalidTitle,
}

pub(super) fn outlines(document: &str) -> Result<Vec<(Url, String)>, Error> {
    if document.contains("<!DOCTYPE") || document.contains("<!ENTITY") {
        return Err(Error::DeclarationsForbidden);
    }
    let tree = roxmltree::Document::parse(document).map_err(|_| Error::InvalidDocument)?;
    if tree.root_element().tag_name().name() != "opml" {
        return Err(Error::InvalidRoot);
    }
    let mut values = Vec::new();
    for node in tree
        .descendants()
        .filter(|value| value.is_element() && value.tag_name().name() == "outline")
    {
        let Some(raw) = node.attribute("xmlUrl") else {
            continue;
        };
        let url = Url::parse(raw).map_err(|_| Error::InvalidUrl)?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(Error::UnsupportedUrlScheme);
        }
        let title = node
            .attribute("text")
            .or_else(|| node.attribute("title"))
            .unwrap_or(raw)
            .to_owned();
        if title.is_empty() || title.trim() != title || title.len() > 256 {
            return Err(Error::InvalidTitle);
        }
        values.push((url, title));
    }
    Ok(values)
}

pub(super) fn has_folders(document: &str) -> Result<bool, Error> {
    let tree = roxmltree::Document::parse(document).map_err(|_| Error::InvalidDocument)?;
    Ok(tree.descendants().any(|node| {
        node.is_element()
            && node.tag_name().name() == "outline"
            && node.attribute("xmlUrl").is_none()
            && node.descendants().any(|child| {
                child.is_element()
                    && child.tag_name().name() == "outline"
                    && child.attribute("xmlUrl").is_some()
            })
    }))
}

pub(super) fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
