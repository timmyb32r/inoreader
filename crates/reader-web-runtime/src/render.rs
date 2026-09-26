use ammonia::{Builder, Url, UrlRelative};

/// Raw source remains owned by storage. This renderer borrows it and returns a
/// separate display value, so sanitization can never overwrite archived input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SafeRenderedContent {
    sanitized_html: String,
}

impl SafeRenderedContent {
    pub fn from_untrusted_html(raw_source: &str) -> Self {
        Self::from_untrusted_html_with_base(raw_source, None)
    }

    pub fn from_untrusted_html_with_base(raw_source: &str, base_url: Option<&str>) -> Self {
        let decoded = decode_escaped_markup(raw_source);
        let mut builder = Builder::default();
        builder
            .tags(
                [
                    "a",
                    "article",
                    "blockquote",
                    "br",
                    "code",
                    "div",
                    "em",
                    "h1",
                    "h2",
                    "h3",
                    "h4",
                    "h5",
                    "h6",
                    "img",
                    "li",
                    "ol",
                    "p",
                    "picture",
                    "pre",
                    "section",
                    "source",
                    "span",
                    "strong",
                    "ul",
                ]
                .into_iter()
                .collect(),
            )
            .generic_attributes(["dir", "lang", "title"].into_iter().collect())
            .add_tag_attributes("a", &["href"])
            .add_tag_attributes(
                "img",
                &["alt", "height", "loading", "src", "srcset", "width"],
            )
            .add_tag_attributes("source", &["media", "srcset", "type"])
            .url_schemes(["http", "https"].into_iter().collect())
            .link_rel(Some("noopener noreferrer"));
        if let Some(base_url) = base_url.and_then(|value| Url::parse(value).ok()) {
            builder.url_relative(UrlRelative::RewriteWithBase(base_url));
        }
        Self {
            sanitized_html: builder.clean(&decoded).to_string(),
        }
    }
    pub fn html(&self) -> &str {
        &self.sanitized_html
    }
}

fn decode_escaped_markup(value: &str) -> String {
    let mut decoded = value.to_owned();
    for _ in 0..2 {
        let next = decode_html_entities_once(&decoded);
        if next == decoded {
            break;
        }
        decoded = next;
    }
    decoded
}

fn decode_html_entities_once(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(offset) = rest.find('&') {
        output.push_str(&rest[..offset]);
        rest = &rest[offset..];
        let Some(end) = rest.find(';').filter(|end| *end <= 12) else {
            output.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" | "#39" => Some('\''),
            "nbsp" => Some('\u{a0}'),
            value if value.starts_with("#x") || value.starts_with("#X") => {
                u32::from_str_radix(&value[2..], 16)
                    .ok()
                    .and_then(char::from_u32)
            }
            value if value.starts_with('#') => value[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        if let Some(character) = decoded {
            output.push(character);
            rest = &rest[end + 1..];
        } else {
            output.push('&');
            rest = &rest[1..];
        }
    }
    output.push_str(rest);
    output
}

/// Attributes for an iframe that displays preview/article markup. Omitting both
/// `allow-scripts` and `allow-same-origin` gives the document an opaque origin
/// and prevents script execution in the application origin.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SandboxContract;

impl SandboxContract {
    pub const IFRAME_SANDBOX: &'static str = "";
    pub const DOCUMENT_CSP: &'static str = "default-src 'none'; img-src 'none'; media-src 'none'; frame-src 'none'; object-src 'none'; script-src 'none'; style-src 'none'; form-action 'none'; base-uri 'none'";
    pub fn allows_scripts(self) -> bool {
        false
    }
    pub fn has_opaque_origin(self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests;
