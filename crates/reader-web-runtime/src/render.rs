use ammonia::Builder;

/// Raw source remains owned by storage. This renderer borrows it and returns a
/// separate display value, so sanitization can never overwrite archived input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SafeRenderedContent {
    sanitized_html: String,
}

impl SafeRenderedContent {
    pub fn from_untrusted_html(raw_source: &str) -> Self {
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
                    "li",
                    "ol",
                    "p",
                    "pre",
                    "section",
                    "span",
                    "strong",
                    "ul",
                ]
                .into_iter()
                .collect(),
            )
            .generic_attributes(["dir", "lang", "title"].into_iter().collect())
            .url_schemes(["http", "https"].into_iter().collect())
            .link_rel(Some("noopener noreferrer"));
        Self {
            sanitized_html: builder.clean(raw_source).to_string(),
        }
    }
    pub fn html(&self) -> &str {
        &self.sanitized_html
    }
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
