# ADR-0005 — Mediate agent context and proposal output in Offish

Status: Proposed\
Date: 2026-11-26\
Requirements: AI-01, AI-02, CHG-01, NFR-04

## Context and options

The initial trials target Codex, Cursor and Claude. They have different
authentication paths, protocol capabilities and executable privileges.
A direct model API can limit what the caller sends and expose typed tools.
An external agent offers more existing session behavior but can expose
powerful tools unless their operation is constrained. ACP can provide a
common editor-facing session transport, while MCP supports structured tool
calls. Each transport requires a host policy.

## Proposed decision

Use an Offish-owned adapter contract for capability negotiation, streaming,
cancellation, errors, and protocol-specific authentication. Start by passing
explicitly approved document selections and reference snippets as immutable
context. Let an adapter return a candidate operation only. Validate its
selection, revision and range in Rust and stage it for review. Cancellation
revokes subsequent reads and proposals in Offish even if the provider
responds late. Never route an approval-grant operation through model output.
Record context disclosure decisions without logging raw credentials or
private text in diagnostics.

Evaluate Codex app-server and ACP adapters, Cursor's non-interactive CLI and
Claude's Agent SDK or Zed's ACP adapter. Enable filesystem-capable adapter
modes only after verifying actual workspace containment and supported
authentication. A direct model endpoint remains the fallback for a
context-only prototype if a subscription path is unavailable.

## Validation and reconsideration

The [Phase 0 evidence](../research/phase-0/README.md) distinguishes the
stub's host-level grant checks from live account and sandbox checks. Before
accepting this decision, verify the pilot account workflow and process
cleanup. Test cancellation, redaction, attempted out-of-scope reads,
and late output against the pinned runtime and intended user login.
