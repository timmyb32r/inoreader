use super::*;
use reader_core::resolve_publication;
fn extract(html: &str) -> Vec<PublicationEvidence> {
    extract_publication(html, &Url::parse("https://example.com/post").unwrap())
}
#[test]
fn metadata_and_jsonld_are_scoped_and_keep_original_evidence() {
    let values = extract(
        r#"<meta property="article:published_time" content="2026-09-27T10:00:00+03:00"><script type="application/ld+json">{"@graph":[{"@type":"BlogPosting","url":"/post","datePublished":"2026-09-27T07:00:00Z","dateModified":"2026-09-28"},{"@type":"BlogPosting","url":"/other","datePublished":"2025-01-01"}]}</script>"#,
    );
    assert_eq!(values.len(), 2);
    assert_eq!(values[0].raw, "2026-09-27T10:00:00+03:00");
    assert!(resolve_publication(&values).is_some());
}
#[test]
fn modified_and_comment_dates_do_not_become_publication_dates() {
    assert!(extract(r#"<meta property="article:modified_time" content="2026-09-27"><article><time class="updated" datetime="2026-09-27"></time><div class="comments"><time datetime="2026-09-26"></time></div></article>"#).is_empty());
    assert!(extract(r#"<script type="application/ld+json">{"@type":"Organization","datePublished":"2020-01-01"}</script>"#).is_empty());
    assert!(extract("Copyright 2026").is_empty());
}
#[test]
fn day_only_and_local_dates_never_get_fabricated_timezone_or_midnight() {
    let values = extract(r#"<article><time datetime="2026-09-27">Sep 27</time></article>"#);
    assert_eq!(resolve_publication(&values).unwrap().as_str(), "2026-09-27");
    let values = extract(r#"<span itemprop="datePublished">2026-09-27 12:34:56</span>"#);
    assert_eq!(
        resolve_publication(&values).unwrap().as_str(),
        "2026-09-27T12:34:56"
    );
}
#[test]
fn conflicting_and_invalid_evidence_is_preserved_but_not_guessed() {
    let values = extract(
        r#"<meta property="article:published_time" content="2026-09-27"><meta itemprop="datePublished" content="2026-09-26">"#,
    );
    assert_eq!(values.len(), 2);
    assert!(resolve_publication(&values).is_none());
    let values = extract(r#"<meta itemprop="datePublished" content="nonsense">"#);
    assert_eq!(values[0].raw, "nonsense");
    assert!(resolve_publication(&values).is_none());
}

#[test]
fn publisher_bylines_are_host_scoped_and_do_not_use_modification_dates() {
    let cases = [
        ("https://tapdata.io/blog/test", "<div class='learnpost_title-wrapper'><div class='blog-author-name-wrap'><div>Sep 26, 2025</div></div></div>", "2025-09-26"),
        ("https://www.alibabacloud.com/blog/test", "<div class='wrap-main-left'><aside><main><a>Author</a><span>September 23, 2026</span><span>123 views</span></main></aside></div>", "2026-09-23"),
        ("https://oxide.computer/blog/test", "<main><article><div class='text-mono-sm text-secondary'><span class='inline-block'>8 Sep 2022</span></div></article></main>", "2022-09-08"),
    ];
    for (url, html, expected) in cases {
        let evidence = extract_publication(html, &Url::parse(url).unwrap());
        assert_eq!(
            reader_core::resolve_publication(&evidence)
                .unwrap()
                .as_str(),
            expected
        );
        assert!(
            extract_publication(html, &Url::parse("https://other.example/post").unwrap())
                .is_empty()
        );
    }
    assert!(extract_publication("<meta name='date' content='2026-09-23'><meta property='og:article:modified_time' content='2026-09-24'>", &Url::parse("https://martinfowler.com/post").unwrap()).is_empty());
}

#[test]
fn release_and_publisher_headers_exclude_commit_and_sidebar_dates() {
    let cases = [
        ("https://github.com/owner/repo/releases/tag/v1", "<div data-pjax='#repo-content-pjax-container'><div><div><relative-time class='no-wrap' datetime='2026-08-03T08:51:21Z'></relative-time></div></div></div><div class='signed-commit-footer'><relative-time datetime='2026-07-30T22:16:32Z'></relative-time></div>", "2026-08-03T08:51:21Z"),
        ("https://tech.meituan.com/post", "<div class='vp-post-meta'><span class='vp-post-date'>2026-09-10</span></div><div class='recommended-list'>2025-12-29</div>", "2026-09-10"),
        ("https://postgrespro.ru/blog/post", "<span class='description'><time datetime='2026-04-13'></time></span>", "2026-04-13"),
        ("https://research.yandex.com/blog/post", "<aside><div class='ArticleProps_wrapper__a PostHead_date__b'><div class='ArticleProps_value__c'>November 24, 2024</div></div></aside>", "2024-11-24"),
    ];
    for (url, html, expected) in cases {
        let evidence = extract_publication(html, &Url::parse(url).unwrap());
        assert_eq!(resolve_publication(&evidence).unwrap().as_str(), expected);
        assert!(
            extract_publication(html, &Url::parse("https://other.example/post").unwrap())
                .is_empty()
        );
    }
}

#[test]
fn huawei_publication_bylines_retain_time_precision_and_offsets() {
    let html = r#"<span class="article-write-time isPc"> 发表于 2026/06/09 08:50:02</span><span class="article-write-time isMb">2026/06/09</span>"#;
    let blog = Url::parse("https://bbs.huaweicloud.com/blogs/478868").unwrap();
    let evidence = extract_publication(html, &blog);
    assert_eq!(evidence[0].raw, " 发表于 2026/06/09 08:50:02");
    assert_eq!(
        resolve_publication(&evidence).unwrap().as_str(),
        "2026-06-09T08:50:02"
    );
    assert!(extract(html).is_empty());
    assert!(extract_publication(&format!("{html}{html}"), &blog).is_empty());
    let news =
        Url::parse("https://www.huaweicloud.com/intl/en-us/news/20260918131810473.html").unwrap();
    let evidence = extract_publication(
        r#"<body><time datetime="2026-09-18T13:31:18.000+08:00">2026-09-18</time></body>"#,
        &news,
    );
    assert_eq!(
        resolve_publication(&evidence).unwrap().as_str(),
        "2026-09-18T13:31:18.000+08:00"
    );
}
