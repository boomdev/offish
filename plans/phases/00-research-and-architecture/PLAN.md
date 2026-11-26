# Phase 0 — Research and architecture

Status: In progress · Owner: TBD\
[Master plan](../../MASTER_PLAN.md) · [Initial technology assessment](../../research/INITIAL_TECHNOLOGY_ASSESSMENT.md) · [Phase 0 evidence](../../research/phase-0/README.md)

## 1. Objectives

Validate that Offish can preserve editable local documents and give users meaningful control over AI changes across desktop platforms. Select foundations for Phase 1 and retire the largest risks to Phase 2. Use later-format investigations to test shared boundaries before committing to them.

## 2. Scope

Competitive and ecosystem study, UI/editor comparison, native format and calculation feasibility, recovery semantics, AI integration and security, license/distribution strategy, target architecture and layout, fixtures, benchmarks, and decision records. Use isolated technical proofs of concept after their respective research questions and boundaries are defined.

## 3. Out of scope

Production application, complete editors, production installer, cloud service, complete converter coverage, and implementation plans for Phases 2–7. Build spikes outside the application tree and keep reusable inputs and probes in the research folder.

## 4. Prerequisites and dependencies

- Run the pilot with the project maintainer and early adopters. Use [MIT](../../../LICENSE) for Offish.
- Use Omarchy (Arch Linux, Hyprland/Wayland) for the first hands-on maintainer pilot. Record the actual Omarchy, Hyprland, WebKitGTK and kernel versions and baseline hardware. Identify test machines or CI images for the latest stable Windows, macOS, and remaining Linux coverage.
- Prioritise Codex, Cursor, and Claude trials. Verify the supported integration and authentication options for each available account before attempting a live trial.
- Use synthetic or redistribution-cleared fixtures. Define review criteria before collecting results. Maintain a capability matrix and update the source notes when versions or licensing evidence change.
- Establish containment before live workspace access in an agent spike. Use fixture data and stub adapters first.

## 5. Technical design for investigations

Use repeatable scenarios across candidates and record versions, commands, input size, run conditions, results, and decision implications. Keep spikes disposable. For each area compare capability, responsiveness and memory, platform portability, accessibility where applicable, upstream maintenance, extensibility, licensing and redistribution, build/package effort, and long-term maintenance. Mark essential criteria as pass/fail and compare passing options on cost and user value.

For UI use the same tabbed editor, keyboard review, IME, screen reader and large document scenarios on native and WebView candidates. Include font and clipboard behavior. Judge editor alternatives on source fidelity as well as runtime performance. For filesystem and changes, define revision IDs, operation IDs, apply/recovery states and external concurrency. Model the trust boundaries between UI, filesystem, editor, agent subprocess, provider, and converter. Mark operations that cross them and what validates or authorises each crossing.

## 6. Research tasks

| Task | Requirements | Depends on | Evidence / result |
| --- | --- | --- | --- |
| P0-01 · Product and ecosystem | ED-01, CHG-01, EXT-01 | None | Interview or workflow walkthroughs for first users. Compare adjacent products and independent, component reuse, and fork strategies. Record pain points and prioritised pilot scenario. |
| P0-02 · Fixtures and baselines | NFR-01–06 | P0-01 pilot scenario | Reproducible fixture corpus, OS/hardware matrix, budgets, benchmark procedure, and source/license inventory. |
| P0-03 · UI and editor | ED-01, DOC-01, NFR-01, NFR-02, NFR-05 | P0-02 | Compare GPUI, Iced and Tauri plus editor components. Build minimal interaction spikes for credible finalists. Record selection, keyboard, screen reader, IME, editing latency, memory and packaging concerns. |
| P0-04 · Markdown fidelity | DOC-01, WS-02, NFR-03 | P0-02 | Compare source-spanning edits and deterministic serialisation with unknown constructs, source mode and external edits. Specify baseline syntax, escape hatches, metadata, and compatibility strategy. Feed results into P0-03 and P0-05. |
| P0-05 · Workspace and proposals | WS-01–03, CHG-01–02, GIT-01, SYNC-01 | P0-02, draft P0-04 contract | Compare patch overlays, snapshots, staging and worktrees. Inject failures during replace, partial multi-file apply and rollback. Document observable file semantics, conflict UX and recovery state. Compare search architecture and Git integration options at evidence depth needed for Phase 1. |
| P0-06 · Agent integration | AI-01–02, NFR-04 | P0-02, draft P0-05 contract | Capture provider/protocol and authentication matrix. Exercise streamed scoped read/propose/cancel with a stub and one supported adapter if credentials are available. Document containment, trust model and fallback route. |
| P0-07 · Future formats and interoperability | SHEET-01, SLIDE-01, REF-01, IO-01–02, LINK-01 | P0-02 | Compare sparse/dense sheet representation and engine candidates using formula, type, formatting, and range fixtures. Compare narrative versus positioned slide models and sample render/import/export fidelity. Check Word-to-Markdown structural preservation and image links. Compare local PDF extraction and OCR candidates on reading order, completeness, and character error rate. Compare optional AI corrections against source-page evidence, including introduced errors. Test shared asset paths and stable references. Resolve early interface constraints and defer later implementation choices. |
| P0-08 · Licensing and distribution | NFR-06, EXT-01 | Candidate inventories from P0-03–07 | Confirm the MIT license setup and assess notice obligations, contributor process, package rights, optional sidecars and models, and signing/update ownership. Record exact licenses at evaluated versions. |
| P0-09 · Architecture and handoff | All Phase 1 dependencies | P0-03–08 | Write ADRs, boundary diagrams, target repository structure, risk register, Phase 1 prerequisites and acceptance evidence. Set follow-up owners and dates or phase gates for deferrals. |

