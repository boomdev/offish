"""Disposable model of the Phase 0 review and recovery contract."""
from dataclasses import dataclass
from hashlib import sha256
from pathlib import Path
from tempfile import TemporaryDirectory
import errno
from contextlib import contextmanager
from fcntl import flock, LOCK_EX
from stat import S_IMODE
from uuid import uuid4
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


def durable_replace(path, data, *, mode=0o600, expected=None):
    """Linux local-filesystem experiment. Temp and destination share a directory."""
    with TemporaryDirectory(dir=path.parent) as directory:
        staged = Path(directory) / "staged"
        with staged.open("wb") as stream:
            stream.write(data)
            stream.flush()
            os.fchmod(stream.fileno(), mode)
            os.fsync(stream.fileno())
        if expected is not None and path.read_bytes() != expected:
            raise ValueError("changed after journaling")
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
    durable_replace(path, output, mode=S_IMODE(path.stat().st_mode), expected=preimage)


def recovery_state(path, journal):
    entry = json.loads(journal.read_bytes())
    current = digest(path.read_bytes())
    if current == digest(bytes.fromhex(entry["before"])):
        return "original-on-disk"
    if current == entry["after"]:
        return "proposal-on-disk"
    return "conflict"


def restore(path, journal):
    entry = json.loads(journal.read_bytes())
    current = path.read_bytes()
    if digest(current) != entry["after"]:
        raise ValueError("restore would overwrite an unrelated revision")
    durable_replace(path, bytes.fromhex(entry["before"]),
                    mode=S_IMODE(path.stat().st_mode), expected=current)


@dataclass(frozen=True)
class Hunk:
    hunk_id: str
    start: int
    end: int
    expected: bytes
    replacement: bytes


@dataclass(frozen=True)
class ChangeSet:
    """One proposal made of independent hunks against a captured revision."""

    operation_id: str
    revision: str
    hunks: tuple

    def apply(self, source, hunk_ids):
        if digest(source) != self.revision:
            raise ValueError("stale proposal")
        selected = [hunk for hunk in self.hunks if hunk.hunk_id in hunk_ids]
        if len(selected) != len(set(hunk_ids)):
            raise ValueError("unknown hunk")
        if not selected:
            raise ValueError("no hunks selected")
        output = source
        for hunk in sorted(selected, key=lambda item: item.start, reverse=True):
            if output[hunk.start:hunk.end] != hunk.expected:
                raise ValueError("selection changed")
            parts = (output[:hunk.start], hunk.replacement, output[hunk.end:])
            output = "".join(part.decode("utf-8") for part in parts).encode()
        return output

    def rebase_remaining(self, new_source, accepted_ids):
        accepted = [hunk for hunk in self.hunks if hunk.hunk_id in accepted_ids]
        remaining = []
        for hunk in self.hunks:
            if hunk.hunk_id in accepted_ids:
                continue
            shift = 0
            for done in accepted:
                if done.end <= hunk.start:
                    shift += len(done.replacement) - (done.end - done.start)
                elif done.start < hunk.end:
                    raise ValueError("remaining hunk overlaps an accepted hunk")
            start, end = hunk.start + shift, hunk.end + shift
            if new_source[start:end] != hunk.expected:
                raise ValueError("rebase mismatch")
            remaining.append(Hunk(hunk.hunk_id, start, end, hunk.expected, hunk.replacement))
        return ChangeSet(f"{self.operation_id}-rebased", digest(new_source), tuple(remaining))


def propose_changes(source, edits):
    """edits are (start, end, replacement) byte spans. Overlaps are refused."""
    ordered = sorted(edits, key=lambda item: item[0])
    hunks = []
    cursor = 0
    for index, (start, end, replacement) in enumerate(ordered, start=1):
        if start < cursor or not 0 <= start < end <= len(source):
            raise ValueError("hunks overlap or are out of bounds")
        expected = source[start:end]
        "".join(part.decode("utf-8") for part in (source[:start], replacement, source[end:]))
        hunks.append(Hunk(f"h{index}", start, end, expected, replacement))
        cursor = end
    return ChangeSet("op-1", digest(source), tuple(hunks))


