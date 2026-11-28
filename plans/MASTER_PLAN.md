# Offish master plan

Status: Proposed · 26 November 2026

## Vision and users

Offish aims to bring the AI collaboration model of modern coding editors to office and knowledge work. A person should be able to open a folder, work visually, ask an agent to reason over authorised material, inspect its proposed edits in the relevant editors, and recover earlier work. Humans, agents, and external tools share open working files within a coherent workspace.

- Analysts and finance teams maintain calculation models and explain how revised assumptions affect decisions.
- Founders and operators keep a business plan, forecast, and investor deck aligned. A future agent reads named assumptions, proposes edits to the plan and slides, and shows reviewers the sources and effects before approval.
- Researchers, writers, and consultants organise references, draft documents, and trace statements back to source material.
- Technical users edit working files in external tools, automate repeatable operations, and track changes with Git. Non-technical users reach the same core workflows through visual controls.

The Phase 0 pilot is the project maintainer, the only early adopter for now, editing Markdown-based documents in local folders on Omarchy. More early adopters are expected and will be recorded when they join. Phase 0 and Phase 1 proceed with this pilot. The first useful release lets the maintainer complete a real author–review–restore workflow on existing files.

## Product principles

- **AI-native.** Make context, typed operations, provenance, validation, and review part of each document type from its first release.
- **Human-friendly.** Visual editing is the default. Users can inspect source without needing Markdown, YAML, JSON, or Git knowledge for routine work.
- **Open formats.** Prefer documented, portable, readable representations with deterministic writes and meaningful diffs. Preserve unsupported source constructs or make the editing limitation explicit.
- **AI-agnostic.** Keep agent protocols, tool interfaces, and model APIs replaceable. Support user-managed accounts, keys, or local models where the provider permits.
- **Local-first.** The filesystem owns accepted content. Editing, calculation, rendering, and file management work offline without an Offish account.
- **User control.** Users control data disclosure, credentials, agents, sync, and acceptance. Proposals are reviewable and recoverable.
- **Performance.** Set measurable budgets against representative documents and workspaces. Use incremental work and bounded reads.
- **Open source.** Document formats, contribution workflows, release packaging, and architectural decisions. Reuse maintained components where integration and distribution costs justify them.

## Functional requirements

