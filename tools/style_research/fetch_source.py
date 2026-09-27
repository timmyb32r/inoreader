#!/usr/bin/env python3
"""Fetch a complete public article snapshot for a frozen research partition.

Requires lxml (available in the bundled research Python). Never consults the
Telegram corpus or credentials. Extraction is a documented derivative; raw HTML
is always retained. Ambiguous content containers fail instead of returning nav or
silently shortening an article. Proxy transport must be explicitly requested.
"""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import ipaddress
import json
from pathlib import Path
import re
import subprocess
from urllib.parse import urlparse

from lxml import html


def class_xpath(name):
    return f"//*[contains(concat(' ',normalize-space(@class),' '),' {name} ')]"


CONTENT_SELECTORS = [
    "//*[@itemprop='articleBody']",
    class_xpath("blog-post-content"),
    class_xpath("article__data"),
    class_xpath("article-content"),
    class_xpath("blog-single-body"),
    class_xpath("article_body"),
    class_xpath("article-body"),
    class_xpath("article__body"),
    class_xpath("post-content"),
    class_xpath("postContent"),
    class_xpath("post-body"),
    class_xpath("entry-content"),
    "//*[@id='post-content']",
    "//*[@id='article-content']",
    "//article",
    "//main",
]
BLOCK_TAGS = frozenset("p div section article main blockquote pre ul ol li h1 h2 h3 h4 h5 h6 table tr dl dt dd".split())
NON_CONTENT_TAGS = frozenset("script style noscript nav form button".split())


def validate_url(url):
    parsed = urlparse(url)
    if parsed.scheme != "https" or not parsed.hostname or parsed.username or parsed.password:
        raise ValueError("Research source must be an HTTPS URL without userinfo")
    try:
        address = ipaddress.ip_address(parsed.hostname)
    except ValueError:
        return
    if not address.is_global:
        raise ValueError("Research source must not target a private address")


def content_text(element):
    """Retain all selected content, with explicit paragraph/table boundaries."""
    pieces = []

    def walk(node):
        if not isinstance(node.tag, str):
            return
        tag = node.tag.lower()
        if tag in NON_CONTENT_TAGS:
            return
        if tag in BLOCK_TAGS or tag == "br":
            pieces.append("\n")
        if node.text:
            pieces.append(node.text)
        for child in node:
            walk(child)
            if child.tail:
                pieces.append(child.tail)
        if tag in BLOCK_TAGS:
            pieces.append("\n")
        elif tag in {"td", "th"}:
            pieces.append("\t")

    walk(element)
    # HTML layout whitespace is not source prose. Raw content remains in HTML.
    lines = [re.sub(r"[ \t\r\f\v]+", " ", line).strip() for line in "".join(pieces).split("\n")]
    return re.sub(r"\n{3,}", "\n\n", "\n".join(lines)).strip()


def extract(source: bytes, selector: str | None = None, min_text_chars: int = 100):
    document = html.fromstring(source)
    h1s = ["".join(node.itertext()).strip() for node in document.xpath("//h1")]
    metas = document.xpath("//meta[@property='og:title']/@content")
    page_titles = document.xpath("//title/text()")
    selected = None
    used_selector = None
    for candidate in [selector] if selector is not None else CONTENT_SELECTORS:
        elements = document.xpath(candidate)
        if len(elements) == 1 and isinstance(elements[0], html.HtmlElement):
            selected = elements[0]
            used_selector = candidate
            break
        if selector is not None:
            raise ValueError(f"Explicit content XPath matched {len(elements)} nodes, expected exactly one")
    if selected is None:
        raise ValueError("No unambiguous article content container; explicit XPath required")
    title = None
    title_selector = None
    for title_xpath in [".//h1", ".//*[@itemprop='headline']"]:
        found = selected.xpath(title_xpath)
        if len(found) == 1:
            title = "".join(found[0].itertext()).strip()
            title_selector = "selected-content:" + title_xpath
            break
    if not title:
        for title_xpath in ["//*[@itemprop='headline']", class_xpath("post-title"), class_xpath("entry-title")]:
            found = document.xpath(title_xpath)
            if len(found) == 1:
                title = "".join(found[0].itertext()).strip()
                title_selector = title_xpath
                break
    if not title:
        heading_xpath = "//h1[not(ancestor::nav) and (not(ancestor::header) or ancestor::article)]"
        headings = document.xpath(heading_xpath)
        if len(headings) == 1 and "".join(headings[0].itertext()).strip():
            title = "".join(headings[0].itertext()).strip()
            title_selector = heading_xpath
    if not title:
        title = next((value for value in [*metas, *h1s, *page_titles] if value), None)
        title_selector = "og:title then h1 then title"
    if not title:
        raise ValueError("Source has no visible or metadata title")
    text = content_text(selected)
    if len(text) < min_text_chars:
        raise ValueError(f"Selected content has {len(text)} characters, below configured minimum {min_text_chars}")
    canonical = document.xpath("//link[@rel='canonical']/@href")
    return {
        "title": title,
        "title_selector": title_selector,
        "h1_titles": h1s,
        "metadata_titles": metas + page_titles,
        "canonical_url": canonical[0] if canonical else None,
        "text": text,
        "extraction_selector": used_selector,
        "text_chars": len(text),
        "selected_html": html.tostring(selected, encoding="unicode"),
    }