class CheckpointLog:
    """Disposable journal model for one document per private directory."""

    def __init__(self, directory, limit=20):
        if limit < 1:
            raise ValueError("checkpoint limit must be positive")
        self.directory = Path(directory)
        self.directory.mkdir(mode=0o700, parents=True, exist_ok=True)
        self.limit = limit
        self.journal = self.directory / "history.json"

    @contextmanager
    def _locked(self):
        # The lock inode stays in place while the journal is replaced.
        fd = os.open(self.directory / "history.lock", os.O_CREAT | os.O_RDWR, 0o600)
        with os.fdopen(fd, "rb") as lock:
            flock(lock, LOCK_EX)
            yield

    def _read(self):
        try:
            return json.loads(self.journal.read_bytes())
        except FileNotFoundError:
            return {"records": [], "head": None, "chain": None}

    def _write(self, state):
        durable_replace(self.journal, json.dumps(state).encode())

    @property
    def records(self):
        return self._read()["records"]

    def _begin(self, state, path, before, after, final, interrupt):
        mode = S_IMODE(path.stat().st_mode)
        pending = {"before": before.hex(), "after": digest(after), "final": final}
        self._write({**state, "pending": pending})
        if interrupt == "before-replace":
            os._exit(73)
        durable_replace(path, after, mode=mode, expected=before)
        if interrupt == "after-replace":
            os._exit(75)
        self._write(final)

    def accept_hunks(self, path, changeset, hunk_ids, interrupt=None):
        with self._locked():
            state = self._read()
            if "pending" in state:
                raise ValueError("resolve the pending operation before accepting")
            preimage = path.read_bytes()
            hunk_ids = sorted(set(hunk_ids))
            output = changeset.apply(preimage, hunk_ids)
            remaining = changeset.rebase_remaining(output, hunk_ids)
            # External edits break continuity with earlier restore points.
            chain = state["chain"] if state["head"] == digest(preimage) else uuid4().hex
            entry = {
                "id": uuid4().hex,
                "operation_id": changeset.operation_id,
                "hunk_ids": hunk_ids,
                "before": preimage.hex(),
                "after": digest(output),
                "chain": chain,
            }
            final = {"records": (state["records"] + [entry])[-self.limit:],
                     "head": entry["after"], "chain": chain}
            self._begin(state, path, preimage, output, final, interrupt)
            return remaining

    @staticmethod
    def _recovery_state(path, pending):
        if pending is None:
            return "idle"
        try:
            current = digest(path.read_bytes())
        except FileNotFoundError:
            return "conflict"
        if current == pending["after"]:
            return "proposal-on-disk"
        if current == digest(bytes.fromhex(pending["before"])):
            return "original-on-disk"
        return "conflict"

    def recovery_state(self, path):
        return self._recovery_state(path, self._read().get("pending"))

    def recover(self, path):
        with self._locked():
            state = self._read()
            pending = state.get("pending")
            status = self._recovery_state(path, pending)
            if status == "proposal-on-disk":
                self._write(pending["final"])
            elif status == "original-on-disk":
                del state["pending"]
                self._write(state)
            return status

    def restore_to(self, path, checkpoint_id, interrupt=None):
        with self._locked():
            state = self._read()
            if "pending" in state:
                raise ValueError("resolve the pending operation before restoring")
            records = state["records"]
            if not records:
                raise ValueError("no checkpoint")
            current = path.read_bytes()
            if digest(current) != state["head"]:
                raise ValueError("restore would overwrite an unrelated revision")
            index = next((i for i, record in enumerate(records) if record["id"] == checkpoint_id), None)
            if index is None:
                raise ValueError("unknown checkpoint")
            record = records[index]
            if record["chain"] != state["chain"]:
                raise ValueError("checkpoint history has a gap")
            preimage = bytes.fromhex(record["before"])
            final = {"records": records[:index], "head": digest(preimage), "chain": state["chain"]}
            self._begin(state, path, current, preimage, final, interrupt)

    def delete(self, checkpoint_id):
        with self._locked():
            state = self._read()
            if "pending" in state:
                raise ValueError("resolve the pending operation before deleting")
            state["records"] = [record for record in state["records"] if record["id"] != checkpoint_id]
            self._write(state)


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


