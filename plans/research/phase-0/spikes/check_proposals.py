"""Disposable model of the Phase 0 review and recovery contract."""
from dataclasses import dataclass
from hashlib import sha256
from pathlib import Path
from tempfile import TemporaryDirectory
import errno
import json
import os
import unittest
from unittest.mock import patch


FIXTURE = Path(__file__).resolve().parent.parent / "fixtures" / "pilot"
BEFORE = "Keep the first release on the current schedule.".encode()
AFTER = "Move the pilot launch to 14 December.".encode()


def digest(data):
    return sha256(data).hexdigest()


@dataclass(frozen=True)
class Proposal:
    revision: str
    start: int
    end: int
    expected: bytes
    replacement: bytes

    def preview(self, source):
        if digest(source) != self.revision:
            raise ValueError("stale proposal")
        if not 0 <= self.start < self.end <= len(source):
            raise ValueError("selection out of bounds")
        if source[self.start:self.end] != self.expected:
            raise ValueError("selection changed")
        # Decode boundaries separately so a patch cannot split a code point.
        parts = (source[:self.start], self.replacement, source[self.end:])
        return "".join(part.decode("utf-8") for part in parts).encode()


class Session:
    """An adapter can work only with context explicitly supplied by the host."""

    def __init__(self, references, source, start, end):
        self.references = dict(references)
        self.source = source
        self.start = start
        self.end = end
        self.cancelled = False

    def cancel(self):
        self.cancelled = True
        self.references.clear()

    def read(self, reference):
        if self.cancelled or reference not in self.references:
            raise PermissionError("reference unavailable")
        return self.references[reference]

    def propose(self, replacement):
        if self.cancelled:
            raise PermissionError("session cancelled")
        proposal = Proposal(digest(self.source), self.start, self.end,
                            self.source[self.start:self.end], replacement)
        proposal.preview(self.source)
        return proposal


def durable_replace(path, data):
    """Linux local-filesystem experiment. Temp and destination share a directory."""
    with TemporaryDirectory(dir=path.parent) as directory:
        staged = Path(directory) / "staged"
        with staged.open("wb") as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(staged, path)
        directory_fd = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
        try:
            os.fsync(directory_fd)
        finally:
            os.close(directory_fd)


def accept(path, journal, proposal, interrupt=None):
    if journal.exists():
        raise ValueError("resolve the current checkpoint before accepting")
    preimage = path.read_bytes()
    output = proposal.preview(preimage)
    entry = {"before": preimage.hex(), "after": digest(output)}
    durable_replace(journal, json.dumps(entry).encode())
    if interrupt == "before-replace":
        os._exit(73)
    if path.read_bytes() != preimage:
        raise ValueError("changed after journaling")
    # An external writer can still race the final check and the replacement.
    durable_replace(path, output)


def recovery_state(path, journal):
    entry = json.loads(journal.read_bytes())
    current = digest(path.read_bytes())
    if current == digest(bytes.fromhex(entry["before"])):
        return "original-on-disk"
    if current == entry["after"]:
        return "proposal-on-disk"
    return "conflict"


def restore(path, journal):
    if recovery_state(path, journal) != "proposal-on-disk":
        raise ValueError("restore would overwrite an unrelated revision")
    entry = json.loads(journal.read_bytes())
    durable_replace(path, bytes.fromhex(entry["before"]))


class ReviewTests(unittest.TestCase):
    def setUp(self):
        directory = self.enterContext(TemporaryDirectory())
        self.path = Path(directory) / "brief.md"
        self.journal = Path(directory) / "review.json"
        self.original = (FIXTURE / "brief.md").read_bytes()
        self.path.write_bytes(self.original)
        start = self.original.index(BEFORE)
        self.session = Session(
            {"assumptions": (FIXTURE / "assumptions.md").read_bytes()},
            self.original, start, start + len(BEFORE),
        )
        self.proposal = self.session.propose(AFTER)

    def test_review_is_private_until_accept_and_restore_is_exact(self):
        preview = self.proposal.preview(self.original)
        self.assertEqual(preview, self.original.replace(BEFORE, AFTER, 1))
        self.assertEqual(self.path.read_bytes(), self.original)
        self.assertFalse(self.journal.exists())
        accept(self.path, self.journal, self.proposal)
        self.assertEqual(self.path.read_bytes(), preview)
        self.assertEqual(recovery_state(self.path, self.journal), "proposal-on-disk")
        restore(self.path, self.journal)
        self.assertEqual(self.path.read_bytes(), self.original)

    def test_reject_has_no_filesystem_effect(self):
        self.proposal.preview(self.original)
        del self.proposal
        self.assertEqual(self.path.read_bytes(), self.original)
        self.assertFalse(self.journal.exists())

    def test_external_edit_blocks_stale_proposal(self):
        external = self.original + b"\nExternal writer.\n"
        self.path.write_bytes(external)
        with self.assertRaisesRegex(ValueError, "stale"):
            accept(self.path, self.journal, self.proposal)
        self.assertEqual(self.path.read_bytes(), external)
        self.assertFalse(self.journal.exists())

    def test_recovery_recognises_pending_journal_and_divergence(self):
        child = os.fork()
        if child == 0:
            try:
                accept(self.path, self.journal, self.proposal, "before-replace")
            finally:
                os._exit(74)
        _, status = os.waitpid(child, 0)
        self.assertEqual(os.waitstatus_to_exitcode(status), 73)
        self.assertEqual(recovery_state(self.path, self.journal), "original-on-disk")
        self.path.write_bytes(b"External writer.")
        self.assertEqual(recovery_state(self.path, self.journal), "conflict")
        with self.assertRaisesRegex(ValueError, "unrelated revision"):
            restore(self.path, self.journal)
        self.assertEqual(self.path.read_bytes(), b"External writer.")

    def test_injected_replace_failure_keeps_a_recoverable_preimage(self):
        real_replace = os.replace

        def replace_with_disk_failure(source, destination):
            if destination == self.path:
                raise OSError(errno.ENOSPC, "injected disk full")
            real_replace(source, destination)

        with patch("os.replace", side_effect=replace_with_disk_failure):
            with self.assertRaises(OSError) as failure:
                accept(self.path, self.journal, self.proposal)
        self.assertEqual(failure.exception.errno, errno.ENOSPC)
        self.assertEqual(self.path.read_bytes(), self.original)
        self.assertEqual(recovery_state(self.path, self.journal), "original-on-disk")

    def test_restore_preserves_later_edits(self):
        accept(self.path, self.journal, self.proposal)
        later = self.path.read_bytes() + b"\nManual revision.\n"
        self.path.write_bytes(later)
        with self.assertRaisesRegex(ValueError, "unrelated revision"):
            restore(self.path, self.journal)
        self.assertEqual(self.path.read_bytes(), later)

    def test_selectors_and_cancellation(self):
        self.assertIn(b"14 December", self.session.read("assumptions"))
        with self.assertRaises(PermissionError):
            self.session.read("../private.md")
        self.session.cancel()
        with self.assertRaises(PermissionError):
            self.session.read("assumptions")
        with self.assertRaises(PermissionError):
            self.session.propose(AFTER)

    def test_patch_cannot_split_utf8(self):
        start = self.original.index("é".encode())
        session = Session({}, self.original, start + 1, start + 2)
        with self.assertRaises(UnicodeDecodeError):
            session.propose(AFTER)


if __name__ == "__main__":
    unittest.main(verbosity=2)