| ID | Requirement |
| --- | --- |
| WS-01 | Open ordinary directories. The left explorer browses, creates, renames, moves, duplicates, and deletes files and folders, distinguishes editable and reference files, and shows dirty and Git status. Keep destructive paths reviewable and recoverable where the platform permits. |
| WS-02 | Watch external edits, detect stale buffers, preserve unsaved work, resolve conflicts, validate files, and recover interrupted writes. Support relative references, workspace settings, imported sources, assets, and versioned migrations. |
| WS-03 | Maintain a rebuildable local search index. Respect workspace exclusions, permission boundaries, ignore rules, and external changes. Show scope and freshness for search results. |
| ED-01 | Central tabs host type-specific visual editors and read-only viewers. Provide shortcuts, command palette, undo/redo, optional source mode, and split views where useful. |
| DOC-01 | Visual Markdown editing starts with common formatting, links, lists, tables, and images as fidelity permits. The Phase 2 pilot includes a source-preserving live preview, which renders formatting and reveals Markdown syntax near the cursor, and a simple syntax-hiding mode made by extending that preview. In the simple mode, syntax stays hidden on the line being edited, and table cells are edited as widgets mapped to cell spans. Unsupported spans stay explicit. Extend through footnotes, cross-references, pagination, headers/footers, page breaks, styles, templates, layouts, and embeds. |
| SHEET-01 | Visual multi-sheet grid supports typed values, Excel-style formulas and references, named ranges, cross-sheet recalculation, formatting and conditional formatting, charts, validation, and row/column operations. Investigate semantic names and bounded range access. |
| SLIDE-01 | Visual slide editing supports layouts, themes, images, shapes, tables, charts, notes, and positioning with stable element identities. |
| REF-01 | Progressively support plain text, CSV, JSON, YAML, Mermaid, SVG, images, and PDF viewing, plus diagram and structured-data workflows. Render external assets in restricted viewers. |
| AI-01 | Right panel offers Ask and Edit, provider-supported model, effort and speed settings, persistent conversations, file and folder @ mentions, document or selection context, streamed activity, permission prompts, cancellation, and recovery. Selections cover paragraphs, cells/ranges, slides, and objects. |
| AI-02 | Agents can search and read across the authorised workspace and request typed operations against bounded context. Provide a common integration layer and evaluate a headless CLI and MCP tools. Workspace instructions such as AGENTS.md have explicit trust and scope. |
| CHG-01 | Stage AI proposals against a captured revision. Validate proposed changes, highlight additions/removals in red and green with accessible labels, review inline, accept or reject individual changes or changesets, detect conflicts, checkpoint accepted changes, and support rollback. Explicit opt-in is required for any automatic acceptance. |
| CHG-02 | Extend review and crash recovery to coherent multi-file changesets. Group dependent hunks and revalidate partial acceptance. Provide semantic diffs for paragraph/sentence/format/structure, cell/value/formula/format/structure, and slide/layout/object changes. |
| IO-01 | Import DOCX, XLSX, PPTX, PDF, OpenDocument, HTML, CSV, and Markdown in stages. Preserve source bytes, assets and relevant metadata, trace source to editable derivative, and report conversion losses. Keep source and derivative visibly distinct. |
| IO-02 | Export DOCX, XLSX, PPTX, PDF, HTML, OpenDocument and appropriate open formats in stages. Apply reusable styles/templates and preserve the native working files. Report losses and measure fidelity. |
| GIT-01 | Detect repos and progressively expose init, clone, status, diffs, commit, optional AI commit messages, push/pull, branches, history, restoration, and conflict resolution without terminal knowledge. Accepting AI changes has no Git side effect. |
| SYNC-01 | Validate Dropbox, Google Drive, OneDrive, and other OS-synced folders before dedicated connectors. Design separate user flows for sync, version history, and independent backup/restore. |
| LINK-01 | Add explicit document dependencies, reusable themes and assets, embedded cell values and shared charts, consistency checks, and controlled propagation. |
| EXT-01 | Support repeatable workflows, headless processing, extension points, additional formats and integrations, and eventually multi-user collaboration. |

## Non-functional requirements and first release targets

These are proposed targets to calibrate in Phase 0 on named reference hardware, per OS, with fixtures and reproducible commands.

| ID | Requirement and evidence |
| --- | --- |
| NFR-01 | Target the latest stable Windows and macOS releases and the latest stable release of each selected Linux distribution. Support is tiered: Linux on Omarchy is the first platform and provides the evidence for Phase 0 and Phase 1 exit; macOS and Windows evidence is a gate for the Phase 2 release. Avoid platform-specific design choices that would block the later platforms. Record exact versions for each validation run. Run offline acceptance tests with networking disabled. Verify Rust owns persistence, validation, and document operations across UI choices. |
| NFR-02 | Initial targets include p95 keystroke-to-paint under 50 ms on a 50,000-word document, cold start to usable explorer under 3 s for a 10,000-file fixture, and warm local search p95 under 300 ms. Record cold/warm conditions, peak memory, and slow cases. Later phases define equivalent grid and slide budgets. |
| NFR-03 | Save and reload are deterministic for supported fixtures. No accepted content or recoverable draft is silently lost in crash, concurrent-change, or disk error fixtures. Older supported formats have migration and downgrade/read-only fixtures. |
| NFR-04 | Default-deny agent writes and context disclosure outside authorised scope. Store secrets in OS credential stores. Keep chat and diagnostics local by default, with retention and deletion controls. Verify containment and credential redaction with security tests. |
| NFR-05 | MVP critical flows are keyboard-operable, expose accessible semantics, and avoid colour-only review. Verify focus, screen reader basics, IME, bidirectional text, and Unicode handling. Broaden OS/assistive technology coverage and locale support over time. |
| NFR-06 | Maintain reproducible fixtures, OS CI, dependency and license inventories, versioned format contracts, documented plugin boundaries, and actionable errors. Prepare signed distribution and update policy before broad release. |

