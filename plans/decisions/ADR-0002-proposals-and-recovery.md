# ADR-0002 — Review proposals against a captured revision

Status: Proposed\
Date: 2026-11-26\
Owner and reviewers: Project maintainer; reviewers TBD\
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

Open design items to settle before acceptance:

- **Checkpoint history.** Each acceptance creates a checkpoint. Define how
  many are retained, where they live, how they are deleted, and how restoring
  an older checkpoint treats later accepted and manual edits.
- **Undo.** Define whether editor undo can cross an acceptance, and how undo
  history and checkpoints are presented so users are not offered two
  conflicting ways to go back.

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

The Linux spike passed a process exit after journaling (before replacement),
an injected ENOSPC during replacement, stale proposal rejection and restore
conflict checks. It models one span per proposal and one outstanding
checkpoint. [Evidence and reproduction](../research/phase-0/README.md) cover
only a small Linux fixture on overlayfs. Before accepting, extend the spike to
several hunks with partial acceptance and a checkpoint history, and verify
directory durability and metadata preservation on real local volumes,
concurrent writers, symlinks, dropped watch events and restart behavior on
Omarchy. Open-handle behavior on Windows and macOS replacement are Phase 2
release gates. Defer journal format and multi-file apply decisions until
those tests define the guarantees.
