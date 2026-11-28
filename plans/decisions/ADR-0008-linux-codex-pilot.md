# ADR-0008 — Isolate the Codex app-server in the Linux pilot

Status: Implemented for pilot verification. Release evidence pending.\
Date: 2026-11-28\
Requirements and phase: AI-01, AI-02, CHG-01, NFR-01, NFR-04, NFR-06 · P2-07\
Related decisions: [ADR-0002](ADR-0002-proposals-and-recovery.md), [ADR-0005](ADR-0005-mediated-agent-integration.md), [ADR-0007](ADR-0007-pilot-conversation-retention.md)

## Context

ADR-0005 selects a mediated Codex adapter and Sign in with ChatGPT. The
Phase 0 spike isolated the runtime with bubblewrap on Omarchy. Shipping
requires independent evidence for the official public OAuth client flow
and for native isolation on each release platform. The desktop shell and
workspace backend currently build on Linux.

## Options

| Option | Credential handling and containment | Consequences |
| --- | --- | --- |
| Reuse the Codex CLI profile | The child receives a profile containing reusable credentials and configuration | Suitable for the existing Phase 0 experiment |
| Run Codex directly with read-only tool permissions | The child can access paths and services available to a normal desktop process | Insufficient isolation for approved selections and references |
| Use Offish OAuth and an isolated ephemeral process | Offish owns approval and refresh. The child receives the current access token and approved text | A separate launcher per platform and one pinned protocol to verify |

## Decision

Use the documented [public OAuth flow](https://developers.openai.com/siwc/token-sharing-open-source/sign-in)
with a system browser and a loopback-only callback. Validate state, PKCE,
the ID token signature and claims, and the granted ChatGPT plan scopes.
Keep credentials and host identity in the Linux Secret Service. Serialize
sign-in, refresh and local disconnect per installation. The pilot
supports one connected account. Disconnection clears the local credential
record. ChatGPT settings manage the remote grant.

On Linux, verify the static Codex CLI 0.172.0 binary inside bubblewrap.
Run one ephemeral thread per approved turn and pass the access token to
the child environment as described in the [official app-server guide](https://developers.openai.com/siwc/token-sharing-open-source/codex-app-server).
Mount the binary and the system DNS and CA files read-only. The child
gets temporary state and no workspace, desktop environment or home
mount. Configure the public Responses endpoint and disable the shell,
web search, Apps and MCP servers. Reject server-initiated requests and
tool items. The outer process uses the host network for inference.
The thread uses read-only tool permissions. A host-network destination
boundary still needs release assessment.

Stream progress when the first model delta arrives, then display the
validated summary and stage one typed replacement for the selected
source span. This keeps partial structured output and protocol diagnostics
out of the transcript. Reuse the revision check and existing review
path. Cancellation discards unread events and terminates the sandboxed
process. The retained transcript identifies the provider. Prior turns
and account credentials are outside the next turn's approved context.

## Dependencies and reconsideration

Use the system bubblewrap, Secret Service client, curl and browser
launcher on Linux. The Omarchy packages provide LGPL-2.1-or-later,
curl and MIT licenses. Use ring 0.17.14 (Apache-2.0 and ISC) for random
values and signature verification. Reuse the locked URL and Base64
libraries. Packaging must account for these dependencies and for the
static Codex binary distribution choice.

Keep this as a Linux pilot until the live ChatGPT entitlement and
containment checks succeed. macOS and Windows need desktop ports,
OS credential stores, isolated launchers and native test environments.
Recheck the pinned protocol and supported sign-in terms before changing
Codex versions or selecting a fallback.
