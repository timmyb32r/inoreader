#!/usr/bin/env python3
"""Conservative compile-only development gate.

This intentionally performs no formatting, linting, tests, bundling, generation,
Docker work, or release builds. The repository is small enough that Rust changes
currently check the workspace; frontend changes use TypeScript's no-emit mode.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def command_exists(path: Path) -> bool:
    return path.exists()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("paths", nargs="*")
    args = parser.parse_args()

    commands: list[list[str]] = []
    if command_exists(ROOT / "Cargo.toml"):
        commands.append(["cargo", "check", "--workspace", "--all-targets"])
    if command_exists(ROOT / "web" / "package.json"):
        commands.append(["npm", "run", "typecheck", "--prefix", "web"])

    if args.dry_run:
        for command in commands:
            print(" ".join(command))
        return 0

    started = time.monotonic()
    reports = []
    for command in commands:
        command_started = time.monotonic()
        result = subprocess.run(command, cwd=ROOT, check=False)
        reports.append(
            {
                "command": command,
                "duration_seconds": round(time.monotonic() - command_started, 3),
                "exit_code": result.returncode,
            }
        )
        if result.returncode:
            break
    report = {
        "duration_seconds": round(time.monotonic() - started, 3),
        "commands": reports,
    }
    target = ROOT / "target"
    target.mkdir(exist_ok=True)
    (target / "affected-tests-timings.json").write_text(
        json.dumps(report, indent=2) + "\n", encoding="utf-8"
    )
    return reports[-1]["exit_code"] if reports else 0


if __name__ == "__main__":
    sys.exit(main())

