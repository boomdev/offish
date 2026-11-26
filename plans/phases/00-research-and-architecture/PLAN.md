# Phase 0 — Research and architecture

Status: In progress\
Owner: Project maintainer (assumed; confirm)\
Master requirements: ED-01, DOC-01, WS-01–03, CHG-01–02, AI-01–02, GIT-01, SYNC-01, SHEET-01, SLIDE-01, REF-01, IO-01–02, LINK-01, EXT-01, NFR-01–06\
[Master plan](../../MASTER_PLAN.md) · [Initial technology assessment](../../research/INITIAL_TECHNOLOGY_ASSESSMENT.md) · [Phase 0 evidence](../../research/phase-0/README.md) · [Decisions](../../decisions/)

## 1. Objectives

Validate that Offish can preserve editable local documents and give users meaningful control over AI changes across desktop platforms. Select foundations for Phase 1 and retire the largest risks to Phase 2. Use later-format investigations to test shared boundaries before committing to them.

## 2. Scope

Competitive and ecosystem study, UI/editor comparison, native format and calculation feasibility, recovery semantics, AI integration and security, license/distribution strategy, target architecture and layout, fixtures, benchmarks, and decision records. Use isolated technical proofs of concept after their respective research questions and boundaries are defined.

## 3. Out of scope

Production application, complete editors, production installer, cloud service, complete converter coverage, and implementation plans for Phases 2–7. Build spikes outside the application tree and keep reusable inputs and probes in the research folder. Implementation-depth spreadsheet, presentation, PDF and OCR evaluation belongs to the phase that delivers it (see P0-07).

## 4. Prerequisites and dependencies

- Run the pilot with the project maintainer and early adopters. The number of early adopters and their operating systems are not yet known; P0-01 records them. Use [MIT](../../../LICENSE) for Offish.
- Platform support is tiered (NFR-01). Linux on Omarchy (Arch Linux, Hyprland/Wayland) is the first platform and provides the evidence for Phase 0 and Phase 1 exit. Record the actual Omarchy, Hyprland, WebKitGTK and kernel versions and baseline hardware. macOS and Windows evidence is a gate for the Phase 2 release. Identify machines or CI images for them before Phase 2 planning, and avoid Linux-only design choices meanwhile.
- The first integration targeted for release is Codex through its app-server with ChatGPT sign-in. Cursor sign-in is the next candidate if Codex is blocked. API-key and local-model adapters are later-release work. Verify that a sign-in path is supported for Offish before calling it supported.
- Use synthetic or redistribution-cleared fixtures. Define review criteria before collecting results. Maintain a capability matrix and update the source notes when versions or licensing evidence change.
- Establish containment before live workspace access in an agent spike. Use fixture data and stub adapters first.

## 5. Technical design for investigations

Use repeatable scenarios across candidates and record versions, commands, input size, run conditions, results, and decision implications. Keep spikes disposable, but pin their dependencies so results can be reproduced. For each area compare capability, responsiveness and memory, platform portability, accessibility where applicable, upstream maintenance, extensibility, licensing and redistribution, build/package effort, and long-term maintenance. Mark essential criteria as pass/fail and compare passing options on cost and user value.

For UI use the same tabbed editor, keyboard review, IME, screen reader and large document scenarios on native and WebView candidates. Include font and clipboard behavior. Judge editor alternatives on source fidelity as well as runtime performance. Measure WebView candidates in the embedded engine they would ship with (WebKitGTK on Linux), not only in Chromium. For filesystem and changes, define revision IDs, operation IDs, hunk IDs, apply/recovery states and external concurrency. Model the trust boundaries between UI, filesystem, editor, agent subprocess, provider, and converter. Mark operations that cross them and what validates or authorises each crossing.

## 6. Research tasks

