#!/usr/bin/env python3
"""Create a reviewable, account-scoped seed draft without applying it."""

from __future__ import annotations

import argparse
import json
import sys
import uuid
from pathlib import Path


def parse_uuid(label: str, value: str) -> str:
    try:
        return str(uuid.UUID(value))
    except ValueError as error:
        raise SystemExit(f"{label} must be a UUID: {error}") from error


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--inventory", type=Path, required=True)
    parser.add_argument("--account-id", required=True)
    parser.add_argument("--workspace-id", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    account_id = parse_uuid("--account-id", args.account_id)
    workspace_id = parse_uuid("--workspace-id", args.workspace_id)
    inventory = json.loads(args.inventory.read_text(encoding="utf-8"))
    sources = inventory.get("sources")
    if not isinstance(sources, list) or len(sources) != inventory.get("expected_source_count"):
        raise SystemExit("inventory source count is incomplete")

    items = []
    for source in sources:
        source_id = source.get("id")
        configuration = source.get("configuration")
        if not isinstance(source_id, str) or not source_id or not isinstance(configuration, dict):
            raise SystemExit("inventory contains a malformed source")
        supported = True
        items.append(
            {
                "source_inventory_id": source_id,
                "idempotency_key": f"personal_feed:{source_id}",
                "selected": supported,
                "status": "reviewed" if supported else "unresolved",
                "configuration": configuration,
                "note": (
                    "Imported losslessly into the shared feed, validated HTML recipe, or built-in adapter pipeline; runtime execution remains pending"
                ),
            }
        )

    manifest = {
        "schema_version": 1,
        "owner_account_id": account_id,
        "workspace_id": workspace_id,
        "items": items,
    }
    if args.output.exists():
        raise SystemExit(f"refusing to overwrite existing output: {args.output}")
    args.output.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    selected = sum(item["selected"] for item in items)
    print(f"wrote {len(items)} items ({selected} selectable, {len(items) - selected} unresolved) to {args.output}; no data was applied")
    return 0


if __name__ == "__main__":
    sys.exit(main())
