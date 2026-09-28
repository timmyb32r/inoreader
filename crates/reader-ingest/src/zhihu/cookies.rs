use super::Error;
use http::HeaderValue;
use std::collections::HashSet;
/// Validated, opaque session. No Debug/Serialize: neither diagnostics nor DTOs may
/// expose it. Input is retained encrypted verbatim; domain/path scope is enforced
/// when creating the Cookie header. Cookie values are never trimmed or decoded.
pub struct Cookies(Vec<(String, String, String)>);
impl Cookies {
    pub fn parse(raw: &str) -> Result<Self, Error> {
        let mut values = Vec::new();
        let mut keys = HashSet::new();
        if raw.contains('\t') {
            for line in raw.lines() {
                let cells: Vec<_> = line.split('\t').collect();
                if cells.len() != 12
                    || !matches!(
                        cells[2],
                        ".zhihu.com" | "zhihu.com" | "www.zhihu.com" | ".www.zhihu.com"
                    )
                    || !cells[3].starts_with('/')
                {
                    return Err(Error::InvalidCookies);
                }
                if !keys.insert((cells[0], cells[2], cells[3])) {
                    return Err(Error::InvalidCookies);
                }
                values.push((
                    cells[0].to_owned(),
                    cells[1].to_owned(),
                    cells[3].to_owned(),
                ));
            }
        } else {
            for pair in raw.split(';') {
                let pair = pair.strip_prefix(' ').unwrap_or(pair);
                let (name, value) = pair.split_once('=').ok_or(Error::InvalidCookies)?;
                if !keys.insert((name, "www.zhihu.com", "/")) {
                    return Err(Error::InvalidCookies);
                }
                values.push((name.into(), value.into(), "/".into()));
            }
        }
        if !values
            .iter()
            .any(|(n, v, p)| n == "z_c0" && !v.is_empty() && p == "/")
        {
            return Err(Error::InvalidCookies);
        }
        for (name, value, path) in &values {
            // Chromium preserves comma-separated analytics values and quoted
            // cookie values. Validate their contents without changing the bytes
            // that are stored or sent; semicolons and controls remain forbidden.
            let octets = value
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .unwrap_or(value);
            if name.is_empty()
                || !name
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&c))
                || !octets
                    .bytes()
                    .all(|c| matches!(c,0x21|0x23..=0x3a|0x3c..=0x5b|0x5d..=0x7e))
                || path.contains(['\r', '\n', '\t'])
            {
                return Err(Error::InvalidCookies);
            }
        }
        Ok(Self(values))
    }
    pub(super) fn header(&self, path: &str) -> Result<HeaderValue, Error> {
        let raw = self
            .0
            .iter()
            .filter(|(_, _, scope)| {
                path == scope
                    || (path.starts_with(scope)
                        && (scope.ends_with('/') || path[scope.len()..].starts_with('/')))
            })
            .map(|(n, v, _)| format!("{n}={v}"))
            .collect::<Vec<_>>()
            .join("; ");
        let mut value = HeaderValue::from_str(&raw).map_err(|_| Error::InvalidCookies)?;
        value.set_sensitive(true);
        Ok(value)
    }
}
