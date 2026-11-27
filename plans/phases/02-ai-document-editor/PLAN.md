# Phase 2 — AI-native document editor

**Status** In progress · Proposal acceptance and recovery\
**Owner** Project maintainer  
**Master requirements** DOC-01, ED-01, WS-02, WS-03, AI-01, AI-02, CHG-01, NFR-01–06  
[Master plan](../../MASTER_PLAN.md) · [Phase 1 evidence](../01-core-workspace/PLAN.md) · [Phase 0 fixtures and research](../../research/phase-0/README.md)

## 1. Objectives

Deliver the first useful Offish release. A user opens a local folder, edits
Markdown visually, asks an authorised agent to revise a selection using chosen
references, reviews the proposed changes, accepts independent hunks, and restores
a checkpoint. The local authoring and review workflow also runs offline with a
deterministic fixture provider. Offline authoring requires no account.

## 2. Scope

- **DOC-01, ED-01, WS-02.** Keep source mode and add a source-preserving live
  preview for common formatting, links, lists, pipe tables, and workspace-relative
  images. Add a simple syntax-hiding mode with widget-based table-cell editing.
  Label unsupported ranges and offer source mode or read-only treatment.
- **AI-01, AI-02, WS-03, NFR-04.** Add a right-panel conversation with
  cancellable streaming, explicit selection and file or folder references,
  bounded context, and local conversation retention and deletion. Ship one
  validated adapter under ADR-0005.
- **CHG-01, NFR-03, NFR-05.** Stage and label single-document proposals,
  review inline by keyboard or pointer, accept or reject independent hunks,
  detect external changes, and support checkpoint restore.
- **NFR-01, NFR-02, NFR-06.** Verify supported behaviour on Linux, macOS,
  and Windows, record performance and interchange fixtures, and package the
  first release with audited dependencies and notices.

## 3. Out of scope

Multi-document acceptance is assigned to Phase 6 under CHG-02. Automatic
acceptance, further agent providers, and a public CLI or MCP surface are
outside this release. Git workflows, converters, DOCX and PDF interchange,
and sync and backup workflows are assigned to Phase 3. Spreadsheet and
presentation delivery remain in their roadmap phases.

## 4. Prerequisites and dependencies

