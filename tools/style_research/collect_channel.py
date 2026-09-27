#!/usr/bin/env python3
"""Archive the accessible public Telegram channel without semantic style analysis.

Raw pages are authoritative; normalized text is a derived representation. All
visible posts, including forwards and media-only entries, are retained. An absent
message ID proves only that this traversal did not observe it, never deletion.
No Telegram account or model API credential is used.
"""

from __future__ import annotations

import argparse
from collections import Counter
from dataclasses import dataclass, field
from datetime import datetime, timezone
from html.parser import HTMLParser
import json
from pathlib import Path
import re
import subprocess
import time
from urllib.parse import urlparse


VOID = frozenset("area base br col embed hr img input link meta param source track wbr".split())
BLOCK = frozenset("p div blockquote pre ul ol li h1 h2 h3 h4 h5 h6".split())
SCHEMA_VERSION = 1


@dataclass
class Node:
    tag: str
    attrs: dict[str, str | None]
    start: int
    open_end: int
    close_start: int
    end: int
    children: list[Node] = field(default_factory=list)

    def has_class(self, name: str) -> bool:
        return name in (self.attrs.get("class") or "").split()

    def walk(self):
        yield self
        for child in self.children:
            yield from child.walk()


class Document(HTMLParser):
    """Small structural index; original byte-decoded HTML is never reserialized."""

    def __init__(self, source: str):
        super().__init__(convert_charrefs=False)
        self.source = source
        self.line_offsets = [0]
        self.line_offsets.extend(match.end() for match in re.finditer("\n", source))
        self.root = Node("root", {}, 0, 0, len(source), len(source))
        self.stack = [self.root]
        self.feed(source)
        self.close()

    def source_offset(self) -> int:
        line, column = self.getpos()
        return self.line_offsets[line - 1] + column

    def handle_starttag(self, tag, attrs):
        start = self.source_offset()
        open_end = start + len(self.get_starttag_text())
        node = Node(tag, dict(attrs), start, open_end, open_end, open_end)
        self.stack[-1].children.append(node)
        if tag not in VOID:
            self.stack.append(node)

    def handle_startendtag(self, tag, attrs):
        self.handle_starttag(tag, attrs)
        if tag not in VOID:
            self.stack.pop()

    def handle_endtag(self, tag):
        for index in range(len(self.stack) - 1, 0, -1):
            if self.stack[index].tag == tag:
                close_start = self.source_offset()
                end = self.source.find(">", close_start) + 1
                for node in self.stack[index:]:
                    node.close_start = close_start
                    node.end = end
                del self.stack[index:]
                return

    def inner(self, node: Node) -> str:
        return self.source[node.open_end:node.close_start]

    def outer(self, node: Node) -> str:
        return self.source[node.start:node.end]


