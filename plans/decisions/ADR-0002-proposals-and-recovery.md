# ADR-0002 — Review proposals against a captured revision

Status: Accepted\
Date: 2026-11-26\
Owner and reviewers: Project maintainer (direction accepted 2026-11-26); reviewers TBD\
Requirements and phase: WS-02, CHG-01, CHG-02, NFR-03 · Phase 0 (P0-05), applies from Phase 2\
Related decisions: [ADR-0001](ADR-0001-markdown-source-preservation.md), [ADR-0005](ADR-0005-mediated-agent-integration.md)

## Context

Proposals need isolation before review, granular acceptance, exact restore
after acceptance, and conflict detection in folders that external programs
can edit. The MVP review transaction targets one file, possibly with several
independent changes. The decision is needed before Phase 2 review work.

## Options

| Option | Works without Git | Semantic preview | Cost |
| --- | --- | --- | --- |
| Full copy of each file | Yes | No | Storage and sync costs |
| Git worktree per proposal | No | No | Needs separate containment; Git-only |
| Typed operations with staged output and a checkpoint journal | Yes | Yes | Needs an operation and journal format |

Criteria: works in ordinary folders, preview quality, recovery guarantees,
and interaction with external writers and sync clients.

## Decision

Use typed operations with staged output and a private checkpoint journal.

- Keep proposals and credentials outside the content tree by default.
- A proposal has an operation ID, document identity, base revision, and one
  or more hunks. Each hunk has a hunk ID, expected source span, and
  replacement. Resolve it against a validated handle.
- Reject stale proposals and regenerate preview after rebase. When the user
  accepts some hunks, rebase the offsets of the remaining hunks against the
  new revision and revalidate them before further acceptance.
- At acceptance, validate scope, schema, revision, and dependencies again.
  Persist a private preimage and expected postimage digest before replacing
  accepted bytes. Observe normal filesystem ownership, ACLs, permissions and
  file attributes explicitly.
- Serialize Offish writers per document and retain both revisions when an
  external conflict is known or recovery is ambiguous.

| Disk state after durable journal | Recovery action |
| --- | --- |
| Matches preimage | Content remains at base. Report the interrupted operation and offer a fresh review. |
| Matches postimage | Offer the checkpoint and restore only after another revision check. |
| Differs from both | Preserve on-disk bytes and checkpoint. Offer conflict resolution. |

Checkpoint history and undo, settled for the MVP:

- **Checkpoint history.** Each acceptance appends a private checkpoint outside
  the content tree. Keep the latest 20 per document; creating another drops
  the oldest. Deleting a checkpoint removes that restore point and does not
  change the file or break revision continuity for other restore points.
  Each checkpoint has a stable ID. Restore requires the file to match the
  latest recorded revision and the checkpoint to belong to its uninterrupted
  revision chain. Restoring returns the file to that checkpoint's preimage
  and discards it and every newer checkpoint. A detected manual edit breaks
  the chain and keeps the edited bytes.
- **Undo.** Editor undo does not cross an acceptance. Acceptance seals the
  undo stack; the control for going back across that boundary is checkpoint
  restore, which checks the file revision first. This avoids a buffer-only
  undo that diverges from the accepted file.

## Consequences

- Works in folders without Git, and accepting changes has no Git side effect
  (GIT-01).
- External applications can still race the last observation and replacement
  on ordinary files. Mitigate with per-document coordination among Offish
  processes, narrow the window, test platform semantics and tell users what
  external edits the app can detect.
- Per-file atomic replacement and multi-file recoverability have different
  guarantees. Multi-file apply stays deferred (CHG-02).
- Private journals and checkpoints hold document content and need retention
  and deletion controls (NFR-04).

## Validation and reconsideration

The maintainer accepted this direction on 2026-11-26. On Omarchy Btrfs the spike recovered after process exit before and after
replacement, including an interrupted restore. It retained conflict bytes
after an injected journal finalization failure. The tests also verified
partial hunk rebase, 20 distinct retained checkpoints across restart,
deletion without breaking the revision chain, and preservation of file
mode bits. The CodeMirror Chromium check verified that undo stops at
acceptance and still works for later edits (P0-T29).
[Evidence and reproduction](../research/phase-0/README.md)

Review is not called ready until power-loss durability, ownership, ACLs and other
file metadata, coordination across processes and external writers, symlinks,
and dropped watch events are verified on Omarchy.
Open-handle behavior on Windows and macOS replacement are Phase 2 release
gates. Defer the on-disk journal format and multi-file apply until those
tests define the guarantees.
