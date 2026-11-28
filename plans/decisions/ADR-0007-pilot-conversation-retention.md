# ADR-0007 — Keep bounded pilot conversations in private local storage

Status: Implemented for the Phase 2 pilot. Maintainer review pending for release.\
Date: 2026-11-28\
Requirements and phase: AI-01, AI-02, NFR-04 · P2-06\
Related decisions: [ADR-0002](ADR-0002-proposals-and-recovery.md), [ADR-0005](ADR-0005-mediated-agent-integration.md), [ADR-0006](ADR-0006-phase-1-search-index.md)

## Context

The pilot needs a recoverable conversation that stays outside the content
tree. An approved request can expose a selection and several references
to an adapter. The user needs to see precisely which text is included and
be able to remove the transcript. Reusing an approval or attached text
after restart would require a new disclosure decision.

## Options

| Option | Recovery | Exposure and maintenance |
| --- | --- | --- |
| Keep every turn in memory | Conversation disappears on restart | Minimal durable exposure |
| Keep complete turns, including context snapshots and proposals | Can replay an exact request | Copies source text into durable storage and creates an approval replay risk |
| Keep a bounded transcript and ephemeral context | Restore messages and show interrupted requests | Requires a private store and fresh approval for each turn |

## Decision

For the pilot, keep the latest 20 turns per saved workspace in the
application's private local data directory. A turn stores the user-entered
message, the visible fixture response, the number of approved references
and the outcome. An approved turn is initially recorded as interrupted
and updated at completion or cancellation. Clearing the conversation
deletes its transcript. Another window finishing a cleared turn cannot
recreate it.

Keep selection bytes, reference snapshots, grants and unaccepted
proposals in memory. The host shows the exact text before authorization.
Captured snapshots are limited to a 2 KiB message, a 4 KiB selection and
eight UTF-8 references of up to 4 KiB each. Folder traversal is bounded.
Reference reads skip hidden entries, follow the search ignore policy, and
use read-only, no-follow workspace handles. Keep the selected document
limited to its approved selection even when a mentioned folder contains
it. Cancellation or a workspace switch drops any active grant. Starting
again requires fresh context approval.

## Consequences and validation

Restart recovers the transcript and shows incomplete turns as
interrupted. The fixture provider operates on the current approved
snapshot. An interrupted or earlier turn needs to be started again to
generate a reviewable edit.

P2-T04 checks immutable context, excluded canaries, cancellation, stale
proposals, history bounds, deletion and absence of captured source text
from the transcript. The release review can revisit the 20-turn default
and any history controls based on the maintainer's pilot and the live
adapter's disclosure requirements.
