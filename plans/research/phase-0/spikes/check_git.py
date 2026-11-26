"""Read structured Git status for an ordinary nested workspace."""
from pathlib import Path
from tempfile import TemporaryDirectory
import subprocess


def git(root, *args):
    return subprocess.check_output(["git", "--no-optional-locks", "-C", str(root), *args])


with TemporaryDirectory() as directory:
    root = Path(directory)
    git(root, "init", "-q")
    root.joinpath(".gitignore").write_text("private/\n")
    root.joinpath("private").mkdir()
    root.joinpath("private/ignore.md").write_text("Example exclusion.\n")
    nested = root / "documents"
    nested.mkdir()
    path = nested / "pilot brief ü.md"
    path.write_text("# Pilot\n")
    assert Path(git(nested, "rev-parse", "--show-toplevel").decode().strip()).samefile(root)
    git(root, "add", "--", str(path))
    path.write_text("# Revised pilot\n")
    entries = git(nested, "status", "--porcelain=v2", "-z", "--untracked-files=all").split(b"\0")
    assert any(entry.startswith(b"1 AM ") and entry.endswith("documents/pilot brief ü.md".encode())
               for entry in entries)
    assert not any(b"private/ignore.md" in entry for entry in entries)
    print("PASS: repository discovery, staged plus working-tree status, UTF-8 path, ignore rule")
