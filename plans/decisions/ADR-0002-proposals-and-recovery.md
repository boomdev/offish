# ADR-0002 — Review proposals against a captured revision

Status: Proposed\
Date: 2026-11-26\
Requirements: WS-02, CHG-01, CHG-02, NFR-03

## Context and options

Proposals need isolation before review, exact restore after acceptance, and
conflict detection in folders that external programs can edit. A copy of each
file is simple but has storage and synchronisation costs. Git worktrees can
isolate some agents in projects using Git and require separate containment.
Typed operations with staged output and a checkpoint journal work in normal
folders and give reviewers a semantic preview.

## Proposed decision

Keep proposals and credentials outside the content tree by default. A proposal
has an operation ID, document identity, base revision, expected source span,
and proposed replacement. Resolve it against a validated handle. Reject
stale proposals and regenerate preview after rebase. At acceptance, validate
scope, schema, revision, and dependencies again. Persist a private preimage
and expected postimage digest before replacing accepted bytes. Observe
normal filesystem ownership, ACLs, permissions and file attributes explicitly.
Serialize Offish writers per document and retain both revisions when an
external conflict is known or recovery is ambiguous.

| Disk state after durable journal | Recovery action |
| --- | --- |
| Matches preimage | Content remains at base. Report the interrupted operation and offer a fresh review. |
| Matches postimage | Offer the checkpoint and restore only after another revision check. |
| Differs from both | Preserve on-disk bytes and checkpoint. Offer conflict resolution. |

The Linux spike passed a process exit after journaling (before replacement),
an injected ENOSPC during replacement, stale proposal rejection and restore
conflict checks. External applications can still race the last observation
and replacement on ordinary files. Mitigate with per-document coordination
among Offish processes, narrow the window, test platform semantics and tell
users what external edits the app can detect. Per-file atomic replacement
and multi-file recoverability have different guarantees.

## Validation and reconsideration

[Evidence and reproduction](../research/phase-0/README.md) cover only a small
Linux fixture on overlayfs. Verify directory durability and metadata
preservation on real local volumes, open-handle behavior on Windows, macOS
replacement, concurrent writers, symlinks, dropped watch events and
restart behavior. Defer journal format and multi-file apply decisions until
those tests define the guarantees.