class PlainText(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.parts = []
        self.ignored_depth = 0

    def handle_starttag(self, tag, attrs):
        if tag in ("script", "style"):
            self.ignored_depth += 1
        if self.ignored_depth == 0 and (tag == "br" or tag in BLOCK):
            self.parts.append("\n")

    def handle_endtag(self, tag):
        if tag in ("script", "style"):
            self.ignored_depth = max(0, self.ignored_depth - 1)
        elif self.ignored_depth == 0 and tag in BLOCK:
            self.parts.append("\n")

    def handle_data(self, data):
        if self.ignored_depth == 0:
            self.parts.append(data)


def plain_text(fragment: str) -> str:
    parser = PlainText()
    parser.feed(fragment)
    parser.close()
    return "".join(parser.parts).strip()


def find_class(node: Node, name: str) -> Node | None:
    return next((child for child in node.walk() if child.has_class(name)), None)


def is_external_link(url: str) -> bool:
    parsed = urlparse(url)
    return parsed.scheme in ("http", "https") and parsed.hostname not in {
        "t.me", "telegram.me", "telegram.org", "www.telegram.org"
    }


def parse_page(source: str, channel: str, provenance: dict) -> list[dict]:
    document = Document(source)
    posts = []
    for node in document.root.walk():
        if not node.has_class("tgme_widget_message"):
            continue
        post = node.attrs.get("data-post") or ""
        match = re.fullmatch(re.escape(channel) + r"/(\d+)", post)
        if match is None:
            raise ValueError("Unexpected or absent channel/message ID in public preview")
        text_node = find_class(node, "tgme_widget_message_text")
        formatted_html = document.inner(text_node) if text_node else ""
        text = plain_text(formatted_html)
        time_node = next((child for child in node.walk() if child.tag == "time"), None)
        timestamp = time_node.attrs.get("datetime") if time_node else None
        if timestamp is not None:
            datetime.fromisoformat(timestamp)
        forwarded = find_class(node, "tgme_widget_message_forwarded_from")
        links = []
        for anchor in node.walk():
            if anchor.tag != "a" or not anchor.attrs.get("href"):
                continue
            inside_text = text_node is not None and text_node.start < anchor.start < text_node.end
            preview = anchor.has_class("tgme_widget_message_link_preview")
            if inside_text or preview:
                href = anchor.attrs["href"]
                links.append({
                    "url": href,
                    "text": plain_text(document.inner(anchor)),
                    "location": "message_text" if inside_text else "link_preview",
                    "external": is_external_link(href),
                    "evidence": "explicit_html_hyperlink",
                })
        previews = []
        for preview_node in node.walk():
            if preview_node.has_class("link_preview_title"):
                previews.append(plain_text(document.inner(preview_node)))
        external = [link for link in links if link["external"]]
        category = (
            "forwarded" if forwarded is not None
            else "nonforwarded_linked_text" if text and external
            else "nonforwarded_unlinked_text" if text
            else "media_or_unavailable_text"
        )
        posts.append({
            "schema_version": SCHEMA_VERSION,
            "channel": channel,
            "id": int(match.group(1)),
            "permalink": f"https://t.me/{post}",
            "timestamp": timestamp,
            "text": text,
            "formatted_html": formatted_html,
            "raw_post_html": document.outer(node),
            "forwarded": forwarded is not None,
            "forwarded_from_html": document.outer(forwarded) if forwarded else None,
            "links": links,
            "title_candidates": ([{"text": text.splitlines()[0], "evidence": "first_text_line_unverified_title"}] if text else [])
            + [{"text": title, "evidence": "link_preview_title_unverified_article_match"} for title in previews],
            "descriptive_type": category,
            "text_length_chars": len(text),
            "provenance": [provenance],
        })
    return posts


def missing_ranges(ids: list[int]) -> list[list[int]]:
    return [[left + 1, right - 1] for left, right in zip(ids, ids[1:]) if right > left + 1]


def write_json(path: Path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n")


def write_artifacts(directory: Path, posts: dict[int, dict], pages: list[dict], stop_reason: str):
    ordered = [posts[key] for key in sorted(posts)]
    with (directory / "posts.jsonl.tmp").open("w") as output, (directory / "index.jsonl.tmp").open("w") as index:
        for post in ordered:
            output.write(json.dumps(post, ensure_ascii=False) + "\n")
            metadata = {key: value for key, value in post.items() if key not in {"text", "formatted_html", "raw_post_html", "forwarded_from_html"}}
            # Preserve candidate URLs/title metadata without copying preview descriptions.
            metadata["links"] = [{key: value for key, value in link.items() if key != "text"} for link in post["links"]]
            index.write(json.dumps(metadata, ensure_ascii=False) + "\n")
    (directory / "posts.jsonl.tmp").replace(directory / "posts.jsonl")
    (directory / "index.jsonl.tmp").replace(directory / "index.jsonl")
    ids = sorted(posts)
    times = sorted(post["timestamp"] for post in ordered if post["timestamp"])
    inventory = {
        "schema_version": SCHEMA_VERSION,
        "channel": "reading_data_news",
        "collected_at": datetime.now(timezone.utc).isoformat(),
        "stop_reason": stop_reason,
        "post_count": len(ordered),
        "first_id": min(ids) if ids else None,
        "last_id": max(ids) if ids else None,
        "earliest_timestamp": times[0] if times else None,
        "latest_timestamp": times[-1] if times else None,
        "posts_without_timestamp": sum(post["timestamp"] is None for post in ordered),
        "missing_id_ranges_within_observed_span": missing_ranges(ids),
        "descriptive_types": dict(Counter(post["descriptive_type"] for post in ordered)),
        "posts_with_external_links": sum(any(link["external"] for link in post["links"]) for post in ordered),
        "unique_external_urls": len({link["url"] for post in ordered for link in post["links"] if link["external"]}),
        "observed_years": dict(sorted(Counter(post["timestamp"][:4] for post in ordered if post["timestamp"]).items())),
        "pages": pages,
        "limitations": [
            "Public preview coverage only, not an authenticated Telegram export.",
            "Missing IDs are unobserved, not evidence of deletion or a known publication count.",
            "Nonforwarded does not prove authorship; linked does not prove article-summary pairing.",
            "First lines and preview titles are candidates, not verified source titles.",
            "All visible formats retained; semantic authorship/type assignment occurs after partition freeze.",
        ],
    }
    write_json(directory / "inventory.json", inventory)
    return inventory


def fetch(url: str, html_path: Path, headers_path: Path, timeout: int, transport: str = "direct"):
    fetch_url = url if transport == "direct" else "https://r.jina.ai/" + url
    additional_headers = [] if transport == "direct" else ["--header", "X-Return-Format: html"]
    completed = subprocess.run([
        "curl", "--fail", "--silent", "--show-error", "--location", "--proto", "=https",
        "--proto-redir", "=https", "--max-redirs", "3", "--connect-timeout", "15",
        "--max-time", str(timeout), "--dump-header", str(headers_path),
        "--output", str(html_path), "--write-out", "%{url_effective}\n%{http_code}",
        *additional_headers, fetch_url,
    ], text=True, capture_output=True)
    if completed.returncode != 0:
        raise RuntimeError(f"Public fetch failed (curl {completed.returncode}): {completed.stderr.strip()}")
    effective, status = completed.stdout.rsplit("\n", 1)
    expected_host = "t.me" if transport == "direct" else "r.jina.ai"
    if urlparse(effective).hostname != expected_host:
        raise ValueError(f"Public channel archive redirected outside {expected_host}")
    return effective, int(status)


def collect(directory: Path, delay: float, timeout: int, resume: bool, transport: str = "direct"):
    directory.mkdir(parents=True, exist_ok=True)
    raw = directory / "raw"
    raw.mkdir(exist_ok=True)
    posts: dict[int, dict] = {}
    pages = []
    before = None
    while True:
        url = "https://t.me/s/reading_data_news" + (f"?before={before}" if before else "")
        page_key = "latest" if before is None else f"before-{before:08d}"
        html_path = raw / f"{page_key}.html"
        headers_path = raw / f"{page_key}.headers"
        metadata_path = raw / f"{page_key}.json"
        if resume and html_path.exists() and metadata_path.exists():
            provenance = json.loads(metadata_path.read_text())
        else:
            effective, status = fetch(url, html_path, headers_path, timeout, transport)
            provenance = {
                "requested_url": url, "effective_url": effective, "status": status,
                "transport": transport,
                "html_representation": "server_response" if transport == "direct" else "public_proxy_returned_html_not_original_wire_bytes",
                "fetched_at": datetime.now(timezone.utc).isoformat(),
                "raw_html": str(html_path.relative_to(directory)),
                "raw_headers": str(headers_path.relative_to(directory)),
                "raw_bytes": html_path.stat().st_size,
            }
            write_json(metadata_path, provenance)
        parsed = parse_page(html_path.read_text(encoding="utf-8"), "reading_data_news", provenance)
        ids = [post["id"] for post in parsed]
        pages.append({**provenance, "visible_ids": ids})
        new_ids = []
        for post in parsed:
            old = posts.get(post["id"])
            if old is not None:
                if old["formatted_html"] != post["formatted_html"] or old["timestamp"] != post["timestamp"]:
                    write_artifacts(directory, posts, pages, "conflicting_observed_post_versions")
                    raise ValueError(f"Observed conflicting post versions for ID {post['id']}; raw snapshots retained")
                old["provenance"].append(provenance)
            else:
                posts[post["id"]] = post
                new_ids.append(post["id"])
        if not ids:
            reason = "empty_older_public_page"
        elif before is not None and min(ids) >= before:
            reason = "public_preview_returned_no_older_ids"
        elif min(ids) == 1:
            reason = "reached_message_id_1"
        else:
            reason = "in_progress"
        inventory = write_artifacts(directory, posts, pages, reason)
        print(json.dumps({"page": len(pages), "observed": len(posts), "page_min": min(ids) if ids else None, "new": len(new_ids), "status": reason}), flush=True)
        if reason != "in_progress":
            return inventory
        before = min(ids)
        time.sleep(delay)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=Path(".inoreader-state/style-research/corpus"))
    parser.add_argument("--delay-seconds", type=float, default=0.4)
    parser.add_argument("--timeout-seconds", type=int, default=60)
    parser.add_argument("--resume", action="store_true", help="Reparse existing immutable raw pages before fetching missing pages")
    parser.add_argument("--transport", choices=("direct", "jina"), default="direct", help="Explicitly select public Jina HTML proxy when direct Telegram access is unavailable")
    args = parser.parse_args()
    if args.delay_seconds < 0 or args.timeout_seconds <= 0:
        parser.error("delay must be nonnegative and timeout positive")
    collect(args.output, args.delay_seconds, args.timeout_seconds, args.resume, args.transport)


if __name__ == "__main__":
    main()
