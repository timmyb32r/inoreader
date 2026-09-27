import contextlib
import importlib.util
import io
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "operational_assets", ROOT / "tools/check_operational_assets.py"
)
CHECKER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECKER)


class OperationalAssetsTest(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        files = (
            "Dockerfile", "compose.yaml", "compose.local.yaml", ".dockerignore",
            "config.example.yaml", "docs/operations.md", "docs/seed-manifest.schema.json",
            "docs/deepseek-operations.md", "source-inventory/coverage-matrix.json",
            "web/dist/index.html", "web/package-lock.json", "tools/test_ydb_backup_restore.sh",
            "tools/fixtures/ydb_backup_restore.sql", "tools/ydb_backup.sh", "tools/ydb_restore.sh",
            "prompts/reading-data-news/transport.md", "justfile", ".github/workflows/ci.yml",
        )
        for relative in files:
            target = self.root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes((ROOT / relative).read_bytes())

    def check(self):
        with patch.object(CHECKER, "ROOT", self.root), contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            return CHECKER.main()

    def replace(self, file, before, after):
        path = self.root / file
        content = path.read_text()
        self.assertIn(before, content)
        path.write_text(content.replace(before, after))

    def test_app_networks_are_selected_with_postgres_between_app_and_chromium(self):
        self.assertEqual(self.check(), 0)
        self.replace("compose.yaml", "      - public-egress\n", "")
        self.assertEqual(self.check(), 1)

    def test_chromium_cannot_gain_public_egress(self):
        self.replace("compose.yaml", "      - browser-control\nsecrets:", "      - browser-control\n      - public-egress\nsecrets:")
        self.assertEqual(self.check(), 1)

    def test_ai_secret_mount_prompt_and_build_context_are_required(self):
        changes = (
            ("compose.yaml", "target: ai-encryption-key", "target: wrong-key"),
            ("compose.yaml", "./prompts:/etc/inoreader/prompts:ro", "./prompts:/etc/inoreader/prompts:rw"),
            ("Dockerfile", "COPY prompts/reading-data-news/transport.md prompts/reading-data-news/transport.md", ""),
            (".dockerignore", "secrets\n", ""),
        )
        for file, before, after in changes:
            with self.subTest(file=file, before=before):
                original = (self.root / file).read_text()
                self.replace(file, before, after)
                self.assertEqual(self.check(), 1)
                (self.root / file).write_text(original)


if __name__ == "__main__":
    unittest.main()
