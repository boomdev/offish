# ADR-0003 — Evaluate a Rust host with a WebView editor first

Status: Proposed\
Date: 2026-11-26\
Requirements: ED-01, DOC-01, NFR-01, NFR-02, NFR-05

## Context and options

The editor needs rich text, keyboard-accessible interaction, reliable
Markdown round trips, and support for Linux, macOS and Windows. Iced and GPUI
keep the presentation layer in Rust. Their built-in text primitives need
additional work to meet the tested office editor scenarios. Tauri hosts
a web editor from a Rust application while adding OS WebView variation,
IPC, package/runtime requirements and JavaScript dependency maintenance.

## Proposed decision

Prioritise a minimal Tauri 2 and ProseMirror candidate for the remaining
validation. Keep Iced as the native comparator. Revisit GPUI if it offers
a demonstrable advantage on the same fixtures. Keep revisioning, validation,
workspace operations and policy in Rust. Restrict WebView navigation and use
a typed bridge, explicit capabilities and application-controlled asset
resolutions. Start with in-process Rust modules and isolate editor transport
from document semantics.

## Validation and reconsideration

The [research evidence](../research/phase-0/README.md) includes an Iced
text-widget edit, a ProseMirror browser component run, and a compiled Tauri
host. The shared Linux image lacked the WebKit network helper at its required
system path, leaving embedded editor behavior unverified. Start runtime validation on the Omarchy maintainer desktop with Hyprland and
Wayland. Validate actual WebViews on all target platforms,
including editable tables, keyboard review, real IME composition, screen
readers and repeatable timings. Resolve lane-specific packaging and
installer requirements. If WebView behavior misses essential criteria,
pursue the native comparator only with a credible rich-text delivery plan.