Start P0-01 and source/license discovery early. Run format and UI feasibility together once common fixtures exist. Iterate P0-04, P0-05 and P0-06 on draft contracts before locking decisions.

## 7. Testing strategy

- Run file and conflict exercises on local filesystems per OS. Include dropped watch events, case changes, unreadable paths, stale proposals, disk-full simulations, interruption, symlink escapes, and external changes during review. Repeat representative cases on a supported synced folder where available.
- Test Markdown no-op preservation, localized edits, repeated serialisation, table/image assets, comments, front matter, invalid input, Unicode, and unsupported extensions. Exercise source-to-visual and visual-to-source loss reporting.
- Verify keyboard navigation, screen reader semantics, comparison without colour, IME composition, and context around zoom and focus. Collect p50/p95 latency, memory and startup evidence using repeated runs with cold/warm state recorded.
- Test agent context boundaries, unknown tools, prompt injection through references, cancellation, provider failures, duplicate tool calls, credential redaction and expired grants using fixtures. Capture metadata without storing secrets.
- Use sample DOCX, XLSX, PPTX, ODT and PDF fixtures, with selected complex features and expected outputs or documented losses. Include spreadsheet dependency updates, dates, formula errors, row/column moves and large bounded reads.

## 8. Acceptance criteria

| Gate | Required evidence |
| --- | --- |
| G0-1 · Product scope | Pilot audience and MVP workflow affirmed. Known gaps, deferrals, support policy, and user success measures are recorded. |
| G0-2 · UI and source | One viable shell/editor path meets calibrated essential criteria or has an approved fallback and explicit constraint. Markdown fixtures show supported editing behavior with visible loss handling. |
| G0-3 · Workspace reliability | Filesystem and proposal ADRs describe revision/conflict/recovery guarantees and pass representative crash and conflict spike checks on the supported platforms. Any gaps carry a named owner and release gate. |
| G0-4 · Agent boundary | Least-privilege tool model and test evidence cover scoped context, proposed writes and cancellation. Record a supported authentication path and a fallback if the preferred provider fails verification. Direct-workspace subprocess access requires enforceable containment evidence. |
| G0-5 · Architecture and distribution | Accepted foundations cover dependencies, licensing, platform targets, boundaries and test fixtures. Later-format decisions can be deferred with a reason, impact and earliest decision phase. |

## 9. Risks and open questions

- Candidate native editors may miss keyboard, IME, visual fidelity, or assistive technology thresholds. P0-03 owns a WebView fallback comparison.
- Public APIs and subscriptions vary in entitlement. P0-06 owns the evidence matrix and identifies how constrained adapters degrade.
- Crash durability differs across operating systems and synced storage. P0-05 owns platform guarantees and failure tests.
- Use pilot fixtures to calibrate PDF extraction thresholds. Include text-layer, scanned, mixed, and multi-column PDFs. Check that gaps are flagged, that OCR works offline, and that AI suggestions cite the relevant page and remain reviewable. For Word, verify editable text, headings, lists, links, tables, and images in portable Markdown. Record comments, tracked changes, and advanced layout as deferred features in conversion reports. P0-02 owns the reference hardware and Linux distribution matrix. P0-06 owns verification of available account entitlements and integration paths.

## 10. Deliverables and definition of done

Put research results and benchmark references in `plans/research/` and decisions in `plans/decisions/`. Publish the source/license and capability matrix, reproducible fixture and spike results, architecture and data flow diagrams, recommended repository structure, measurable budgets and outstanding risk list. Complete G0-1 through G0-5 or secure explicit approval for each deferred item with a bounded impact and follow-up gate. Use validated outcomes to prepare the Phase 1 plan before Phase 1 implementation.

## 11. Progress tracking

| Task | Status | Evidence |
| --- | --- | --- |
| P0-01 | In progress | Pilot audience and Omarchy desktop confirmed. A synthetic scenario and ecosystem comparison are available. Maintainer and early adopter walkthroughs are pending. |
| P0-02 | In progress | Pilot fixtures, Debian cloud baseline, and initial budgets are available. Physical Omarchy hardware baseline and macOS/Windows validation environments are pending. |
| P0-03 | In progress | Iced native text-widget and Chromium ProseMirror checks run on Linux. Tauri host compiled. Omarchy Wayland WebKitGTK runtime and accessibility validation remain open. |
| P0-04 | In progress | Rust source-span and Chromium visual edit probes preserved unchanged fixture bytes. Broader structure and offset coverage pending. |
| P0-05 | In progress | Linux proposal/recovery, Git status, and file search probes run. Other platform and concurrent-writer guarantees pending. |
| P0-06 | In progress | Provider documentation and stub checks available. The installed Codex app server starts but the live request was rejected as unauthorised. Cursor and Claude account trials pending. |
| P0-07 | In progress | Word, PDF, OCR, dense/sparse storage, sample calculations and slide geometry probes run. PDF reading order and broader format validation pending. |
| P0-08 | In progress | MIT project license adopted. Candidate component licenses recorded. Package-level transitive audit pending stack selection. |
| P0-09 | In progress | Proposed foundation ADRs and accepted license ADR recorded. Release gates and Phase 1 handoff remain open. |
