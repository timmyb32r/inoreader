//! Publication metadata is extracted before HTML sanitization. HTTP modification
//! dates, copyright years, URL dates, and Atom updated are not publication dates.
use reader_core::PublicationEvidence;
use scraper::{ElementRef, Html, Selector};
use serde_json::Value;
use url::Url;

pub fn extract_publication(html: &str, url: &Url) -> Vec<PublicationEvidence> {
    let document = Html::parse_document(html);
    let mut evidence = Vec::new();
    let selector = Selector::parse("meta").expect("static selector");
    for node in document.select(&selector) {
        let name = node
            .value()
            .attr("property")
            .or_else(|| node.value().attr("name"))
            .or_else(|| node.value().attr("itemprop"))
            .unwrap_or_default()
            .to_ascii_lowercase();
        if [
            "article:published_time",
            "datepublished",
            "pubdate",
            "publishdate",
            "publication_date",
            "dc.date.issued",
            "dcterms.issued",
            "parsely-pub-date",
            "sailthru.date",
        ]
        .contains(&name.as_str())
        {
            if let Some(raw) = node.value().attr("content") {
                evidence.push(PublicationEvidence {
                    source: format!("meta:{name}"),
                    raw: raw.into(),
                });
            }
        }
    }
    let selector = Selector::parse("script[type='application/ld+json']").expect("static selector");
    for script in document.select(&selector) {
        if let Ok(value) = serde_json::from_str::<Value>(&script.inner_html()) {
            json_dates(&value, url, &mut evidence);
        }
    }
    let selector = Selector::parse("[itemprop~='datePublished']:not(meta), time[pubdate], time.published, .entry-date.published").expect("static selector");
    for node in document.select(&selector) {
        if !unrelated(node) {
            evidence.push(node_date(node, "html:datePublished"));
        }
    }
    if evidence.is_empty() {
        // Verified publisher bylines, scoped to their hosts and article headers.
        // Never search arbitrary dates in article prose or recommendation cards.
        let selector = match url.host_str() {
            Some("tapdata.io" | "www.tapdata.io") => {
                Some(".learnpost_title-wrapper .blog-author-name-wrap > div")
            }
            Some("www.alibabacloud.com" | "alibabacloud.com") => {
                Some(".wrap-main-left > aside > main > span:first-of-type")
            }
            Some("oxide.computer") => {
                Some("main > article .text-mono-sm.text-secondary > span.inline-block")
            }
            Some("tech.meituan.com") => Some(".vp-post-meta .vp-post-date"),
            Some("postgrespro.ru") => Some("span.description > time[datetime]"),
            Some("research.yandex.com") => Some("aside [class*='PostHead_date__'] > [class*='ArticleProps_value__']"),
            Some("github.com") if url.path().contains("/releases/tag/") => Some("[data-pjax='#repo-content-pjax-container'] > div > div > relative-time.no-wrap[datetime]"),
            _ => None,
        };
        if let Some(selector) = selector {
            let selector = Selector::parse(selector).expect("static publisher selector");
            let nodes: Vec<_> = document.select(&selector).collect();
            if nodes.len() == 1 {
                evidence.push(node_date(nodes[0], "html:publisher-byline"));
            }
        }
    }
    if evidence.is_empty() {
        // A unique article-scoped timestamp can be used; related cards, comments
        // and explicitly modified dates are excluded. Multiple times remain unknown.
        let selector = Selector::parse("article time[datetime], main time[datetime]")
            .expect("static selector");
        let times: Vec<_> = document
            .select(&selector)
            .filter(|node| !unrelated(*node))
            .collect();
        if times.len() == 1 {
            evidence.push(node_date(times[0], "html:article-time"));
        }
    }
    evidence
}

fn node_date(node: ElementRef<'_>, source: &str) -> PublicationEvidence {
    PublicationEvidence {
        source: source.into(),
        raw: node
            .value()
            .attr("datetime")
            .or_else(|| node.value().attr("content"))
            .map(str::to_owned)
            .unwrap_or_else(|| node.text().collect::<String>()),
    }
}
fn unrelated(node: ElementRef<'_>) -> bool {
    std::iter::once(node)
        .chain(node.ancestors().filter_map(ElementRef::wrap))
        .any(|node| {
            let marker = format!(
                "{} {} {}",
                node.value().attr("class").unwrap_or_default(),
                node.value().attr("id").unwrap_or_default(),
                node.value().attr("itemprop").unwrap_or_default()
            )
            .to_ascii_lowercase();
            ["comment", "related", "recommended", "updated", "modified"]
                .iter()
                .any(|part| marker.contains(part))
        })
}
fn json_dates(value: &Value, url: &Url, evidence: &mut Vec<PublicationEvidence>) {
    match value {
        Value::Array(values) => {
            for value in values {
                json_dates(value, url, evidence);
            }
        }
        Value::Object(object) => {
            if let Some(graph) = object.get("@graph") {
                json_dates(graph, url, evidence);
            }
            if let Some(entity) = object.get("mainEntity") {
                json_dates(entity, url, evidence);
            }
            let kind = object.get("@type");
            let article_type = |v: &str| {
                [
                    "Article",
                    "BlogPosting",
                    "NewsArticle",
                    "TechArticle",
                    "Report",
                    "ScholarlyArticle",
                    "WebPage",
                ]
                .contains(&v.rsplit('/').next().unwrap_or(v))
            };
            let is_article = kind.and_then(Value::as_str).is_some_and(article_type)
                || kind.and_then(Value::as_array).is_some_and(|values| {
                    values.iter().filter_map(Value::as_str).any(article_type)
                });
            if !is_article {
                return;
            }
            let identity = object.get("url").and_then(Value::as_str).or_else(|| {
                object
                    .get("mainEntityOfPage")
                    .and_then(|v| v.as_str().or_else(|| v.get("@id").and_then(Value::as_str)))
            });
            if identity.is_some_and(|identity| !same_page(identity, url)) {
                return;
            }
            if let Some(raw) = object.get("datePublished").and_then(Value::as_str) {
                evidence.push(PublicationEvidence {
                    source: "json-ld:datePublished".into(),
                    raw: raw.into(),
                });
            }
        }
        _ => {}
    }
}
fn same_page(value: &str, url: &Url) -> bool {
    url.join(value).is_ok_and(|candidate| {
        candidate.host_str() == url.host_str()
            && candidate.path().trim_end_matches('/') == url.path().trim_end_matches('/')
    })
}

#[cfg(test)]
#[path = "tests/publication.rs"]
mod tests;
