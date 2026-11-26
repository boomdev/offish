# ADR-0003 — Evaluate a Rust host with a WebView editor first

Status: Proposed\
Date: 2026-11-26\
Owner and reviewers: Project maintainer; reviewers TBD\
Requirements and phase: ED-01, DOC-01, NFR-01, NFR-02, NFR-05 · Phase 0 (P0-03), decides Phase 1 shell\
Related decisions: [ADR-0001](ADR-0001-markdown-source-preservation.md)

## Context

The editor needs rich text, keyboard-accessible interaction, reliable
Markdown round trips, and support for Linux, macOS and Windows. Support is
tiered: Omarchy (Hyprland/Wayland) provides Phase 0 and Phase 1 evidence,
and macOS and Windows are gates for the Phase 2 release. The MVP may use a
source-preserving live preview. A simple syntax-hiding WYSIWYG mode is
required before the first official release. The decision is needed before
Phase 1 shell work.

## Options

Shells:

| Option | Strengths | Costs |
| --- | --- | --- |
| Tauri 2 on the system WebView | Small footprint, Rust host, mature web editors | WebKitGTK on Linux differs from Chromium in performance, rendering and GPU behavior; per-OS WebView variance; IPC |
| Bundled Chromium (Electron, or a maintained Chromium runtime for Tauri if available) | One engine on every OS; the Chromium spike results transfer | Larger packages and memory; Rust core needs a sidecar or native module; frequent security updates |
| Iced | Native Rust, MIT | Rich-text structure, edit mapping and accessibility need custom work |
| GPUI | Native Rust, used by Zed | Pre-1.0 churn; rich-text editor and accessibility effort; no spike run |

Editors (for WebView shells):

| Option | Fit with ADR-0001 | Path to WYSIWYG |
| --- | --- | --- |
| CodeMirror 6 live preview | Edits source text directly, so bytes and spans are preserved by construction | Needs widget decorations that hide syntax, with tables the hardest case |
| ProseMirror with a source-mapped schema | The default Markdown serialiser changed bytes and has no table node; needs a custom mapping layer | Native |
| Tiptap, Milkdown, Lexical | Built on or similar to the above; recorded as considered | Native |

Criteria: source fidelity, keyboard and IME behavior, accessibility,
latency on the shipped engine, packaging, and maintenance.

## Decision

Prioritise a minimal Tauri 2 candidate for the remaining validation. Use a
CodeMirror 6 live preview as the MVP editor candidate: it passed every
pilot fixture scenario in Chromium without a source-mapping layer. For the
WYSIWYG mode required before the first official release, first extend the
live preview (hide syntax on the active line, edit table cells as widgets
mapped to cell spans). Keep ProseMirror with a source-mapped schema as the
alternative if that extension proves inadequate. Keep a bundled-Chromium shell as the fallback if WebKitGTK fails essential
criteria on Omarchy. Keep Iced as the native comparator and revisit GPUI if
it offers a demonstrable advantage. Keep revisioning, validation, workspace
operations and policy in Rust. Restrict WebView navigation and use a typed
bridge, explicit capabilities and application-controlled asset resolutions.
Start with in-process Rust modules and isolate editor transport from
document semantics.

## Consequences

- JavaScript editor dependencies need maintenance and license tracking.
- Results measured in Chromium do not transfer to WebKitGTK; latency and
  rendering must be measured in the shipped engine.
- A live preview editor reduces the MVP mapping cost but needs a credible
  plan for the WYSIWYG mode before the first official release. Non-technical
  pilot users will see Markdown syntax on the line they edit, including table
  pipes; record where this costs them confidence.
- The CodeMirror candidate must configure the file's line separator and
  normalise typed and pasted line breaks, or it changes line endings.
- Choosing bundled Chromium would change package size, memory budgets and
  update obligations.

## Validation and reconsideration

The [research evidence](../research/phase-0/README.md) includes an Iced
text-widget edit, ProseMirror and CodeMirror 6 browser component runs on the
same fixture, and a compiled Tauri host. In Chromium the CodeMirror live
preview preserved bytes across line-ending variants, edited a table cell,
replayed UTF-8 byte operations exactly and showed labelled inline review.
Its work-and-layout latency proxy had a p95 of 6.1–11.8 ms against
3.3–4.5 ms for ProseMirror without source mapping, both well inside the
50 ms budget. The shared Linux image lacked the WebKit network helper at its required
system path, leaving embedded editor behavior unverified. Start runtime
validation on the Omarchy maintainer desktop with Hyprland and Wayland, and
record native Wayland versus XWayland and any renderer workarounds. Validate
the CodeMirror candidate there with nested lists and blockquotes, editable
tables, keyboard review, real IME composition and repeatable timings.

On 26 November 2026 the Tauri/ProseMirror host and a temporary CodeMirror
host both opened and accepted keyboard input in native Wayland WebKitGTK on
Omarchy. The CodeMirror table rendered; the image showed alt text because
that temporary host still used a placeholder asset URL. The committed
`tauri-codemirror` spike now serves relative assets through a `workspace:`
protocol scoped to `OFFISH_WORKSPACE`, with path-escape tests. Re-run it on
Omarchy to confirm the timeline image loads. Remaining interaction and
latency checks still decide the shell.

Validate WebViews and screen readers (VoiceOver, NVDA) on macOS and Windows
before the Phase 2 release, and resolve packaging and installer
requirements. If WebKitGTK misses essential criteria, evaluate the
bundled-Chromium fallback. Pursue the native comparator only with a credible
rich-text delivery plan.