| Task | Requirements | Depends on | Evidence / result |
| --- | --- | --- | --- |
| P0-01 · Product and ecosystem | ED-01, CHG-01, EXT-01 | None | Interview or workflow walkthroughs for first users. Record the early adopter count and operating systems, and confirm or recalibrate the pilot success target. Compare adjacent products and independent, component reuse, and fork strategies. Record pain points and prioritised pilot scenario. |
| P0-02 · Fixtures and baselines | NFR-01–06 | P0-01 pilot scenario | Reproducible fixture corpus, Omarchy hardware baseline, budgets, benchmark procedure, pinned spike dependencies, and source/license inventory. Record the macOS/Windows environments planned for the Phase 2 gate. |
| P0-03 · UI and editor | ED-01, DOC-01, NFR-01, NFR-02, NFR-05 | P0-02 | Compare shells: Tauri on the system WebView, a bundled-Chromium option (Electron or a maintained Chromium runtime for Tauri), Iced, and GPUI. Compare editors: a source-first CodeMirror 6 live preview, ProseMirror with a source-mapped schema, and note Tiptap, Milkdown and Lexical as considered. Build minimal interaction spikes for credible finalists on Omarchy. Record selection, keyboard, IME, editing latency, memory and packaging concerns. Record how the chosen path can add a simple syntax-hiding WYSIWYG mode before the first official release. |
| P0-04 · Markdown fidelity | DOC-01, IO-01, WS-02, NFR-03 | P0-02 | Specify the Offish Markdown profile shared by the editor and importers, including front matter, comments, raw HTML and unknown syntax. Choose one authoritative span parser and show that the editor and Rust validation agree. Compare source-spanning edits and deterministic serialisation with unknown constructs, source mode and external edits. Feed results into P0-03 and P0-05. |
| P0-05 · Workspace and proposals | WS-01–03, CHG-01–02, GIT-01, SYNC-01 | P0-02, draft P0-04 contract | Compare patch overlays, snapshots, staging and worktrees. Model hunks within one proposal, partial acceptance with offset rebasing, a checkpoint history and retention policy, and the relationship between checkpoints and editor undo. Inject failures during replace, partial multi-file apply and rollback. Document observable file semantics, conflict UX and recovery state. Compare search architecture and Git integration options at evidence depth needed for Phase 1. |
| P0-06 · Agent integration | AI-01–02, NFR-04 | P0-02, draft P0-05 contract | Run the Codex app-server spike on the maintainer's signed-in Omarchy session. Confirm that ChatGPT sign-in is supported for an Offish integration. Show that Offish controls the files the agent sees and the changes that reach disk, and test streaming, cancellation and process cleanup. If Codex is blocked, repeat with Cursor sign-in. Keep the adapter contract provider-neutral. |
| P0-07 · Future formats and interoperability | SHEET-01, SLIDE-01, REF-01, IO-01–02, LINK-01 | P0-02 | Resolve only the early interface constraints that the Phase 1–2 contracts need, and defer implementation choices with a reason, impact and earliest decision phase. Completed constraint evidence: Word-to-Markdown structure and image links, dense versus manifest-plus-CSV sheet storage, cross-sheet recalculation, and positioned versus narrative slide data. Deferred to Phase 3: PDF reading order, OCR and AI-correction thresholds, and converter distribution. Deferred to Phases 4–5: formula corpus agreement, XLSX/PPTX round trips, and layout fidelity. |
| P0-08 · Licensing and distribution | NFR-06, EXT-01 | Candidate inventories from P0-03–07 | Confirm the MIT license setup and assess notice obligations, contributor process, package rights, optional sidecars and models, and signing/update ownership. Record exact licenses at evaluated versions. |
| P0-09 · Architecture and handoff | All Phase 1 dependencies | P0-03–08 | Bring ADRs to the template, write boundary diagrams, target repository structure, risk register, Phase 1 prerequisites and acceptance evidence. Set follow-up owners and phase gates for deferrals. |

Start P0-01 and source/license discovery early. Run format and UI feasibility together once common fixtures exist. Iterate P0-04, P0-05 and P0-06 on draft contracts before locking decisions.

P0-T29 follow-up (WS-02, CHG-01, NFR-03): verify durable checkpoint
finalization after restart, unique retained IDs, deletion without loss of
revision continuity, file modes, and the undo boundary in CodeMirror.

### P0-03 Omarchy WebKitGTK follow-up

| Task | Requirements | Acceptance evidence |
| --- | --- | --- |
| P0-03a · Structured Markdown | DOC-01, NFR-03 | P0-T24: in the embedded WebKitGTK editor, nested lists and blockquotes render without losing source bytes; a bounded edit leaves unrelated source unchanged. |
| P0-03b · Keyboard review | CHG-01, NFR-05 | P0-T25: with real keyboard input on Omarchy, focus reaches labelled accept and reject controls; rejection preserves source, acceptance changes only the selected range. |
| P0-03c · Composition | DOC-01, NFR-05 | P0-T26: record a real IME composition session and confirm committed Unicode text survives source export and a reload. Synthetic event dispatch alone does not satisfy this check. |
| P0-03d · Embedded latency | NFR-01, NFR-02 | P0-T27: repeat the 50,000-word input proxy in WebKitGTK, record p50/p95, display refresh, warm/cold state and limitations against the 50 ms target. |

Keep these as spike checks. Record their commands, exact environment, pass/fail results and remaining risks in the Phase 0 evidence before changing ADR-0003's status.