def fetch_article(url: str, key: str, output_root: Path, *, transport="direct", selector=None, timeout=75, min_text_chars=100):
    """Persist source and return provenance. Same key never overwrites a snapshot.

    Caller owns article-title matching and cohort selection. Success means content
    extraction succeeded, not that the author summary/source pair is confirmed.
    """
    validate_url(url)
    if not re.fullmatch(r"[A-Za-z0-9_.-]+", key):
        raise ValueError("Snapshot key must be a simple authored filename component")
    if timeout <= 0 or min_text_chars < 0 or transport not in {"direct", "jina"}:
        raise ValueError("Invalid fetch/extraction configuration")
    output_root = Path(output_root)
    output_root.mkdir(parents=True, exist_ok=True)
    raw_path = output_root / f"{key}.html"
    metadata_path = output_root / f"{key}.json"
    if metadata_path.exists():
        existing = json.loads(metadata_path.read_text())
        if existing["requested_url"] != url or existing["transport"] != transport or existing.get("requested_selector") != selector:
            raise ValueError("Snapshot key already belongs to different source or extraction configuration")
        return existing
    if raw_path.exists():
        raise ValueError("Unrecorded raw snapshot exists; preserve it and use a new key")
    headers_path = output_root / f"{key}.headers"
    fetch_url = url if transport == "direct" else "https://r.jina.ai/" + url
    result = {
        "schema_version": 1,
        "requested_url": url,
        "transport": transport,
        "requested_selector": selector,
        "fetch_time": datetime.now(timezone.utc).isoformat(),
        "raw_path": str(raw_path),
        "headers_path": str(headers_path),
        "status": "fetch_failed",
        "extraction_policy": "Complete selected content; paragraph/table boundaries retained; HTML layout whitespace normalized; script/style/noscript/nav/form/button excluded; raw HTML retained; no length truncation",
    }
    command = [
        "curl", "--silent", "--show-error", "--location", "--proto", "=https", "--proto-redir", "=https",
        "--max-redirs", "5", "--connect-timeout", "15", "--max-time", str(timeout),
        "--dump-header", str(headers_path), "--output", str(raw_path),
        "--write-out", "%{url_effective}\n%{http_code}",
    ]
    if transport == "jina":
        command.extend(["--header", "X-Return-Format: html"])
    completed = subprocess.run([*command, fetch_url], text=True, capture_output=True)
    if completed.returncode:
        result["error"] = f"curl {completed.returncode}: {completed.stderr.strip()}"
    else:
        effective, status = completed.stdout.rsplit("\n", 1)
        result.update(effective_url=effective, http_status=int(status), raw_bytes=raw_path.stat().st_size)
        if int(status) != 200:
            result["error"] = f"HTTP {status}"
        else:
            try:
                extracted = extract(raw_path.read_bytes(), selector, min_text_chars)
                text_path = output_root / f"{key}.txt"
                text_path.write_text(extracted.pop("text") + "\n")
                selected_path = output_root / f"{key}.content.html"
                selected_path.write_text(extracted.pop("selected_html"))
                result.update(extracted, status="extracted_unverified_pair", text_path=str(text_path), selected_html_path=str(selected_path))
            except (ValueError, html.etree.Error) as error:
                result.update(status="extraction_failed", error=str(error))
    metadata_path.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    return result


def derive_article(metadata_path: Path, key: str, *, selector=None, min_text_chars=100):
    """Re-extract retained raw bytes into a new immutable derivative, no network."""
    if not re.fullmatch(r"[A-Za-z0-9_.-]+", key):
        raise ValueError("Derivative key must be a simple authored filename component")
    metadata_path = Path(metadata_path)
    original = json.loads(metadata_path.read_text())
    output = metadata_path.parent
    destination = output / f"{key}.json"
    if destination.exists():
        raise ValueError("Derivative already exists; preserve it and choose a new key")
    extracted = extract(Path(original["raw_path"]).read_bytes(), selector, min_text_chars)
    text_path = output / f"{key}.txt"
    text_path.write_text(extracted.pop("text") + "\n")
    selected_path = output / f"{key}.content.html"
    selected_path.write_text(extracted.pop("selected_html"))
    result = {**original, **extracted, "derived_from": str(metadata_path), "requested_selector": selector,
              "status": "extracted_unverified_pair", "text_path": str(text_path),
              "selected_html_path": str(selected_path)}
    result.pop("error", None)
    destination.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--url", required=True)
    parser.add_argument("--key", required=True)
    parser.add_argument("--output-root", type=Path, default=Path(".inoreader-state/style-research/sources"))
    parser.add_argument("--transport", choices=("direct", "jina"), default="direct")
    parser.add_argument("--selector", help="Explicit XPath selecting exactly one full article body")
    parser.add_argument("--timeout", type=int, default=75)
    parser.add_argument("--min-text-chars", type=int, default=100)
    args = parser.parse_args()
    print(json.dumps(fetch_article(args.url, args.key, args.output_root, transport=args.transport, selector=args.selector, timeout=args.timeout, min_text_chars=args.min_text_chars), ensure_ascii=False))


if __name__ == "__main__":
    main()