## Preliminary architecture

Rust modules own workspace I/O, document models, format codecs, validation, patch application, and agent policy. The desktop shell owns windows, routing, editor views, and interaction. [ADR-0003](decisions/ADR-0003-desktop-and-first-editor.md) accepts a Tauri 2 host and a CodeMirror 6 live preview as the direction. Use typed IPC, disable arbitrary navigation, and keep workspace access behind Rust commands. Reuse the same operations in desktop, CLI, and tool adapters. Start with a single Cargo workspace and in-process modules wherever feasible.

```text
apps/desktop/                 desktop shell and editor views
crates/workspace/             paths, watching, save/recovery, search interfaces
crates/documents/             document codecs, validation, typed operations
crates/changes/               review, baselines, checkpoints, recovery
crates/ai/                    agent adapters, sessions, permissions, tools
crates/interop/               import/export adapters as needed
tests/fixtures/               licensed and synthetic verification corpus
plans/                        planning, research, and ADRs
```

This is a candidate layout. Extract a crate when it has an independently useful boundary and identifiable callers. Add optional Git, CLI, and format modules as required.

### Filesystem and document model

- Resolve relative paths within an authorised root. Define policies for symlinks, junctions, case-only moves, Unicode-equivalent names, ignored paths, and nested repos. Avoid path checks that can be invalidated between validation and use. Keep secrets and private session state outside shared folders.
- Track on-disk revisions with content hashes. Coalesce watch events and rescan after dropped events. Reload clean buffers and surface dirty conflicts with base, local, and disk versions. Detect competing app instances and external changes before apply. Consider atomic replace with sibling temp files, durability barriers where supported, and platform-specific file sharing. Document guarantees for local and synced volumes.
- Keep essential content and assets in files. Use local databases for rebuildable indexes, caches, and appropriate session state. Define cache invalidation, index privacy, settings precedence, manifest discovery, and portable relative references.
- Each document handler owns format detection, parsing, serialisation, validation, capabilities, bounded selection reads, semantic operations, and diff presentation. Start with static registration. Define schema versions and forward compatibility. Unknown newer formats open safely with a read-only or source fallback. Migrations need preview, a checkpoint, and documented compatibility.
- Markdown baseline, accepted as the direction in [ADR-0001](decisions/ADR-0001-markdown-source-preservation.md), is a documented Offish Markdown profile: CommonMark with explicitly selected extensions, shared by the editor and by importers such as DOCX-to-Markdown. One parser is authoritative for source spans, so that the editor and Rust validation cannot disagree. Retain source spans or patch the smallest affected region. Define loss handling for raw HTML, front matter, comments, and unknown syntax before enabling visual saves. Compare structured sidecars and front matter for styles, page metadata, footnotes, cross-references, and stable links. Keep public identifiers where cross-document links need stability.
- Evaluate sparse YAML, YAML manifests with per-sheet shards, and CSV data regions. Keep formulas, types, styles, named ranges, dependencies, and structural edits round-trippable. Define blank vs empty string, date/time and locale behavior, error/cycle semantics, volatile functions, and reference rewriting after moves. Treat calculated values as derived with an explicit caching policy. A range read returns only authorised rows/columns, formula metadata, and selected dependency summaries.
- Compare Markdown slides for narrative decks, a documented structured model for positioned elements, and ODP as a working or exchange format. Compare readability, stable IDs, layout fidelity, editable round trips, and large deck changes. Start by preserving model data for features the editor cannot render.
- Store diagram source and structured data in open formats. Sanitize rendering of Mermaid, SVG, HTML and embedded assets, and resolve links with workspace policy. Define how bundles move or export with their relative assets.

### Change review and recovery

