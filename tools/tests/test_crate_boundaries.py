import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("boundaries", Path(__file__).resolve().parents[2] / "scripts/check_crate_boundaries.py")
boundaries = importlib.util.module_from_spec(spec)
spec.loader.exec_module(boundaries)


class CrateBoundaries(unittest.TestCase):
    def test_renamed_target_build_and_workspace_edges_use_package_identity(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            crate = root / "crates" / "reader-core"
            crate.mkdir(parents=True)
            (root / "Cargo.toml").write_text('[workspace.dependencies]\nprivate_ai={package="reader-ai",path="crates/reader-ai"}\n')
            manifest = crate / "Cargo.toml"
            manifest.write_text('''[dependencies]
private_ai={workspace=true}
renamed={package="reader-server",path="../reader-server"}
[target.'cfg(unix)'.build-dependencies]
other={package="reader-ingest",path="../reader-ingest"}
[dev-dependencies]
reader-storage-postgres={path="../reader-storage-postgres"}
''')
            self.assertEqual(boundaries.internal_dependencies(manifest), {"reader-ai", "reader-server", "reader-ingest"})

    def test_cycle_detection(self):
        self.assertIsNotNone(boundaries.dependency_cycle({"a": {"b"}, "b": {"a"}}))
        self.assertIsNone(boundaries.dependency_cycle({"a": {"b"}, "b": set()}))
