# ADR-0005 — Mediate agent context and proposal output in Offish

Status: Proposed\
Date: 2026-11-26\
Owner and reviewers: Project maintainer; reviewers TBD\
Requirements and phase: AI-01, AI-02, CHG-01, NFR-04 · Phase 0 (P0-06), applies from Phase 2\
Related decisions: [ADR-0002](ADR-0002-proposals-and-recovery.md)

## Context

The first integration targeted for release is Codex with ChatGPT sign-in.
Existing Codex users can sign in with their plan, with no separate API key
or billing setup. External agents bring session behavior and their own
tools, and those tools can read or write the workspace unless constrained.
Providers differ in authentication paths, protocol capabilities and
executable privileges. Offish must control which files an agent sees and
which changes reach disk, whatever the provider. The decision is needed
before Phase 2 agent work.

## Options

| Option | Sign-in for users | Containment | Status |
| --- | --- | --- | --- |
| Codex app-server (JSON-RPC over stdio) with ChatGPT / SIWC | Existing ChatGPT plan via Sign in with ChatGPT OAuth for shipping; local `codex login` for maintainer trials | Needs OS sandbox plus host mediation of files and writes | Synthetic streaming and interruption passed on signed-in Omarchy; SIWC+app-server documented by OpenAI for open-source apps |
| Cursor CLI with Cursor sign-in | Existing Cursor plan | Headless mode can write in its workspace; needs an enforceable boundary | Documentation only |
| Claude Agent SDK or Zed's ACP adapter | Terms refer to commercial agreements; subscription use unverified | Permission model available; needs verification | Documentation only |
| Direct model APIs, API keys, local or OpenAI-compatible endpoints | Separate key and billing, or local setup | Offish owns the tool loop | Later release |

ACP can provide a common editor-facing session transport, while MCP supports
structured tool calls. Each transport requires a host policy.

## Decision

Use a provider-neutral, Offish-owned adapter contract for capability
negotiation, streaming, cancellation, errors, and protocol-specific
authentication.

- Start by passing explicitly approved document selections and reference
  snippets as immutable context. Let an adapter return a candidate operation
  only. Validate its selection, revision and range in Rust and stage it for
  review.
- Cancellation revokes subsequent reads and proposals in Offish even if the
  provider responds late. Never route an approval-grant operation through
  model output. Record context disclosure decisions without logging raw
  credentials or private text in diagnostics.
- Target Codex app-server first. For shipping, use OpenAI’s Sign in with
  ChatGPT open-source OAuth flow and pass `ACCESS_TOKEN` into app-server as
  documented for third-party hosts. Maintainer trials may use an existing
  `codex login` session. Call the adapter supported only after the
  containment checks below pass.
- If Codex is blocked, evaluate Cursor sign-in next. Claude remains a trial
  candidate.
- Enable filesystem-capable adapter modes only after verifying actual
  workspace containment.
- API-key, direct API and local-model adapters are later-release work and
  fit the same contract.

## Consequences

- Users who already have a Codex plan need no new account or billing.
- The first release depends on one provider's sign-in terms and protocol
  stability. Pin the app-server version and test capability negotiation.
- Users without a ChatGPT or Cursor plan have no agent in the first release.
- The external agent's own tools must be disabled or mediated. Read-only
  sandbox modes do not stop reads, so they are not a sufficient boundary.

## Validation and reconsideration

The [Phase 0 evidence](../research/phase-0/README.md) distinguishes the
stub's host-level grant checks from live account and sandbox checks. Before
accepting this decision:

1. Run `spikes/check_codex.py` on the maintainer's signed-in Codex session on
   Omarchy and record the app-server version, streaming and interrupt result.
   Completed 26 November 2026 with Codex CLI 0.172.0: nine streamed text
   events, interruption acknowledged, and final status `interrupted`.
2. Confirm from official OpenAI documentation or terms that ChatGPT sign-in
   is supported for a third-party integration such as Offish.
   Confirmed: OpenAI documents Sign in with ChatGPT for open-source apps and
   a Codex app-server token-sharing guide. Shipping must implement that OAuth
   client flow; a pre-existing CLI login is only a pilot convenience.
3. Show that Offish controls which files the agent sees and which changes
   reach disk: attempted out-of-scope reads, tool calls and direct writes are
   refused.
   Completed 26 November 2026 on Omarchy: offline host mediation and `bwrap`
   isolation checks pass (P0-T23, 6/6), and `check_codex_containment.py --live`
   (P0-T28a) ran the static `codex-cli 0.172.0` binary inside `bwrap` without
   `/bin`, `/usr/bin`, `/home`, or the workspace mounted. An adversarial prompt
   targeting a sibling canary file could not reach it, zero direct disk writes
   occurred, and only Offish's validated proposal path wrote and restored
   `brief.md`.
4. Test cancellation, late output, redaction, and process cleanup against the
   pinned runtime.
   Completed 26 November 2026 on Omarchy: P0-T23 verifies host drop of late
   deltas, post-cancel refusal, and credential/private-text redaction;
   P0-T28b–T28d verify live active-stream interruption (`status=interrupted`),
   clean process teardown (`survivors=0`), and redacted audit output against
   `codex-cli 0.172.0`.

Phase 0 Linux validation items 1–4 are now evidenced; review this ADR for
acceptance under G0-5. Implementing the SIWC OAuth client flow and macOS /
Windows OS sandbox equivalents remain Phase 2 deliverables. If step 2 terms
change or a platform blocks containment, repeat the checks with Cursor
sign-in. Revisit when a provider changes its sign-in terms or protocol, or
when API-key and local adapters are scheduled.