An agent receives read access and proposal tools within a defined scope. Each proposal records target paths, base hashes, semantic operations, provenance, and dependencies. Validate against format and permission rules before preview. Perform a second revision and validation check before commit to disk. A stale proposal requires a fresh preview after a safe rebase or an explicit conflict resolution. Acceptance creates a checkpoint distinct from undo and Git. [ADR-0002](decisions/ADR-0002-proposals-and-recovery.md) accepts this direction. The MVP keeps the latest 20 checkpoints per document, and editor undo does not cross an acceptance. Rollback compares current revisions before restoring so intervening user changes remain recoverable.

Compare typed patch overlays, snapshots, staging directories, and Git worktrees. Start evaluation with an isolated patch overlay plus journaled preimages and staged output. This also works in folders without Git. Multi-file replacement has per-file atomicity where supported. Across files, use a durable transaction log, ordered recoverable steps, validation of preimages, and visible recovery state. External tools and sync clients can observe intermediate writes. Test interrupted apply, rollback interrupted by another edit, depleted disk space, and permission errors. Git worktrees may help contain compatible external agents if isolation is enforced independently.

### AI integration and trust boundary

Use a provider-neutral adapter contract for capabilities, authentication, streamed events, context budgets, permissions, cancellation, sessions, retries, and proposal results. Evaluate ACP for editor-to-agent sessions and MCP for exposing scoped document tools. The first integration targeted for release is Codex through its app-server with ChatGPT sign-in, so existing Codex users can work with their plan and need no separate API key or billing. It ships only after the sign-in path is confirmed as supported for Offish and Offish demonstrably controls which files the agent sees and which changes reach disk. If Codex is blocked, evaluate Cursor sign-in next. Claude remains a trial candidate. API-key, direct API, local and OpenAI-compatible endpoints, Claude Code, Gemini CLI, OpenCode, and custom adapters are later-release work. Account for session capability negotiation, version pinning, tool errors, deadlines, and process cleanup. Associate retries with operation IDs to prevent duplicate writes. Define what leaves the machine before requests and preserve source attribution. Support disclosure controls for folders, selections, chat retention, and provider endpoints. Treat API keys, subscription login, and local access separately and confirm official provider terms for each.

Offish tools operate on a staged representation. External agents that need a subprocess run with least privilege and scoped capability grants. Provider or protocol prompts alone cannot enforce containment. Require a sandbox or mediated mode that constrains file, network, and process access before enabling direct-write adapters. Keep secrets out of context, records, and editable workspace metadata. Instructions from approved workspace files have bounded scope and cannot grant permissions. Imported documents, tool outputs, and retrieved text are data. Agent tool calls require policy checks even when the model claims user approval. Include injection, path traversal, indirect tool execution, and revoked/expired grants in verification.

### Interoperability, Git, and sync

The first PDF import extracts editable text in reading order. Use the embedded text where reliable and local OCR for scanned or image-only pages, including mixed PDFs. Preserve the source PDF and flag pages or regions where extraction is incomplete or reading order is uncertain. Offer optional AI review of the extraction against authorised PDF pages or rendered regions. Show proposed corrections with page references and require review before applying them. Flag unreadable content and unsupported reconstructions, including complex tables, rather than filling gaps with guesses. Basic extraction and OCR work offline.

Import yields a preserved original, a native derivative, and a conversion report with source hash, converter version, asset links, and unsupported features. The first Word import produces a portable, editable Markdown file with text, headings, lists, links, tables (using a documented Markdown table extension), and relative links to extracted images. Defer comments, tracked changes, and advanced layout. Evaluate documented Markdown tags or a sidecar for future enhancements, with ordinary Markdown readers still able to read the base document. Run converters with resource bounds and restricted permissions, including limits on expansion, external links, and embedded objects. Export writes a separate destination under an explicit overwrite policy. Track fidelity by feature with reference fixtures, visual comparisons, semantic extraction, and round-trip loss reports. Include font substitutions, recalculation differences, unsupported macros, and layout changes.

Git records intentional history. Sync replicates files and can propagate deletions, partial writes, or conflicting copies. Backup retains independently restorable versions. Test OS sync clients with offline changes, case conflicts, concurrent sync, partial assets, and restore drills. Keep internal journals and indexes out of shared or synced paths where feasible. Detect Git state and keep network operations explicit. Evaluate credentials, signing, filters, hooks, worktrees, submodules, and conflict handling before exposing each operation.