Omarchy follow-up status (26 November 2026): P0-T24 and P0-T25 pass. P0-T27
records p50 and p95 across paced (150 ms) and burst (20 ms) runs after
removing O(N) spike shortcuts; paced runs clear 50 ms (work p50 4.0 ms,
second-frame p95 32.0–33.0 ms), while direct keystroke-to-paint evidence
remains open. P0-T26 is pending because the active Fcitx group has no
composing IME.

## 7. Testing strategy

- Run file and conflict exercises on local Linux filesystems first, including the Omarchy desktop. Include dropped watch events, case changes, unreadable paths, stale proposals, disk-full simulations, interruption, symlink escapes, and external changes during review. Repeat representative cases on a supported synced folder where available. Repeat on macOS and Windows for the Phase 2 gate.
- Test Markdown no-op preservation, localized edits, repeated serialisation, table/image assets, comments, front matter, invalid input, Unicode, and unsupported extensions. Exercise source-to-visual and visual-to-source loss reporting. Check that the editor and the Rust validator report identical spans for the same fixture.
- Verify keyboard navigation, comparison without colour, IME composition, and context around zoom and focus on Omarchy. Run screen reader checks where the assistive technology is mature (VoiceOver on macOS, NVDA on Windows) for the Phase 2 gate, and record the Linux AT-SPI result as observed. Collect p50/p95 latency, memory and startup evidence using repeated runs with cold/warm state recorded.
- Test agent context boundaries, unknown tools, prompt injection through references, cancellation, provider failures, duplicate tool calls, credential redaction and expired grants using fixtures. Capture metadata without storing secrets.
- Use sample DOCX and PDF fixtures, with selected features and expected outputs or documented losses, at the depth needed for the P0-07 constraints.

