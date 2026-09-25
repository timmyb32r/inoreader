import os
import pathlib
import subprocess
import tempfile
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]


class YdbBackupToolsTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.base = pathlib.Path(self.temp.name)
        self.bin = self.base / "bin"
        self.bin.mkdir()
        fake = self.bin / "ydb"
        fake.write_text(
            "#!/usr/bin/env bash\n"
            "set -euo pipefail\n"
            "printf '%s\\n' \"$*\" >>\"$YDB_FAKE_LOG\"\n"
            "if [[ \" $* \" == *' tools dump '* ]]; then\n"
            "  args=(\"$@\"); for ((i=0;i<${#args[@]};i++)); do\n"
            "    if [[ ${args[$i]} == --output ]]; then mkdir \"${args[$((i+1))]}\"; fi\n"
            "  done\n"
            "fi\n"
        )
        fake.chmod(0o755)
        self.log = self.base / "calls"
        self.env = os.environ | {
            "PATH": f"{self.bin}:{os.environ['PATH']}",
            "YDB_FAKE_LOG": str(self.log),
        }

    def tearDown(self):
        self.temp.cleanup()

    def run_tool(self, name, *args, check=True):
        return subprocess.run(
            [str(ROOT / "tools" / name), *args],
            env=self.env,
            text=True,
            capture_output=True,
            check=check,
        )

    def test_backup_manifest_binds_endpoint_database_and_directory(self):
        backup = self.base / "backup"
        self.run_tool(
            "ydb_backup.sh",
            "--endpoint", "grpc://source:2136",
            "--database", "/local",
            "--output", str(backup),
            "--no-discovery",
        )
        manifest = pathlib.Path(f"{backup}.inoreader-manifest").read_text()
        self.assertIn("schema_version=2\n", manifest)
        self.assertIn("source_endpoint=grpc://source:2136\n", manifest)
        self.assertIn("source_database=/local\n", manifest)
        self.assertIn("backup_directory=backup\n", manifest)
        self.assertIn("--no-discovery tools dump --path /local", self.log.read_text())

    def test_restore_accepts_exact_noninteractive_target_identity(self):
        backup = self.base / "backup"
        backup.mkdir()
        pathlib.Path(f"{backup}.inoreader-manifest").write_text(
            "schema_version=2\ncreated_at_utc=2026-09-25T00:00:00Z\n"
            "source_database=/local\nsource_endpoint=grpc://source:2136\n"
            "scope=/\nbackup_directory=backup\n"
        )
        self.run_tool(
            "ydb_restore.sh",
            "--endpoint", "grpc://target:2136",
            "--database", "/local",
            "--input", str(backup),
            "--confirm-target", "grpc://target:2136|/local",
            "--no-discovery",
        )
        self.assertIn("--no-discovery tools restore --path /local", self.log.read_text())

    def test_restore_rejects_source_and_mismatched_confirmation(self):
        backup = self.base / "backup"
        backup.mkdir()
        pathlib.Path(f"{backup}.inoreader-manifest").write_text(
            "schema_version=2\nsource_database=/local\n"
            "source_endpoint=grpc://source:2136\nscope=/\nbackup_directory=backup\n"
        )
        same = self.run_tool(
            "ydb_restore.sh",
            "--endpoint", "grpc://source:2136",
            "--database", "/local",
            "--input", str(backup),
            "--confirm-target", "grpc://source:2136|/local",
            check=False,
        )
        self.assertNotEqual(same.returncode, 0)
        self.assertIn("restore target must differ", same.stderr)
        mismatch = self.run_tool(
            "ydb_restore.sh",
            "--endpoint", "grpc://target:2136",
            "--database", "/local",
            "--input", str(backup),
            "--confirm-target", "wrong",
            check=False,
        )
        self.assertNotEqual(mismatch.returncode, 0)
        self.assertIn("confirmation did not match", mismatch.stderr)


if __name__ == "__main__":
    unittest.main()
