# ADR-0009 — Bind conversation modes and model settings to approval

Status: Implemented for the Phase 2 pilot.\
Requirements and phase: AI-01, AI-02, CHG-01, NFR-04 · P2-06 and P2-07 follow-up\
Related decisions: [ADR-0002](ADR-0002-proposals-and-recovery.md), [ADR-0007](ADR-0007-pilot-conversation-retention.md), [ADR-0008](ADR-0008-linux-codex-pilot.md)

## Context

Users need to ask about the active document, request an edit without
selecting a passage, and choose how a Codex turn runs. The expected edit
experience is an inline proposal as soon as the validated result arrives.

## Options

| Option | Effects |
| --- | --- |
| Infer intent from the prompt | A question could unexpectedly produce a proposal. |
| Keep selection mandatory | The user has to select document text before every request. |
| Explicit Ask and Edit with an approved saved document or selection | Users choose intent and see the exact disclosure before sending. |

## Decision

Use **Ask** by default. Ask can include the saved active Markdown document
or a nonempty selection, and its output is an answer. The host rejects
proposal output in Ask mode. Edit uses a saved Markdown selection or the
entire editable body when there is no selection. Keep BOM and front
matter out of the whole-body edit. The pilot allows up to 48 KiB of
document text. These rules supersede the selection-only context and 4 KiB
selection limit in ADR-0007. Its private retention and reference rules
continue to apply.

When a validated Edit result arrives, open inline review automatically.
If the editor is busy or already reviewing a proposal, retain the
manual review action for the pending result. Commit remains explicit
and revision-checked. For whole-body edits, isolate the smallest single
changed span bounded by valid UTF-8 and CRLF positions.

Populate the Codex picker from the signed-in account's [model catalog](https://developers.openai.com/siwc/token-sharing-open-source/models-and-inference).
Read supported efforts and speed tiers from the pinned app-server
catalog. Bind the choice to the same approval as the prompt and context.
Leave model-specific controls unavailable when the runtime has no
capability metadata. If the account catalog cannot be read, offer the
pinned runtime catalog with an explicit notice. The provider still
decides entitlement at inference.

## Validation and reconsideration

P2-T04 covers questions without selection, whole-body edit validation,
automatic review, cancellation and account/runtime capability matching.
Revisit the context budget after measuring representative documents and
the model response budget. Recheck the capability fields when changing
the pinned app-server version.
