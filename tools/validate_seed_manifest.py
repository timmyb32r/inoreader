#!/usr/bin/env python3
"""Validate source inventory or an account-scoped seed review artifact."""

from __future__ import annotations

import argparse
import json
import sys
import uuid
from pathlib import Path


SECRET_FRAGMENTS = ("password", "secret", "token", "credential", "private_key")


def fail(message: str) -> None:
    raise ValueError(message)


def validate_inventory(document: object) -> None:
    if not isinstance(document, dict):
        fail("inventory must be an object")
    expected = document.get("expected_source_count")
    sources = document.get("sources")
    if not isinstance(expected, int) or expected <= 0 or not isinstance(sources, list):
        fail("inventory count and sources are required")
    if len(sources) != expected:
        fail(f"inventory has {len(sources)} sources, expected {expected}")
    ids = [item.get("id") for item in sources if isinstance(item, dict)]
    if len(ids) != expected or any(not isinstance(value, str) or not value for value in ids):
        fail("each source needs a non-empty id")
    if len(set(ids)) != len(ids):
        fail("source ids must be unique")


def validate_manifest(document: object) -> None:
    if not isinstance(document, dict) or set(document) != {
        "schema_version", "owner_account_id", "workspace_id", "items"
    }:
        fail("manifest has missing or unknown top-level fields")
    if document["schema_version"] != 1:
        fail("unsupported schema_version")
    for field in ("owner_account_id", "workspace_id"):
        try:
            uuid.UUID(document[field])
        except (ValueError, TypeError, AttributeError) as error:
            fail(f"{field} must be a UUID: {error}")
    items = document["items"]
    if not isinstance(items, list) or not items:
        fail("manifest must contain at least one item")

    source_ids: set[str] = set()
    keys: set[str] = set()
    for index, item in enumerate(items):
        if not isinstance(item, dict):
            fail(f"item {index} must be an object")
        required = {"source_inventory_id", "idempotency_key", "selected", "status", "configuration"}
        allowed = required | {"note"}
        if not required <= set(item) or not set(item) <= allowed:
            fail(f"item {index} has missing or unknown fields")
        source_id = item["source_inventory_id"]
        key = item["idempotency_key"]
        if not isinstance(source_id, str) or not source_id:
            fail(f"item {index} source_inventory_id is empty")
        if key != f"personal_feed:{source_id}":
            fail(f"item {index} idempotency_key does not match source id")
        if source_id in source_ids or key in keys:
            fail(f"item {index} duplicates a source or idempotency key")
        source_ids.add(source_id)
        keys.add(key)
        if not isinstance(item["selected"], bool):
            fail(f"item {index} selected must be boolean")
        if item["status"] not in {"reviewed", "unresolved", "disabled"}:
            fail(f"item {index} has invalid status")
        if item["selected"] and item["status"] != "reviewed":
            fail(f"item {index} selects an item that is not reviewed")
        configuration = item["configuration"]
        if not isinstance(configuration, dict) or not configuration:
            fail(f"item {index} configuration is empty")
        for name in configuration:
            lowered = name.lower()
            if any(fragment in lowered for fragment in SECRET_FRAGMENTS):
                fail(f"item {index} contains forbidden secret-like field {name!r}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--inventory-only", action="store_true")
    parser.add_argument("path", type=Path)
    args = parser.parse_args()
    try:
        document = json.loads(args.path.read_text(encoding="utf-8"))
        if args.inventory_only:
            validate_inventory(document)
        else:
            validate_manifest(document)
    except (OSError, json.JSONDecodeError, ValueError) as error:
        print(f"seed validation failed: {error}", file=sys.stderr)
        return 1
    print("seed inventory is valid" if args.inventory_only else "seed manifest is valid; no data was applied")
    return 0


if __name__ == "__main__":
    sys.exit(main())
