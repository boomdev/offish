"""Host-side containment model for a mediated Codex session (P0-06 / G0-4).

The signed-in Omarchy trial proved transport. This spike records the host
policy Offish must enforce before a filesystem-capable adapter is allowed:

1. The agent process sees only an empty or disposable working directory.
2. Context arrives only as host-supplied immutable snippets.
3. The only write path is a validated proposal staged by Offish.
4. Cancellation drops late output and clears granted references.
5. Path escape attempts through proposals or reads are refused.

It does not launch Codex. Pair it with an OS sandbox (bubblewrap/Landlock)
around the app-server process before enabling workspace tools.
"""
from dataclasses import dataclass
from pathlib import Path
from tempfile import TemporaryDirectory
import json
import unittest

from check_proposals import Proposal, Session, accept, digest


def sandbox_env(root: Path) -> dict:
    """Environment for a Codex child that must not inherit workspace paths."""
    return {
        "HOME": str(root / "home"),
        "CODEX_SQLITE_HOME": str(root / "state"),
        "TMPDIR": str(root / "tmp"),
        "OFFISH_WORKSPACE": "",  # never point the child at the real workspace
    }


@dataclass
class MediatedHost:
    """Offish-owned tool surface. The model never receives raw filesystem tools."""

    workspace: Path
    allowed: frozenset
    session: Session | None = None

    def open_session(self, document: str, start: int, end: int, references: dict):
        for name in references:
            if name not in self.allowed:
                raise PermissionError(f"reference not authorised: {name}")
        source = (self.workspace / document).read_bytes()
        self.session = Session(references, source, start, end)
        return self.session

    def read_tool(self, name: str) -> bytes:
        if self.session is None:
            raise PermissionError("no session")
        if name not in self.allowed:
            raise PermissionError("reference not authorised")
        return self.session.read(name)

    def propose_tool(self, replacement: bytes) -> Proposal:
        if self.session is None:
            raise PermissionError("no session")
        return self.session.propose(replacement)

    def cancel(self):
        if self.session is not None:
            self.session.cancel()


class ContainmentTests(unittest.TestCase):
    def setUp(self):
        directory = self.enterContext(TemporaryDirectory())
        self.root = Path(directory)
        self.workspace = self.root / "workspace"
        self.workspace.mkdir()
        (self.workspace / "brief.md").write_text(
            "Keep the first release on the current schedule.\n"
        )
        (self.workspace / "assumptions.md").write_text("The pilot date moves to 14 December.\n")
        (self.workspace / "private.md").write_text("secret\n")
        self.sandbox = self.root / "sandbox"
        for name in ("home", "state", "tmp"):
            (self.sandbox / name).mkdir(parents=True)

    def test_child_env_has_no_workspace_path(self):
        env = sandbox_env(self.sandbox)
        self.assertEqual(env["OFFISH_WORKSPACE"], "")
        self.assertFalse(Path(env["HOME"]).is_relative_to(self.workspace))
        self.assertTrue((self.sandbox / "home").exists())

    def test_only_authorised_references_are_readable(self):
        host = MediatedHost(
            self.workspace,
            frozenset({"assumptions"}),
        )
        source = (self.workspace / "brief.md").read_bytes()
        start = source.index(b"Keep the first release")
        end = start + len(b"Keep the first release on the current schedule.")
        host.open_session(
            "brief.md",
            start,
            end,
            {"assumptions": (self.workspace / "assumptions.md").read_bytes()},
        )
        self.assertIn(b"14 December", host.read_tool("assumptions"))
        with self.assertRaises(PermissionError):
            host.read_tool("private")
        with self.assertRaises(PermissionError):
            host.open_session(
                "brief.md",
                start,
                end,
                {"private": (self.workspace / "private.md").read_bytes()},
            )

    def test_writes_only_through_validated_proposal(self):
        host = MediatedHost(self.workspace, frozenset({"assumptions"}))
        source = (self.workspace / "brief.md").read_bytes()
        start = source.index(b"Keep the first release")
        end = start + len(b"Keep the first release on the current schedule.")
        host.open_session(
            "brief.md",
            start,
            end,
            {"assumptions": (self.workspace / "assumptions.md").read_bytes()},
        )
        proposal = host.propose_tool(b"Move the pilot launch to 14 December.")
        journal = self.root / "journal.json"
        target = self.workspace / "brief.md"
        accept(target, journal, proposal)
        self.assertIn(b"14 December", target.read_bytes())
        self.assertEqual(digest(target.read_bytes()), json.loads(journal.read_bytes())["after"])

    def test_cancellation_blocks_late_reads_and_proposals(self):
        host = MediatedHost(self.workspace, frozenset({"assumptions"}))
        source = (self.workspace / "brief.md").read_bytes()
        start = source.index(b"Keep the first release")
        end = start + len(b"Keep the first release on the current schedule.")
        host.open_session(
            "brief.md",
            start,
            end,
            {"assumptions": (self.workspace / "assumptions.md").read_bytes()},
        )
        host.cancel()
        with self.assertRaises(PermissionError):
            host.read_tool("assumptions")
        with self.assertRaises(PermissionError):
            host.propose_tool(b"ignored")

    def test_bubblewrap_command_shape_when_available(self):
        """Document the intended OS sandbox. Skip if bwrap is not installed."""
        from shutil import which

        if which("bwrap") is None:
            self.skipTest("bubblewrap not installed")
        # Read-only bind of an empty dir as the child cwd; no /home workspace bind.
        command = [
            "bwrap",
            "--die-with-parent",
            "--unshare-pid",
            "--ro-bind", "/usr", "/usr",
            "--ro-bind", "/lib", "/lib",
            "--ro-bind", "/lib64", "/lib64",
            "--ro-bind", "/bin", "/bin",
            "--proc", "/proc",
            "--dev", "/dev",
            "--tmpfs", "/tmp",
            "--bind", str(self.sandbox), str(self.sandbox),
            "--chdir", str(self.sandbox),
            "true",
        ]
        import subprocess

        completed = subprocess.run(command, check=False, capture_output=True, text=True)
        if completed.returncode != 0 and "user namespaces" in (completed.stderr or ""):
            self.skipTest("kernel denies unprivileged user namespaces here")
        self.assertEqual(completed.returncode, 0, completed.stderr)


if __name__ == "__main__":
    unittest.main(verbosity=2)