Spike acceptance tests carry IDs and requirement links in the [Phase 0 evidence](../../research/phase-0/README.md#acceptance-test-index).

## 8. Acceptance criteria

| Gate | Requirements | Required evidence |
| --- | --- | --- |
| G0-1 · Product scope | ED-01, CHG-01 | Pilot audience, early adopter count and operating systems, and MVP workflow affirmed. Known gaps, deferrals, tiered support policy, and user success measures are recorded. |
| G0-2 · UI and source | ED-01, DOC-01, NFR-02, NFR-05 | One viable shell/editor path meets calibrated essential criteria on Omarchy or has an approved fallback and explicit constraint. Markdown fixtures show supported editing behavior with visible loss handling under the Offish Markdown profile. The path to a WYSIWYG mode before the first official release is recorded. |
| G0-3 · Workspace reliability | WS-02, CHG-01, NFR-03 | Filesystem and proposal ADRs describe revision/hunk/conflict/recovery/checkpoint guarantees and pass representative crash and conflict spike checks on Linux. macOS and Windows checks are recorded as Phase 2 release gates with an owner. |
| G0-4 · Agent boundary | AI-01, AI-02, NFR-04 | Least-privilege tool model and test evidence cover scoped context, proposed writes and cancellation. A signed-in Codex session completes the synthetic trial, ChatGPT sign-in is confirmed as supported for Offish, and Offish controls disclosed files and disk writes. Otherwise Cursor sign-in meets the same evidence. Direct-workspace subprocess access requires enforceable containment evidence. |
| G0-5 · Architecture and distribution | NFR-06, all | Accepted foundations cover dependencies, licensing, platform tiers, boundaries and test fixtures. Later-format decisions can be deferred with a reason, impact and earliest decision phase. |

## 9. Risks and open questions

| Risk or question | Owner | Mitigation or experiment | Resolution point |
| --- | --- | --- | --- |
| Native editors may miss keyboard, IME, visual fidelity, or assistive technology thresholds. | P0-03 | Compare system WebView, bundled Chromium and native paths on the same fixtures. | G0-2 |
| WebKitGTK on Hyprland/Wayland may differ from Chromium in rendering, latency and GPU behavior. | P0-03 | Run the Tauri spike on Omarchy, record native Wayland versus XWayland and any renderer workarounds. | G0-2 |
| Two Markdown parsers can disagree on spans. | P0-04 | Choose one authoritative parser and compare editor and Rust spans on the fixture corpus. | G0-2 |
| ChatGPT sign-in may not be supported for a third-party integration such as Offish, or the account may lack entitlement. | P0-06 | Confirm the official terms and run the signed-in Omarchy trial. Fall back to Cursor sign-in. | G0-4 |
| Crash durability differs across operating systems and synced storage. | P0-05 | Linux failure tests now; macOS and Windows tests before the Phase 2 release. | G0-3, Phase 2 gate |
| Pilot size and operating systems are unknown, so the success target may not be reachable. | Maintainer | Record early adopters in P0-01 and recalibrate the target. | G0-1 |
| Which phase delivers the WYSIWYG mode required before the first official release. | Maintainer | Decide once the P0-03 editor path is chosen. | Phase 2 planning |
| Reference hardware and the broader Linux distribution matrix. | P0-02 | Record the Omarchy baseline; extend before Phase 2. | G0-1, Phase 2 planning |

## 10. Deliverables and definition of done

Put research results and benchmark references in `plans/research/` and decisions in `plans/decisions/`. Publish the source/license and capability matrix, reproducible fixture and spike results, architecture and data flow diagrams, recommended repository structure, measurable budgets and outstanding risk list. Complete G0-1 through G0-5 or secure explicit approval for each deferred item with a bounded impact and follow-up gate. Use validated outcomes to prepare the Phase 1 plan before Phase 1 implementation.

## 11. Progress tracking

| Task | Requirement | Status | Evidence / blocker |
| --- | --- | --- | --- |
| P0-01 | ED-01, CHG-01, EXT-01 | In progress | Pilot audience and Omarchy desktop confirmed. A synthetic scenario and ecosystem comparison are available. Early adopter count, operating systems, and walkthroughs are pending. |
| P0-02 | NFR-01–06 | In progress | Pilot fixtures, Debian cloud baseline, initial budgets and pinned Python spike requirements are available. Physical Omarchy hardware baseline recorded on 26 November 2026. Cargo lockfiles are pending. macOS/Windows environments are needed before the Phase 2 gate. |
| P0-03 | ED-01, DOC-01, NFR-01, NFR-02, NFR-05 | In progress | Iced native text-widget check run on Linux. CodeMirror 6 live preview and ProseMirror compared in Chromium (P0-T11, P0-T16–T21). CodeMirror is the proposed MVP editor in ADR-0003. The committed Tauri host rendered the workspace image in native Wayland WebKitGTK; asset path checks passed (P0-T22). Nested Markdown and keyboard review passed in WebKitGTK (P0-T24–T25). Embedded latency proxy runs recorded with p50/p95 after removing O(N) spike shortcuts (P0-T27): paced (150 ms) work p50 4.0 ms, p95 7.0–26.0 ms, second-frame p50 25.0 ms, p95 32.0–33.0 ms; burst (20 ms) work p50 3.0–4.0 ms, p95 12.0–30.0 ms, second-frame p50 26.0–29.0 ms, p95 32.0–63.0 ms. Real IME, direct paint latency and screen reader checks remain open. Bundled-Chromium shell not yet run. |
| P0-04 | DOC-01, IO-01, WS-02, NFR-03 | In progress | Rust source-span, ProseMirror and CodeMirror probes preserved unchanged fixture bytes. The CodeMirror probe handled front matter, CRLF, mixed endings, a BOM and a table cell edit. Offish Markdown profile document, nested structures and the authoritative parser for structural modes pending. |
| P0-05 | WS-01–03, CHG-01–02, GIT-01, SYNC-01 | In progress | Omarchy Btrfs probes pass for hunk rebase, checkpoint retention and deletion, restart recovery and mode bits. Chromium verifies the undo barrier (P0-T29). Power-loss durability, full metadata, concurrent writers, symlinks and dropped watch events remain. |
| P0-06 | AI-01–02, NFR-04 | Evidence complete (ADR review pending) | Signed-in Omarchy Codex trial passed streaming and interruption (P0-T13). OpenAI SIWC + Codex app-server is confirmed as the shipping sign-in path. Offline host mediation, redaction and `bwrap` isolation pass (P0-T23, 6/6). Live `bwrap`-contained `codex app-server` passed canary isolation, zero direct writes, active-stream interrupt, process cleanup and audit redaction (P0-T28a–T28d). Ready for ADR-0005 review under G0-5; SIWC OAuth flow and macOS/Windows sandboxes are Phase 2 work. Cursor sign-in remains the fallback. |
| P0-07 | SHEET-01, SLIDE-01, REF-01, IO-01–02, LINK-01 | In progress | Word, PDF, OCR, dense/sparse storage, sample calculations and slide geometry probes run. Proposed deferrals to Phases 3–5 in section 6 need maintainer approval under G0-5. |
| P0-08 | NFR-06, EXT-01 | In progress | MIT project license adopted. Candidate component licenses recorded. Package-level transitive audit pending stack selection. |
| P0-09 | All Phase 1 dependencies | In progress | ADRs restructured to the template. ADR-0004 accepted; ADR-0001, 0002, 0003 and 0005 proposed. Release gates and Phase 1 handoff remain open. |