class HunkHistoryTests(unittest.TestCase):
    def setUp(self):
        directory = self.enterContext(TemporaryDirectory())
        self.path = Path(directory) / "brief.md"
        self.private = Path(directory) / "checkpoints"
        self.original = (FIXTURE / "brief.md").read_bytes()
        self.path.write_bytes(self.original)
        self.first = self.original.index(BEFORE)
        self.second = self.original.index(b"2,500")
        self.changes = propose_changes(self.original, [
            (self.first, self.first + len(BEFORE), AFTER),
            (self.second, self.second + len(b"2,500"), b"3,200"),
        ])

    def test_partial_acceptance_rebases_the_remaining_hunk(self):
        log = CheckpointLog(self.private, limit=20)
        remaining = log.accept_hunks(self.path, self.changes, ["h1"])
        self.assertIn(AFTER, self.path.read_bytes())
        self.assertIn(b"2,500", self.path.read_bytes())
        self.assertEqual(len(remaining.hunks), 1)
        self.assertNotEqual(remaining.hunks[0].start, self.second)
        log.accept_hunks(self.path, remaining, ["h2"])
        self.assertIn(b"3,200", self.path.read_bytes())
        self.assertNotIn(b"2,500", self.path.read_bytes())
        self.assertEqual(len(log.records), 2)

    def test_overlapping_hunks_are_refused(self):
        with self.assertRaisesRegex(ValueError, "overlap"):
            propose_changes(self.original, [
                (self.first, self.first + 10, b"A"),
                (self.first + 4, self.first + 12, b"B"),
            ])

    def test_restore_walks_history_and_keeps_later_manual_edits(self):
        log = CheckpointLog(self.private, limit=20)
        first_remaining = log.accept_hunks(self.path, self.changes, ["h1"])
        log.accept_hunks(self.path, first_remaining, ["h2"])
        newest = log.records[-1]["id"]
        oldest = log.records[0]["id"]
        log.restore_to(self.path, newest)
        self.assertIn(b"2,500", self.path.read_bytes())
        self.assertIn(AFTER, self.path.read_bytes())
        log.restore_to(self.path, oldest)
        self.assertEqual(self.path.read_bytes(), self.original)
        self.assertEqual(log.records, [])

        log.accept_hunks(self.path, self.changes, ["h1"])
        self.path.write_bytes(self.path.read_bytes() + b"\nManual.\n")
        with self.assertRaisesRegex(ValueError, "unrelated revision"):
            log.restore_to(self.path, log.records[-1]["id"])
        self.assertTrue(self.path.read_bytes().endswith(b"\nManual.\n"))

    def test_retention_and_delete_drop_restore_points_only(self):
        log = CheckpointLog(self.private, limit=1)
        log.accept_hunks(self.path, self.changes, ["h1"])
        kept = self.path.read_bytes()
        second = propose_changes(kept, [(kept.index(b"2,500"), kept.index(b"2,500") + 5, b"3,200")])
        log.accept_hunks(self.path, second, ["h1"])
        self.assertEqual(len(log.records), 1)
        self.assertIn(b"3,200", self.path.read_bytes())
        dropped = log.records[0]["id"]
        log.delete(dropped)
        with self.assertRaisesRegex(ValueError, "no checkpoint"):
            log.restore_to(self.path, dropped)
        self.assertIn(b"3,200", self.path.read_bytes())

    def accept_replacement(self, log, replacement):
        before = self.path.read_bytes()
        proposal = propose_changes(before, [(0, len(before), replacement)])
        log.accept_hunks(self.path, proposal, ["h1"])

    def test_restart_and_pruning_keep_distinct_restore_points(self):
        log = CheckpointLog(self.private)
        for index in range(22):
            self.accept_replacement(log, str(index).encode())
        log = CheckpointLog(self.private)
        self.assertEqual(len({record["id"] for record in log.records}), 20)
        self.assertEqual(len(log.records), 20)
        newest = log.records[-1]["id"]
        oldest = log.records[0]["id"]
        log.restore_to(self.path, newest)
        self.assertEqual(self.path.read_bytes(), b"20")
        log.restore_to(self.path, oldest)
        self.assertEqual(self.path.read_bytes(), b"1")

    def test_delete_middle_and_newest_preserves_other_restore_points(self):
        log = CheckpointLog(self.private)
        for replacement in (b"B", b"C", b"D"):
            self.accept_replacement(log, replacement)
        oldest, middle, newest = [record["id"] for record in log.records]
        log.delete(middle)
        log.delete(newest)
        self.assertEqual(self.path.read_bytes(), b"D")
        log.restore_to(self.path, oldest)
        self.assertEqual(self.path.read_bytes(), self.original)

    def test_external_edit_breaks_checkpoint_chain(self):
        log = CheckpointLog(self.private)
        self.accept_replacement(log, b"B")
        oldest = log.records[0]["id"]
        self.path.write_bytes(b"External")
        self.accept_replacement(log, b"C")
        with self.assertRaisesRegex(ValueError, "history has a gap"):
            log.restore_to(self.path, oldest)
        self.assertEqual(self.path.read_bytes(), b"C")

    def test_file_and_journal_modes(self):
        self.path.chmod(0o640)
        log = CheckpointLog(self.private)
        self.accept_replacement(log, b"B")
        self.assertEqual(S_IMODE(self.path.stat().st_mode), 0o640)
        self.assertEqual(S_IMODE(log.journal.stat().st_mode), 0o600)
        self.assertEqual(S_IMODE(self.private.stat().st_mode), 0o700)
        log.restore_to(self.path, log.records[0]["id"])
        self.assertEqual(S_IMODE(self.path.stat().st_mode), 0o640)

    def test_restart_recovers_both_sides_of_replace_and_restore(self):
        log = CheckpointLog(self.private)

        def interrupt(operation, exit_code):
            child = os.fork()
            if child == 0:
                try:
                    operation()
                finally:
                    os._exit(74)
            _, status = os.waitpid(child, 0)
            self.assertEqual(os.waitstatus_to_exitcode(status), exit_code)

        interrupt(lambda: log.accept_hunks(self.path, self.changes, ["h1"], "before-replace"), 73)
        reopened = CheckpointLog(self.private)
        self.assertEqual(reopened.recover(self.path), "original-on-disk")
        self.assertEqual(reopened.records, [])
        self.assertEqual(self.path.read_bytes(), self.original)

        interrupt(lambda: log.accept_hunks(self.path, self.changes, ["h1"], "after-replace"), 75)
        reopened = CheckpointLog(self.private)
        self.assertEqual(reopened.recover(self.path), "proposal-on-disk")
        self.assertEqual(reopened.recover(self.path), "idle")
        self.assertEqual(len(reopened.records), 1)
        checkpoint = reopened.records[0]["id"]
        interrupt(lambda: reopened.restore_to(self.path, checkpoint, "after-replace"), 75)
        reopened = CheckpointLog(self.private)
        self.assertEqual(reopened.recover(self.path), "proposal-on-disk")
        self.assertEqual(self.path.read_bytes(), self.original)
        self.assertEqual(reopened.records, [])

    def test_recovery_preserves_external_conflict(self):
        log = CheckpointLog(self.private)
        write = log._write

        def write_state(state):
            if "pending" not in state:
                raise OSError(errno.ENOSPC, "injected finalization failure")
            return write(state)

        with patch.object(log, "_write", side_effect=write_state):
            with self.assertRaises(OSError):
                log.accept_hunks(self.path, self.changes, ["h1"])
        self.path.write_bytes(b"External")
        reopened = CheckpointLog(self.private)
        self.assertEqual(reopened.recover(self.path), "conflict")
        self.assertEqual(reopened.recover(self.path), "conflict")
        self.assertEqual(self.path.read_bytes(), b"External")


if __name__ == "__main__":
    unittest.main(verbosity=2)
