#!/usr/bin/env python3
"""Build a standalone, network-free author-review page from a blind manifest.

The manifest is a public-to-the-reviewer DTO, not an experiment record. Its exact
keys are schema_version, evaluation_id, and items; each item has id/title/url/text.
IDs must be opaque and unique; callers must not encode arm/model/reference labels
in them. No source/reference files are loaded. The complete supplied text and URL
are retained, and unknown fields are rejected rather than silently discarded.

The browser renders a small safe Markdown subset using DOM nodes, retains the
original text for inspection/export, and namespaces drafts by evaluation_id. A
saved draft must match the entire manifest before it can be reused. This is an
offline research artifact, not a production application HTTP integration.
"""

import argparse
import html
import json
from pathlib import Path
import re
import secrets
from urllib.parse import urlsplit


ASSET_ROOT = Path(__file__).with_name("blind_review")


def _exact_keys(value, keys, name):
    if not isinstance(value, dict) or set(value) != set(keys):
        raise ValueError(f"{name} must have exactly these fields: {', '.join(keys)}")


def _text(value, name):
    if not isinstance(value, str) or not value.strip():
        raise ValueError(f"{name} must be a nonempty string")
    try:
        value.encode("utf-8")
    except UnicodeEncodeError as error:
        raise ValueError(f"{name} must contain valid Unicode") from error
    return value


def _identifier(value, name):
    _text(value, name)
    if not re.fullmatch(r"[A-Za-z0-9_-]+", value):
        raise ValueError(f"{name} must be an opaque ASCII identifier using letters, digits, _ or -")


def validate_manifest(manifest, expected_count=20):
    """Validate the full DTO before rendering; no coercion/defaulting/drop occurs.

    expected_count is the explicit experimental design, not a truncation limit.
    Source URLs must be absolute HTTP(S), without userinfo, backslashes, or
    whitespace/control characters. They are not fetched or rewritten.
    """
    if type(expected_count) is not int or expected_count < 1:
        raise ValueError("expected_count must be a positive integer")
    _exact_keys(manifest, ("schema_version", "evaluation_id", "items"), "manifest")
    if type(manifest["schema_version"]) is not int or manifest["schema_version"] != 1:
        raise ValueError("schema_version must be integer 1")
    _identifier(manifest["evaluation_id"], "evaluation_id")
    items = manifest["items"]
    if not isinstance(items, list) or len(items) != expected_count:
        raise ValueError(f"items must contain exactly {expected_count} complete items")
    ids = set()
    for index, item in enumerate(items):
        context = f"items[{index}]"
        _exact_keys(item, ("id", "title", "url", "text"), context)
        _identifier(item["id"], f"{context}.id")
        if item["id"] in ids:
            raise ValueError(f"{context}.id duplicates an earlier item")
        ids.add(item["id"])
        _text(item["title"], f"{context}.title")
        _text(item["text"], f"{context}.text")
        url = _text(item["url"], f"{context}.url")
        if "\\" in url or any(character.isspace() or ord(character) < 32 or ord(character) == 127 for character in url):
            raise ValueError(f"{context}.url contains whitespace, a control character, or backslash")
        try:
            parsed = urlsplit(url)
            valid = (parsed.scheme.lower() in ("https", "http") and bool(parsed.hostname)
                     and parsed.username is None and parsed.password is None)
            parsed.port  # Validate a supplied port; retain its exact representation.
        except ValueError as error:
            raise ValueError(f"{context}.url is not a valid absolute HTTP(S) URL") from error
        if not valid:
            raise ValueError(f"{context}.url must be absolute HTTP(S) without credentials")
    return manifest


def _embedded_json(manifest):
    # JSON escapes prevent the HTML parser from recognizing </script>, including
    # when that sequence occurs in untrusted prose. Parsed strings remain exact.
    value = json.dumps(manifest, ensure_ascii=False, separators=(",", ":"))
    for character, escaped in (("&", "\\u0026"), ("<", "\\u003c"), (">", "\\u003e"),
                               ("\u2028", "\\u2028"), ("\u2029", "\\u2029")):
        value = value.replace(character, escaped)
    return value


def render_html(manifest, expected_count=20):
    validate_manifest(manifest, expected_count)
    # This random nonce is a CSP security boundary, not a provenance hash.
    nonce = secrets.token_urlsafe(24)
    replacements = {
        "NONCE": html.escape(nonce, quote=True),
        "STYLE": (ASSET_ROOT / "style.css").read_text(encoding="utf-8"),
        "SCRIPT": (ASSET_ROOT / "review.js").read_text(encoding="utf-8"),
        "DATA": _embedded_json(manifest),
    }
    template = (ASSET_ROOT / "template.html").read_text(encoding="utf-8")
    # One substitution pass: user text that resembles a template marker is data.
    return re.sub(r"@@(NONCE|STYLE|SCRIPT|DATA)@@", lambda match: replacements[match[1]], template)


def write_review(manifest_path, output_path, expected_count=20):
    manifest = json.loads(Path(manifest_path).read_text(encoding="utf-8"))
    rendered = render_html(manifest, expected_count)
    output = Path(output_path)
    output.parent.mkdir(parents=True, exist_ok=True)
    # Preserve an existing review artifact and its evaluation identity.
    with output.open("x", encoding="utf-8", newline="") as stream:
        stream.write(rendered)
    return output


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--expected-count", type=int, default=20)
    args = parser.parse_args()
    try:
        output = write_review(args.manifest, args.output, args.expected_count)
    except (ValueError, OSError) as error:
        parser.exit(2, f"Cannot create blind review: {error}\n")
    print(f"Created {output}")


if __name__ == "__main__":
    main()