Reuse the Phase 1 explorer, editor, safe save path, external revision checks,
and scoped local search. Native keyboard save, clean reload, AT-SPI focus,
and search have passed on Omarchy. Phase 1 is closed, with its
[handoff](../01-core-workspace/PLAN.md#phase-1-closeout-review) recording
the remaining checks. Direct paint and screen-reader speech carry into
Phase 2 native acceptance. Real IME verification is deferred until a
composing input method is available. Verify remote CI and finish the
distribution review before release. If a filesystem watcher is introduced,
verify rescan recovery after a dropped notification.
The maintainer scheduled abrupt power-loss testing for Phase 3. Under
ADR-0002, Phase 2 proposal review requires verified power-loss durability
before it can be called ready. If the test remains scheduled after Phase 2,
shipping proposal acceptance and rollback would require a separate
maintainer decision and a linked superseding ADR.

Use [ADR-0001](../../decisions/ADR-0001-markdown-source-preservation.md) and
[ADR-0003](../../decisions/ADR-0003-desktop-and-first-editor.md) for source
spans and the desktop/editor choice. Use
[ADR-0002](../../decisions/ADR-0002-proposals-and-recovery.md) for review and
checkpoint guarantees,
[ADR-0005](../../decisions/ADR-0005-mediated-agent-integration.md) for provider
boundaries, and [ADR-0004](../../decisions/ADR-0004-project-license.md) for
distribution and dependency review.

Before scheduling the cross-platform pilot, the maintainer identifies
macOS and Windows environments and owners and records their OS, WebView and assistive
technology versions. Release requires evidence on both platforms.

## 5. Technical design

Markdown files remain the authoritative bytes. The
[versioned Markdown profile](MARKDOWN_PROFILE_V1.md) defines the syntax,
coordinate mapping, and source edit batch. CodeMirror presents source and
preview. Rust validates the captured revision and expected bytes of each
operation through the shared workspace save path. Use one authoritative
parser for edits through document structure. The preview can use a
presentation parser for ordinary source edits.
Map UTF-16 editor selections to UTF-8 byte ranges across Unicode and line
endings. Preserve BOM, front matter, comments, raw HTML and opaque spans.
Open relative assets through the existing workspace boundary and verify
that unavailable assets have a readable fallback.

An immutable proposal targets one document and records its base revision and
typed hunks. Acceptance revalidates expected bytes, scope and dependencies
before saving. Rebase and validate remaining independent hunks after partial
acceptance. If an external edit changes the revision, refresh the review
against current content before accepting. Journal preimage and expected
postimage privately before replacement. Follow ADR-0002 for recovery,
20 retained checkpoints per document, revision-checked restore, and the
undo boundary at acceptance.

Define a provider-neutral adapter and deterministic fixture provider first.
Offish approves scope and resolves each context item through validated
workspace handles. The adapter receives a bounded snapshot and returns
candidate operations. Offish controls review and writes. Cancellation
revokes the grant and discards late output. Keep credentials in OS stores
and apply local retention and deletion to chat and checkpoints. Test one
pinned Codex app-server integration and its Sign in with ChatGPT flow under
ADR-0005. Evaluate Cursor sign-in only if the accepted fallback conditions
are met.

## 6. Implementation tasks

| Task | Requirements | Depends on | Owner | Result |
| --- | --- | --- | --- | --- |
| P2-01 · Source profile and span contract | DOC-01, WS-02, NFR-03, NFR-06 | Phase 1 save path, ADR-0001 | Workspace and editor | Version the supported Markdown profile and prove UTF-16 to UTF-8 operations and exact replay on the pilot fixtures. |
| P2-02 · Offline live preview | DOC-01, ED-01, NFR-05 | P2-01 | Editor | Render headings, emphasis, links, nested lists, tables and scoped images. Edit representative text and reveal syntax near the caret. |
| P2-03 · Simple syntax-hiding mode | DOC-01, ED-01, NFR-03, NFR-05 | P2-02 | Editor | Edit supported table cells in place with shared span operations. Provide clear source access for unsupported constructs. |
| P2-04 · Proposal acceptance and recovery | CHG-01, WS-02, NFR-03, NFR-04 | P2-01, Phase 1 recovery evidence | Workspace | Validate typed hunks, rebase independent remaining hunks, and exercise durable acceptance and restore with crash and external-edit fixtures. |
| P2-05 · Accessible inline review | CHG-01, ED-01, NFR-05 | P2-02, P2-04 | Desktop and editor | Review deterministic proposals, label additions and removals, accept and reject by keyboard, and restore a checkpoint. |
| P2-06 · Bounded context and conversation | AI-01, AI-02, WS-03, NFR-04 | Phase 1 scope boundaries, P2-04 | Desktop and agent integration | Test streaming, selection and @ references, approvals, cancellation, retention and redaction with the fixture provider. |
| P2-07 · Codex adapter and sign-in | AI-01, AI-02, NFR-01, NFR-04, NFR-06 | P2-06, ADR-0005 | Agent integration | Validate the pinned protocol, sign-in, context mediation, cancellation and containment on all release platforms. |
| P2-08 · Native release acceptance | NFR-01–06 | P2-01–P2-07, Phase 1 handoff | Desktop and release | Complete the release matrix, offline authoring, recovery, accessibility and performance tests, notices, and distribution checks. |

## 7. Testing strategy

Reuse disposable copies of the pilot fixture and build byte-level checks
for nested blocks, tables, repeated paragraphs, CRLF, BOM, multilingual text,
and opaque spans. Test selection mapping and undo in the frontend. Use Rust
fixture tests for proposal validation, independent hunk acceptance, filesystem
containment, recovery journals and interrupted restore. Cover dirty drafts
and concurrent external edits with separate processes.

Run the complete local workflow with networking disabled and the fixture
provider. Test the live provider separately after containment passes.
Record exact platform, WebView, provider, fixture and build versions.
Measure on native WebViews, including the 50,000-word document, and record
p50, p95 and memory. Check keyboard navigation and focus, speech via Orca,
VoiceOver and NVDA on their respective platforms, and real IME commit
followed by save and reopen on a suitable setup.

## 8. Acceptance criteria

| Test | Requirements | Pass criterion |
| --- | --- | --- |
| P2-T01 · Source fidelity | DOC-01, WS-02, NFR-03 | A no-op preserves the complete original file. Expected edits touch only intended bytes across the fixture matrix. Opaque spans survive. |
| P2-T02 · Visual editing | DOC-01, ED-01, NFR-02, NFR-05 | Edit text and pipe-table cells in the two visual modes. Scoped images render. Unsupported ranges have a visible fallback. Record native keystroke-to-paint p95 against 50 ms on the 50,000-word document. |
| P2-T03 · Review and recovery | CHG-01, NFR-03, NFR-05 | Keyboard and pointer can review labelled additions and removals, accept an independent hunk, reject others and restore an eligible checkpoint. Stale revisions refuse acceptance or restore. Restart preserves the specified journal and conflict states. |
| P2-T04 · Context and cancellation | AI-01, AI-02, NFR-04 | Granted selections and references stream through the host. An out-of-scope canary stays unread, a direct write is refused, and cancel prevents late output, access and writes. Logs contain no tokens or private content. |
| P2-T05 · Release workflow | NFR-01, NFR-05, NFR-06 | Omarchy, macOS and Windows run the complete workflow with native keyboard and accessibility evidence. Record IME evidence or a specific maintainer decision on any release exception. Offline editing remains usable throughout. |
| P2-T06 · Distribution | NFR-01, NFR-06 | Produce reproducible release builds with audited dependencies and notices, verified installation and updates, and a documented compatibility and support scope. |

## 9. Risks and open questions

| Risk or decision | Owner | Evidence or decision needed |
| --- | --- | --- |
| Source-span ambiguity or limited visual coverage | Editor | Publish the versioned profile and fixture matrix before treating visual saves as ready. Revisit ADR-0001 or ADR-0003 if the editor cannot meet fidelity or native interaction criteria. |
| Late external changes and interrupted acceptance | Workspace | Before calling proposal review ready, resolve the power-loss gate in ADR-0002. Verify same-file coordination, metadata, symlinks, and replacement behaviour on macOS and Windows before release. |
| Cross-platform file and provider containment | Desktop and agent integration | Identify the CI and native test environments early. Verify the supported sandbox on each platform before enabling the live adapter there. |
| Provider terms or protocol changes | Agent integration | Recheck first-party documentation, pin the tested version and apply ADR-0005 fallback conditions if blocked. |
| Chat and context retention defaults | Maintainer | Agree on local conversation retention and deletion before shipping. Use the 20-checkpoint policy from ADR-0002. |

## 10. Deliverables and definition of done

Deliver the tested editor modes, versioned Markdown profile and fixtures,
private checkpoints and bounded agent integration. Publish the platform
support and limitations, accepted architecture records, CI and native
evidence, dependency notices, and release documentation. The maintainer
reviews P2-T01–T06 and the remaining Phase 1 evidence before release.

## 11. Progress tracking

| Tasks | Status | Evidence or next check |
| --- | --- | --- |
| P2-01 | Complete | Profile v1 is documented. UTF-16 and absolute byte ranges round-trip, and revision-bound edits use the existing workspace save path. |
| P2-02 | Complete | The maintainer verified live preview, table source reveal, and independent scrolling on Omarchy. Native paint timing remains open under P2-T02. |
| P2-03 | Complete | The maintainer confirmed the Omarchy check and pushed the changes. |
| P2-04 | In progress | The single-document workspace backend and restart fixtures pass. Verify checkpoint behavior across file moves and the outstanding native durability and platform gates. |
| P2-05 | Planned | Connect deterministic proposals, review controls, and the editor undo boundary to the workspace backend. |
| P2-06–P2-07 | Planned | Start with the deterministic provider and a negative scope fixture. The shipping sign-in and cross-platform sandbox checks remain open. |
| P2-08 | Planned | Omarchy reference hardware is recorded. Identify macOS and Windows environments and owners and complete the checks carried over at Phase 1 closeout. |

### P2-01 evidence

On 27 November 2026, the locked desktop build, 28 Rust tests, 26 Rust
tests with desktop features disabled, and all five JavaScript suites passed.
The source fixtures covered LF, CRLF, BOM, supplementary Unicode characters,
leading front matter, nested markup and repeated text. The workspace test
verified a byte-identical no-op, one targeted save, and refusal of a stale
revision after an external edit. The CodeMirror test confirmed the editor
positions used by the Rust mapping. The Markdown parser integration and
native live preview were assigned to P2-02.

### P2-02 evidence

The source-first preview uses the MIT-licensed Lezer Markdown parser
(`@lezer/markdown` 1.8.2) and CodeMirror's existing language integration.
The parser is pinned in the npm lockfile. It interprets source for
presentation. Edits go through CodeMirror and replay as revision-checked
byte splices through the existing save path. Tables render as widgets
outside the selection and reveal their Markdown source when entered.

The locked desktop build, 29 Rust tests and six JavaScript suites passed.
Checks covered block and inline decorations, leading metadata, selection
in tables, the source toggle, byte replay and workspace image confinement.
A mocked desktop bridge confirmed that a preview edit saves using the
revision-bound batch with BOM and CRLF preserved. The maintainer confirmed
the native preview worked on Omarchy and reported that long documents
could scroll the entire window. The workspace row is now constrained to
the available viewport height. The editor and Files tree scroll
independently and their headers stay in place. The maintainer finished
the native table-preview and scroll checks. Native paint timing remains
part of P2-T02.

### P2-03 evidence

Simple keeps supported Markdown markers hidden and edits plain-text cells
in the [profile v1 table subset](MARKDOWN_PROFILE_V1.md). Rust validates
the target against the captured source before returning the editor edit.
Source remains available for the full Markdown document through the same
mode selector.

The locked development build, 32 Rust tests and six JavaScript suites
passed. They cover hidden markers on the active line, keyboard interaction
in table cells, empty cells, rejected structural edits, consecutive line
breaks, and exact byte preservation with LF, CRLF, BOM and Unicode.
A mocked desktop bridge also exercised validation, undo, redo,
revision-bound save and switching to Source on the same edited document.
The cell navigation regression checks cover leaving untouched cells,
applying valid edits before opening the next cell, and keeping rejected
edits available for correction. The widget test checks focus after the
table is redrawn.
The table preview suppresses WebKit's context menu on rendered cells.
The JavaScript test covers a secondary click, primary-click source reveal,
and the active cell editor's context menu.

A header-only startup was traced to an unreadable editor asset in the
debug executable. A rebuild restored the asset. The build now watches the
frontend directory and rejects a missing or empty bundle, and a desktop
test checks that Tauri can read the exact bundled editor bytes.

For the Omarchy check, open `brief.md` in a disposable pilot copy. In
Simple mode, click through several cells, then edit the Budget cell and
use Shift+Enter twice between two lines of text. Click another cell,
confirm the blank line renders, then undo, redo, and save. In Source mode,
confirm the cell contains two `<br>` tags and that surrounding pipes and
spacing remain as before. The automated session could build the app but
GTK initialization prevented the native launch.

The maintainer later confirmed the native check and pushed the changes.

### P2-04 first implementation slice

Under CHG-01, WS-02, NFR-03 and NFR-04, validate one Markdown proposal
against the captured revision and apply explicitly selected, independent
hunks. Rebase the remaining byte spans against the committed revision.
Use the workspace's locked, metadata-preserving replacement path and a
private checkpoint journal. Exercise partial acceptance, restore, and
the preimage, postimage and external-conflict recovery branches for
P2-T01 and P2-T03. The internal journal representation is provisional
pending native durability and platform evidence under ADR-0002.

The Linux workspace backend now applies selected hunks under the same
directory lock and metadata-preserving replacement path used for saves.
It checks the proposal revision, rebases and revalidates the remaining
hunks, and retains up to 20 private preimages per document. Restore
requires the latest revision and an intact history chain. An interrupted
replacement is reconciled against the captured preimage and expected
postimage revisions. If the disk matches neither, Offish preserves the
disk and private preimage. Explicitly keeping the disk revision retains
the preimage for inspection and breaks the restore chain.

The locked Rust suite passed 40 tests. The P2-T01 and P2-T03 fixtures
check byte-exact restore with BOM, CRLF and Unicode, rebase, competing
acceptances, history retention and deletion, and subprocess exit before
and after both acceptance and restore. The native power-loss and
cross-platform gates remain open. File moves currently change the
path-based history lookup. Verify how history follows a file before
exposing review in the editor. P2-05 will connect the backend to the
review UI and seal the editor undo history on acceptance.