For cross-document propagation, use explicit references with stable identity, type checks, cycle detection, stale-state reporting, and previews for material changes. Package shared assets and themes without breaking portable relative links. Keep extension APIs capability-scoped and versioned. Multi-user editing requires separate decisions on concurrent operations, conflict semantics, presence, and collaboration trust.

## Roadmap and dependencies

| Phase | Depends on | Milestone and exit evidence |
| --- | --- | --- |
| 0 · Research and architecture | Product scope | Compare stacks and independent/reuse/fork strategies. Record selected UI, document contracts, save/recovery strategy, agent containment, and licensing decisions with reproducible evidence or bounded deferrals. |
| 1 · Core workspace | Phase 0 foundation decisions | Linux (Omarchy) shell with portable foundations, three panels, explorer, tabs, source editing, settings, file watching, safe persistence, basic search, test fixtures and CI. Demonstrate offline editing and recovery from external conflicts and interrupted saves. |
| 2 · AI-native document editor | Phase 1 plus Markdown, change and agent decisions | First useful release. Source-preserving live preview, a simple syntax-hiding mode made by extending that preview, @ mentions, selections, bounded workspace context, one approved adapter (Codex with ChatGPT sign-in unless blocked), inline visual review, granular acceptance, checkpoints, and rollback. Validate external edits during review and offline authoring. Release requires macOS and Windows evidence in addition to Linux. |
| 3 · Versioning and interoperability | Phase 2 plus converter and Git decisions | Git flows and document history, DOCX-to-Markdown import, PDF reading-order extraction with OCR and optional AI review, DOCX and PDF export, source preservation, styles/templates, and initial sync/backup strategy. Demonstrate reportable fidelity and independent recovery. |
| 4 · AI-native spreadsheets | Phase 2 document and change contracts, plus calculation and format gates | Typed grid, formulas, ranges, formatting/charts, structured tools, semantic review, XLSX exchange. Demonstrate recalculation agreement for the supported formula corpus and bounded range operations. |
| 5 · AI-native presentations | Phase 2 contracts and Phase 3 conversion patterns. Phase 4 chart integration can follow. | Visual slide editor with layouts/themes, semantic review, PPTX import/export and PDF export. Demonstrate fixture-based layout and content fidelity. |
| 6 · Unified workspace | Phases 3–5 and multi-file recovery gate | Shared assets, linked values/charts, dependency validation, advanced search, multi-file proposals, and consistency checks. Rehearse a forecast-to-plan-and-deck change with conflict and recovery fixtures. |
| 7 · Ecosystem and maturity | Stable contracts and evidence from preceding phases | Extensions, formats, adapters, advanced sync, accessibility and internationalisation breadth, performance tuning, installers/updates, and public contributor workflows. |

Explore spreadsheet and presentation feasibility early, then begin those delivery phases independently once their shared boundaries are stable. Establish keyboard and assistive technology baselines, CI, and development packaging in Phase 1. Shape the proposal transaction model in Phase 0 even though end-user multi-file acceptance arrives later.

## MVP boundary and success criteria

MVP ends at Phase 2. Its reference workflow is opening a folder, editing a Markdown document visually, asking an authorised agent a question or requesting a revision to the document or a selection, reviewing the proposal inline, accepting selected independent edits, and restoring a checkpoint. Users can complete offline authoring without an account. An agent may read several authorised files, while a review transaction targets one file.

Excluded from the initial MVP are spreadsheets and presentations, broad office format import/export, full Git workflows, sync connectors, multi-file acceptance, automatic acceptance, dynamic references, extensibility marketplace, real-time multi-user editing, and API-key and local-model adapters. The Phase 2 pilot includes a simple syntax-hiding mode, made by extending the CodeMirror 6 live preview. Advanced print layout, complete Markdown dialect coverage, and comprehensive accessibility and locale coverage are deferred. Basic keyboard accessibility and data integrity remain release gates.

