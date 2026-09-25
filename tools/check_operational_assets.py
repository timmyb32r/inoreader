#!/usr/bin/env python3
"""Cheap, network-free consistency checks for deployment artifacts."""

from __future__ import annotations

import sys
from pathlib import Path
import re


ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    failures: list[str] = []
    required = (
        "Dockerfile",
        "compose.yaml",
        "compose.local.yaml",
        "config.example.yaml",
        "docs/operations.md",
        "docs/seed-manifest.schema.json",
        "source-inventory/coverage-matrix.json",
        "web/dist/index.html",
        "web/package-lock.json",
        "tools/test_ydb_backup_restore.sh",
        "tools/fixtures/ydb_backup_restore.sql",
    )
    for relative in required:
        if not (ROOT / relative).is_file():
            failures.append(f"missing required operational asset: {relative}")

    dockerfile = (ROOT / "Dockerfile").read_text(encoding="utf-8")
    for expected in ("npm ci", "npm run build", "cargo build --locked --release -p inoreader"):
        if expected not in dockerfile:
            failures.append(f"Dockerfile does not contain {expected!r}")
    compose = (ROOT / "compose.yaml").read_text(encoding="utf-8")
    if 'command: ["/usr/local/bin/inoreader"' in compose:
        failures.append("Compose command must not repeat the Dockerfile ENTRYPOINT")
    if "target: ydb-key.json" not in compose:
        failures.append("Compose must mount the YDB secret at the configured credential path")
    if "docker.sock" in compose:
        failures.append("production Compose must not mount docker.sock")
    if 'ports: ["9222"]' in compose or '9222:9222' in compose:
        failures.append("Chromium CDP must not be published to the host")
    if "YDB_USE_IN_MEMORY_PDISKS" in compose:
        failures.append("production Compose must not contain local YDB")
    if not re.search(r"browser-control:\s*\n\s+name:.*\n\s+internal:\s*true", compose):
        failures.append("browser-control network must be internal")
    app_block, separator, chromium_block = compose.rpartition("\n  chromium:\n")
    if not separator:
        failures.append("production Compose has no Chromium service")
    else:
        app_networks = app_block.rsplit("    networks:\n", 1)[-1]
        if "- public-egress" not in app_networks or "- browser-control" not in app_networks:
            failures.append("app must join both public-egress and browser-control")
        chromium_service = chromium_block.partition("secrets:\n")[0]
        chromium_networks = chromium_service.rsplit("    networks:\n", 1)[-1]
        if "- browser-control" not in chromium_networks or "- public-egress" in chromium_networks:
            failures.append("Chromium must join only browser-control")
        for hardening in ('cap_drop: ["ALL"]', "read_only: true", 'security_opt: ["no-new-privileges:true"]'):
            if hardening not in chromium_service:
                failures.append(f"Chromium service lacks {hardening}")

    final_stage = dockerfile.rsplit("FROM ", 1)[-1]
    if "COPY --from=rust /tmp/inoreader /usr/local/bin/inoreader" not in final_stage:
        failures.append("final image does not copy the one inoreader app binary")
    if any(value in final_stage for value in ("node_modules", "/src/web", "rustc", "cargo ")):
        failures.append("final image unexpectedly contains build-time UI/Rust inputs")
    if 'USER 10001:10001' not in final_stage or 'ENTRYPOINT ["/usr/local/bin/inoreader"]' not in final_stage:
        failures.append("final image must run the one app binary as the unprivileged user")

    backup = (ROOT / "tools/ydb_backup.sh").read_text(encoding="utf-8")
    restore = (ROOT / "tools/ydb_restore.sh").read_text(encoding="utf-8")
    for expected in ("tools dump", "refusing to overwrite existing backup path", "backup_directory="):
        if expected not in backup:
            failures.append(f"backup wrapper lacks {expected!r}")
    for expected in ("tools restore", "restore target must differ", "requires an interactive terminal", "backup manifest identity"):
        if expected not in restore:
            failures.append(f"restore wrapper lacks {expected!r}")

    acceptance = (ROOT / "tools/test_ydb_backup_restore.sh").read_text(encoding="utf-8")
    fixture = (ROOT / "tools/fixtures/ydb_backup_restore.sql").read_text(encoding="utf-8")
    local_compose = (ROOT / "compose.local.yaml").read_text(encoding="utf-8")
    release_gate = (ROOT / "justfile").read_text(encoding="utf-8")
    workflow = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
    for expected in ("@sha256:", "ydb_backup.sh", "ydb_restore.sh", "resumable-job", "rebuilt-counter"):
        if expected not in acceptance:
            failures.append(f"A24 Docker acceptance lacks {expected!r}")
    for table in ("articles", "rules", "content_manifests", "staged_content_chunks", "library_origins", "ingest_jobs"):
        if not re.search(rf"CREATE TABLE(?: IF NOT EXISTS)? {table}\b", fixture):
            failures.append(f"A24 fixture lacks {table!r}")
    if "@sha256:" not in local_compose or ":latest" in local_compose:
        failures.append("local YDB Compose image must be digest-pinned")
    if "./tools/test_ydb_backup_restore.sh" not in release_gate:
        failures.append("release gate does not run A24 Docker acceptance")
    if "./tools/test_ydb_backup_restore.sh" not in workflow:
        failures.append("release CI does not run A24 Docker acceptance")

    if failures:
        print("\n".join(failures), file=sys.stderr)
        return 1
    print("operational assets: OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
