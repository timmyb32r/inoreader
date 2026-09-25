from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
PREPARE = ROOT / "tools" / "prepare_seed_manifest.py"
VALIDATE = ROOT / "tools" / "validate_seed_manifest.py"
INVENTORY = ROOT / "source-inventory" / "inventory.json"


class SeedToolTests(unittest.TestCase):
    def test_inventory_and_generated_manifest_are_valid(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / "seed.json"
            subprocess.run(
                [
                    sys.executable,
                    PREPARE,
                    "--inventory",
                    INVENTORY,
                    "--account-id",
                    "11111111-1111-4111-8111-111111111111",
                    "--workspace-id",
                    "22222222-2222-4222-8222-222222222222",
                    "--output",
                    manifest,
                ],
                check=True,
            )
            subprocess.run([sys.executable, VALIDATE, manifest], check=True)

    def test_unreviewed_selected_item_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / "seed.json"
            manifest.write_text(
                json.dumps(
                    {
                        "schema_version": 1,
                        "owner_account_id": "11111111-1111-4111-8111-111111111111",
                        "workspace_id": "22222222-2222-4222-8222-222222222222",
                        "items": [
                            {
                                "source_inventory_id": "ocr-row",
                                "idempotency_key": "personal_feed:ocr-row",
                                "selected": True,
                                "status": "unresolved",
                                "configuration": {"url": "https://example.com"},
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )
            result = subprocess.run([sys.executable, VALIDATE, manifest], check=False)
            self.assertNotEqual(result.returncode, 0)

    def test_existing_output_is_not_overwritten(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / "seed.json"
            manifest.write_text("sentinel", encoding="utf-8")
            result = subprocess.run(
                [
                    sys.executable,
                    PREPARE,
                    "--inventory",
                    INVENTORY,
                    "--account-id",
                    "11111111-1111-4111-8111-111111111111",
                    "--workspace-id",
                    "22222222-2222-4222-8222-222222222222",
                    "--output",
                    manifest,
                ],
                check=False,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(manifest.read_text(encoding="utf-8"), "sentinel")


if __name__ == "__main__":
    unittest.main()
