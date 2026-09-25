#!/usr/bin/env python3
"""Validate the complete internal Cargo dependency graph and layer policy."""

from __future__ import annotations

import sys
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CRATES = ROOT / "crates"

# Every internal dependency must be listed. A new edge fails closed until its
# architectural ownership is reviewed.
ALLOWED: dict[str, set[str]] = {
    "reader-core": set(),
    "reader-application": {"reader-core"},
    "reader-collectors": {"reader-core"},
    "reader-web-runtime": set(),
    "reader-ingest": {"reader-core", "reader-collectors", "reader-web-runtime"},
    "reader-server-contracts": {"reader-core"},
    "reader-server": {"reader-core", "reader-application", "reader-server-contracts"},
    "reader-server-ui": set(),
    "reader-storage-ydb": {"reader-core", "reader-application", "reader-ingest"},
    "inoreader": {
        "reader-core", "reader-application", "reader-collectors", "reader-ingest",
        "reader-server", "reader-server-contracts", "reader-server-ui",
        "reader-storage-ydb", "reader-web-runtime",
    },
}


def internal_dependencies(manifest: Path) -> set[str]:
    document = tomllib.loads(manifest.read_text(encoding="utf-8"))
    result: set[str] = set()
    for section in ("dependencies", "build-dependencies"):
        for name, value in document.get(section, {}).items():
            if name in ALLOWED and isinstance(value, dict) and "path" in value:
                result.add(name)
    return result


def dependency_cycle(graph: dict[str, set[str]]) -> list[str] | None:
    active: list[str] = []
    done: set[str] = set()

    def visit(node: str) -> list[str] | None:
        if node in active:
            start = active.index(node)
            return active[start:] + [node]
        if node in done:
            return None
        active.append(node)
        for child in sorted(graph[node]):
            found = visit(child)
            if found:
                return found
        active.pop()
        done.add(node)
        return None

    for node in sorted(graph):
        found = visit(node)
        if found:
            return found
    return None


def main() -> int:
    failures: list[str] = []
    manifests = {path.parent.name: path for path in CRATES.glob("*/Cargo.toml")}
    failures.extend(f"missing crate manifest: {name}" for name in sorted(set(ALLOWED) - set(manifests)))
    failures.extend(f"crate has no reviewed layer policy: {name}" for name in sorted(set(manifests) - set(ALLOWED)))

    graph: dict[str, set[str]] = {}
    for crate, manifest in manifests.items():
        if crate not in ALLOWED:
            continue
        actual = internal_dependencies(manifest)
        graph[crate] = actual
        failures.extend(f"{crate} must not depend on {name}" for name in sorted(actual - ALLOWED[crate]))

    found_cycle = dependency_cycle(graph) if set(graph) == set(ALLOWED) else None
    if found_cycle:
        failures.append("internal crate dependency cycle: " + " -> ".join(found_cycle))
    if failures:
        print("\n".join(failures), file=sys.stderr)
        return 1
    print(f"crate dependency boundaries: OK ({len(graph)} crates, acyclic graph)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