Release evidence includes a full workflow demonstration on each target OS, fixture-based preservation of supported Markdown through visual and source edits, explicit handling of unsupported constructs, authoring with networking disabled, blocked out-of-scope context and writes, successful rejection without changing accepted bytes, and recovery after competing edits and interrupted writes. Report NFR measurement results with hardware and fixture definitions. Validate the workflow with the maintainer on Omarchy. The five-user target, four of them completing edit–review–restore without assistance, waits until more early adopters are available. Phase 0's user-success measure is the maintainer's walkthrough. Record where the maintainer loses confidence or needs source editing.

## Risks and unresolved decisions

| Risk or tension | Mitigation and decision gate |
| --- | --- |
| Native Rust UI preference versus editor maturity and accessibility | ADR-0003 accepts Tauri 2 and a CodeMirror 6 live preview, with bundled Chromium as the fallback if WebKitGTK fails. Measure IME, layout, performance and accessibility on Omarchy before the shell is called ready. |
| Single provider sign-in path for the first release | Verify Codex ChatGPT sign-in terms and containment before relying on it. Keep the adapter contract provider-neutral and evaluate Cursor sign-in if Codex is blocked. |
| Editable open files versus exact visual fidelity and stable IDs | Preserve source, quantify normalisation, choose a narrow supported dialect, and test local edits after external formatting. Gate visual save on round-trip evidence. |
| Single-file atomicity versus multi-file recovery and sync | Specify exact guarantees and test crash windows and competing edits. Defer multi-file apply until the transaction gate passes. |
| Cached formulas or cross-document values go stale | Specify dependency invalidation and visibility of stale states. Show recalculation errors explicitly. |
| YAML scale and numerical compatibility | Benchmark sparse and dense representations, track targeted diff and context sizes, and test numeric/date/reference semantics against fixtures. |
| Agent interchangeability versus provider entitlements and executable behavior | Verify supported authentication and supported capabilities per adapter. Make scoped mediation and cancellation entry criteria. |
| Conversion quality, fonts and redistribution | Pin converter builds, track binaries/fonts/models and terms, publish loss matrices, and isolate conversion sidecars. |
| Broad scope and upstream maintenance | Focus on Phase 2 workflow, retain replacement seams, and select dependencies with a fallback plan. |
| Personal data in chats, indexes, snapshots, and exports | Minimise retention, separate private state from portable content, and add deletion/export controls. Verify context disclosures and excluded files. |
| Untrusted plugins, imported content, and external source edits | Use format validation and capability-scoped adapters. Test malicious inputs and unsupported version handling. |

Phase 0 must validate UI, Markdown round trips, save/recovery, agent isolation, licensing, and format/engine feasibility. Offish uses the [MIT License](../LICENSE). Phase 0 still needs to verify distribution obligations for dependencies and bundled assets. [ADR-0005](decisions/ADR-0005-mediated-agent-integration.md) accepts mediated agent integration, with Codex through ChatGPT sign-in first and Cursor sign-in as the fallback. The documented Sign in with ChatGPT flow for open-source applications supports token sharing with Codex app-server. The Omarchy spike verified signed-in Codex streaming, cancellation, and containment. Implementing the OAuth client and equivalent macOS and Windows isolation remains Phase 2 work. The [Phase 0 evidence](research/phase-0/README.md#26-november-2026-omarchy-run) records the Omarchy reference hardware. The broader Linux support matrix remains open. On 2026-11-26 the maintainer assigned the simple syntax-hiding mode to the Phase 2 pilot, and deferred PDF reading order, OCR and AI-correction thresholds, and converter distribution to Phase 3, and formula corpus agreement, XLSX and PPTX round trips, and layout fidelity to Phases 4 and 5. Open design questions include workspace metadata conventions, initial template and export strategy, supported locale semantics for calculated values, chat/checkpoint retention, and future collaboration expectations. See the [Phase 0 plan](phases/00-research-and-architecture/PLAN.md) and [initial technology assessment](research/INITIAL_TECHNOLOGY_ASSESSMENT.md).
