# ADR-0003 — Evaluate a Rust host with a WebView editor first

Status: Accepted\
Date: 2026-11-26\
Owner and reviewers: Project maintainer (direction accepted 2026-11-26); reviewers TBD\
Requirements and phase: ED-01, DOC-01, NFR-01, NFR-02, NFR-05 · Phase 0 (P0-03), decides Phase 1 shell\
Related decisions: [ADR-0001](ADR-0001-markdown-source-preservation.md)

## Context

The editor needs rich text, keyboard-accessible interaction, reliable
Markdown round trips, and support for Linux, macOS and Windows. Support is
tiered: Omarchy (Hyprland/Wayland) provides Phase 0 and Phase 1 evidence,
and macOS and Windows are gates for the Phase 2 release. The MVP may use a
source-preserving live preview. On 2026-11-26 the maintainer assigned a
simple syntax-hiding mode to the Phase 2 pilot. The decision is needed before
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
pilot fixture scenario in Chromium without a source-mapping layer. The
Phase 2 pilot includes a simple syntax-hiding mode made by extending that
live preview: hide syntax on the active line, and edit table cells as widgets
mapped to cell spans. Keep ProseMirror with a source-mapped schema as the
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
- Phase 2 includes the live preview and a simple syntax-hiding mode. In the
  live preview, syntax shows on the line being edited. In the simple mode,
  syntax stays hidden there, and table cells are edited as widgets. Where
  the simple mode cannot represent a span, show that limitation. Record where
  either mode costs a user confidence.
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
50 ms budget. The maintainer accepted this direction on 2026-11-26. The
cloud image lacked the WebKit network helper, so the Omarchy desktop runs
below are the runtime evidence.

On 26 November 2026 the Tauri/ProseMirror host and a temporary CodeMirror
host both opened and accepted keyboard input in native Wayland WebKitGTK on
Omarchy. The CodeMirror table rendered; the image showed alt text because
that temporary host still used a placeholder asset URL. The committed
`tauri-codemirror` spike now serves relative assets through a `workspace:`
protocol scoped to `OFFISH_WORKSPACE`, with path-escape tests. Its Omarchy
rerun rendered the timeline graphic and changed the window title after a
keyboard edit in native Wayland WebKitGTK. Remaining interaction and
latency checks still decide the shell.

A repeatable Omarchy WebKitGTK driver then passed nested-list and blockquote
source preservation (including exact UTF-8 byte-operation replay) plus
keyboard accept/reject. On the 50,000-word fixture at 60 Hz, the initial
20 ms burst spike runs had work p95 of 29 ms and second-frame p95 of
54–56 ms due to two O(N) spike shortcuts (full-prefix UTF-8 re-encoding and
full-tree table iteration) plus burst queuing. After replacing both with
incremental updates, three paced runs (150 ms delay) recorded work p50
4.0 ms, work p95 7.0–26.0 ms, second-frame p50 25.0 ms and second-frame p95
32.0–33.0 ms—clearing the 50 ms budget at the 60 Hz two-frame floor. Three
20 ms burst runs recorded work p50 3.0–4.0 ms, work p95 12.0–30.0 ms,
second-frame p50 26.0–29.0 ms and second-frame p95 32.0, 50.0 and 63.0 ms
from occasional tail spikes queuing at 50 chars/sec. These remain
refresh-quantised proxies, not direct keystroke-to-paint evidence. A real
IME session is still pending because the active Fcitx group has no composing
input method; an attempted temporary Mozc selection did not take effect and
the original profile was restored. The direction is accepted. Real IME,
direct paint and accessibility evidence remain before the shell is called
ready. The simple syntax-hiding mode is assigned to the Phase 2 pilot.

Validate WebViews and screen readers (VoiceOver, NVDA) on macOS and Windows
before the Phase 2 release, and resolve packaging and installer
requirements. If WebKitGTK misses essential criteria, evaluate the
bundled-Chromium fallback. Pursue the native comparator only with a credible
rich-text delivery plan.
