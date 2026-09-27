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
        "config.example.yaml",
        "docs/operations.md",
        "docs/seed-manifest.schema.json",
        "source-inventory/coverage-matrix.json",
        "web/dist/index.html",
        "web/package-lock.json",
        "docs/deepseek-operations.md",
        "prompts/reading-data-news/transport.md",
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
    for expected in ("POSTGRES_PASSWORD_FILE: /run/secrets/postgres-password", "target: postgres-password", "postgres-data:/var/lib/postgresql/data"):
        if expected not in compose:
            failures.append(f"Compose lacks the PostgreSQL persistence contract {expected!r}")
    for expected in (
        "AI_ENCRYPTION_KEY_FILE: /run/secrets/ai-encryption-key",
        "target: ai-encryption-key",
        "file: ./secrets/ai-encryption-key",
        "./prompts:/etc/inoreader/prompts:ro",
    ):
        if expected not in compose:
            failures.append(f"Compose lacks the DeepSeek deployment contract {expected!r}")
    if "COPY prompts/reading-data-news/transport.md prompts/reading-data-news/transport.md" not in dockerfile:
        failures.append("Rust image build must include the embedded DeepSeek transport instruction")
    if "secrets" not in (ROOT / ".dockerignore").read_text(encoding="utf-8").splitlines():
        failures.append("Docker context must exclude the secrets directory")
    if "docker.sock" in compose:
        failures.append("production Compose must not mount docker.sock")
    if 'ports: ["9222"]' in compose or '9222:9222' in compose:
        failures.append("Chromium CDP must not be published to the host")
    if not re.search(r"browser-control:\s*\n\s+name:.*\n\s+internal:\s*true", compose):
        failures.append("browser-control network must be internal")
    app_match = re.search(r"^  app:\n(.*?)(?=^  \S|^\S|\Z)", compose, re.M | re.S)
    chromium_match = re.search(r"^  chromium:\n(.*?)(?=^  \S|^\S|\Z)", compose, re.M | re.S)
    if app_match is None:
        failures.append("production Compose has no app service")
    else:
        app_networks = app_match.group(1).rsplit("    networks:\n", 1)[-1]
        if "- public-egress" not in app_networks or "- browser-control" not in app_networks:
            failures.append("app must join both public-egress and browser-control")
    if chromium_match is None:
        failures.append("production Compose has no Chromium service")
    else:
        chromium_service = chromium_match.group(1)
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

    postgres = re.search(r"^  postgres:\n(.*?)(?=^  \S|^\S|\Z)", compose, re.M | re.S)
    if postgres is None or "@sha256:" not in postgres.group(1):
        failures.append("PostgreSQL image must be digest-pinned")
    release_gate = (ROOT / "justfile").read_text(encoding="utf-8")
    workflow = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
    for name, content in (("release gate", release_gate), ("release CI", workflow)):
        if "cargo test --workspace --all-targets --all-features" not in content:
            failures.append(f"{name} must run PostgreSQL persistence and backup/restore acceptance")

    if failures:
        print("\n".join(failures), file=sys.stderr)
        return 1
    print("operational assets: OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
