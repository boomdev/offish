"""Checks for workspace path containment used by the Tauri CodeMirror host.

Mirrors resolve_under in spikes/tauri-codemirror/src/main.rs. A URI path may
start with '/', so a leading slash is stripped; parent segments and resolved
paths outside the root are refused.
"""
from pathlib import Path
from tempfile import TemporaryDirectory
import os
import unittest


def resolve_under(root: Path, relative: str) -> Path:
    trimmed = relative.lstrip("/")
    if not trimmed:
        raise ValueError("empty")
    requested = Path(trimmed)
    if requested.is_absolute() or any(part == ".." for part in requested.parts):
        raise PermissionError("outside workspace")
    root = root.resolve()
    candidate = (root / requested).resolve()
    if not candidate.is_relative_to(root):
        raise PermissionError("outside workspace")
    if not candidate.is_file():
        raise FileNotFoundError(trimmed)
    return candidate


class WorkspaceAssetTests(unittest.TestCase):
    def setUp(self):
        self.directory = self.enterContext(TemporaryDirectory())
        self.root = Path(self.directory)
        (self.root / "assets").mkdir()
        (self.root / "assets" / "timeline.svg").write_text("<svg/>")
        outside = Path(self.directory).parent / f"offish-outside-{os.getpid()}.txt"
        outside.write_text("escape")
        self.addCleanup(lambda: outside.unlink(missing_ok=True))
        self.outside = outside
        # Symlink that would escape if followed without a root check.
        (self.root / "escape.svg").symlink_to(self.outside)

    def test_serves_relative_asset(self):
        for request in ("assets/timeline.svg", "/assets/timeline.svg"):
            path = resolve_under(self.root, request)
            self.assertEqual(path.read_text(), "<svg/>")

    def test_rejects_parent_escape(self):
        with self.assertRaises(PermissionError):
            resolve_under(self.root, "../" + self.outside.name)

    def test_rejects_symlink_escape(self):
        with self.assertRaises(PermissionError):
            resolve_under(self.root, "escape.svg")

    def test_missing_file(self):
        with self.assertRaises(FileNotFoundError):
            resolve_under(self.root, "assets/missing.svg")


if __name__ == "__main__":
    unittest.main(verbosity=2)
