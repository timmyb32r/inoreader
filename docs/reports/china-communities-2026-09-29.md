# Chinese developer communities — 2026-09-29

Requested: Alibaba Cloud, Tencent Cloud, JD Cloud, Volcengine and Baidu developer communities. Existing Tencent subscription is retained; the existing Aliyun author subscription is a distinct scope and is retained.

| Community | Collection |
|---|---|
| Alibaba Cloud | Public homepage article-title links, exact article URL pattern; server preview found 53 articles |
| Tencent Cloud | Existing subscription `73a55a10-2aa3-5d60-b292-064f4806c234` |
| JD Cloud | Official public article-list API, no recommendation tag; explicitly configured page size 20 |
| Volcengine | Official public latest-articles API; publisher's current page window (20), rather than personalized homepage recommendations |
| Baidu | Public article-list page; article cards and their h3 titles; server preview found 14 articles |

These are current listing windows, not a complete historical import. Newly encountered older publications retain their actual dates where available and appear as new arrivals when first stored. No author or topic filtering is applied.

JD's server browser preview timed out twice; its public API works without credentials and avoids the browser dependency. Volcengine's clickable article cards have no article hrefs, so a selector-only recipe cannot preserve their identities. Small adapters own these two public JSON schemas and use the existing shared outbound transport, retries, proxy policy and instrumentation. Invalid entries, duplicate identities, unsuccessful API responses and invalid timestamps reject the collection before commit. Article IDs and millisecond precision are preserved. Tests cover successful mappings and rejected payloads.

## Completion

`just check-affected`, `just check-release` and `git diff --check` passed. The updated container was deployed and the four-item seed was applied atomically. Production collection succeeded for every new source; final verified library counts: Alibaba 53, Baidu 14, Volcengine 20, JD Cloud 20. Tencent's existing subscription has 7 articles and a successful latest collection. Alibaba fan-out completed through the existing queue. No existing subscriptions or articles were removed, and no commit was created.
