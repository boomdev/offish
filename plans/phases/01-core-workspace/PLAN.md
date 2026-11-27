# Phase 1 — Core workspace

Status: In progress. Phase 0 closed on 27 November 2026.\
Owner: Project maintainer. Task roles below name the accountable workstream.  
Master requirements: WS-01–03, ED-01, DOC-01, NFR-01–06.  
[Master plan](../../MASTER_PLAN.md) · [Phase 0 closeout](../00-research-and-architecture/PLAN.md#12-phase-0-closeout) · [Phase 0 evidence](../../research/phase-0/README.md)

## 1. Objectives

First milestone: open a local folder on Omarchy, edit one Markdown file in source mode, and save it offline. If another editor saves first, Offish must preserve the on-disk version and the unsaved draft. This milestone is complete when P1-T01–T03 pass.

Phase 1 exit adds restart recovery, folder operations, local search, keyboard and accessibility evidence, reproducible builds and CI.

## 2. Scope

- Three panels, keyboard navigation, an explorer, editor tabs, and local settings. The right panel gives workspace context and file status. (WS-01, ED-01, NFR-05)
- Source editing, save and reload, file watching, conflict resolution, and recovery on ordinary local folders. (WS-02, DOC-01, NFR-03)
- Rebuildable local search with visible scope and freshness. (WS-03, NFR-02)
- Omarchy acceptance, portability boundaries, fixture-based CI, and a dependency and license inventory. (NFR-01, NFR-04, NFR-06)

## 3. Out of scope

Phase 2 owns visual Markdown saves, simple syntax hiding, agent integration, proposal acceptance and checkpoints. Phase 3 owns full Git, converter and sync workflows. Later format work follows the master plan.

## 4. Prerequisites and dependencies

Phase 0 is closed with approved transfers D0-01 through D0-04. Reuse the licensed [pilot fixtures](../../research/phase-0/fixtures/pilot/) and the recorded Omarchy desktop baseline. Identify macOS and Windows verification environments before Phase 2 planning.

The accepted [desktop ADR](../../decisions/ADR-0003-desktop-and-first-editor.md) selects Tauri and CodeMirror. The [Markdown ADR](../../decisions/ADR-0001-markdown-source-preservation.md) makes source bytes authoritative. The [proposal ADR](../../decisions/ADR-0002-proposals-and-recovery.md) informs revision tracking and recovery. Development packaging follows the [license ADR](../../decisions/ADR-0004-project-license.md).

## 5. Technical design

Start with a Cargo workspace and a desktop application under `apps/desktop/`. Keep Rust workspace operations in focused modules. Extract a crate when a second caller needs the boundary. The UI sends typed requests through a narrow bridge. Rust owns authorized roots, path validation, revisions, persistence and indexing. Keep filesystem and watcher behavior behind interfaces with Linux tests first.

Carry base revision, draft and observed disk revision separately. A clean buffer can reload after an external save. A dirty buffer keeps its draft and exposes both versions for explicit resolution. A watcher signals work, while a rescan restores freshness after missed events. Saving checks the base revision, stages data on the destination filesystem, applies the required metadata policy, syncs staged data, replaces the file and syncs the parent directory. Document the race boundaries for external applications and retain recoverable copies where outcomes are ambiguous.

Preserve untouched Markdown bytes, including newline conventions and a BOM. Give invalid or unsupported input an explicit read-only state. Store private recovery and index data outside the opened folder. Pin and audit dependencies before packaging.

The first search slice uses the in-memory index described in
[ADR-0006](../../decisions/ADR-0006-phase-1-search-index.md).

## 6. Implementation tasks

| Task | Requirements | Depends on | Owner | Result |
| --- | --- | --- | --- | --- |
| P1-01 · Shell and folder opening | WS-01, ED-01, NFR-01, NFR-04, NFR-06 | Phase 0 exit (met) | Desktop | Run the locked build on Omarchy. Open a user-selected folder offline and browse it in the three-panel shell. |
| P1-02 · Source tabs and safe save | WS-02, ED-01, DOC-01, NFR-03 | P1-01 | Workspace and desktop | Open one Markdown file, edit, save and reopen. Preserve bytes outside the edit and report errors with the draft intact. |
| P1-03 · External edit handling | WS-02, NFR-03 | P1-02 | Workspace | Reload clean buffers and present both versions for dirty conflicts. Check revisions at save, coordinate Offish writers and recover watcher freshness. |
| P1-04 · Recovery and metadata | WS-02, NFR-03, NFR-04 | P1-02, P1-03 | Workspace | Verify interrupted saves, restart recovery, metadata preservation, and the local filesystem durability policy. |
| P1-05 · Explorer and settings | WS-01, WS-02, ED-01 | P1-03 | Desktop and workspace | Create, rename, move, duplicate, and recoverably delete entries where supported. Show dirty and read-only state and apply local workspace settings. |
| P1-06 · Local search | WS-03, NFR-02, NFR-04 | P1-03 | Workspace | Index authorized files, respect exclusions, refresh after changes, and rebuild the index. |
| P1-07 · Native editor acceptance | ED-01, DOC-01, NFR-01, NFR-02, NFR-05 | P1-02 | Desktop | Exercise keyboard navigation, real IME, Linux assistive technology and direct paint timing in WebKitGTK. Investigate any failure before exit. |
| P1-08 · CI, packaging and handoff | NFR-01, NFR-06 | P1-01 to P1-07 | Maintainer | Pin Rust and frontend builds, run fixture checks in CI, audit transitive terms and notices, and produce the development build and Phase 2 evidence checklist. |

## 7. Testing strategy

Automate Rust filesystem and index checks on disposable folders. Use process-level tests for simultaneous saves and interrupted recovery. Verify the normal desktop flow on Omarchy with networking disabled. Use fixture copies and record the filesystem, runtime, hardware and scale. Capture actual IME and assistive technology observations. Record repeated cold and warm measurements with p50, p95 and peak memory where applicable.

## 8. Acceptance criteria

| Test | Requirements | Pass criterion |
| --- | --- | --- |
| P1-T01 | WS-01, ED-01, NFR-01, NFR-04 | On native Wayland, open a local folder and Markdown tab offline. Reject reads outside the authorized folder and explain inaccessible entries. |
| P1-T02 | WS-02, DOC-01, NFR-03 | A no-op save is byte-identical. A bounded edit changes only the intended bytes on LF, CRLF, BOM and mixed Unicode fixtures. A failed save retains the draft. |
| P1-T03 | WS-02, NFR-03 | A clean tab reloads, a dirty tab keeps both versions, a stale save is refused, and two Offish writers do not silently overwrite one another. A forced missed watcher event is reconciled by a rescan. |
| P1-T04 | WS-02, NFR-03, NFR-04 | Process exits around replacement, disk-full and permission failures, symlink and identity changes, and supported mode, owner, ACL and extended-attribute cases preserve or recover the expected data. Validate power-loss behavior on a disposable filesystem or VM and document the proven guarantee. |
| P1-T05 | WS-01, WS-02, ED-01 | Explorer mutations and reopened settings preserve intended file identity and contents. Destructive operations have a recoverable path where supported and explain limits otherwise. |
| P1-T06 | WS-03, NFR-02, NFR-04 | Exclusions and authorization hold after external edits and index rebuild. On the reference hardware, record warm search p95 against 300 ms and cold explorer startup on 10,000 files against 3 s. |
| P1-T06a | WS-03, ED-01, NFR-04 | Search saved UTF-8 text across available roots. Show root, file, line, scope and scan freshness. Jump to the match without changing an unsaved draft. |
| P1-T06b | WS-03, NFR-02, NFR-04 | Revalidate the bounded in-memory index on every search, provide a rebuild action, and verify exclusions, external edits, deletes, and link containment. Measure before adopting persistent storage. |
| P1-T07 | ED-01, DOC-01, NFR-01, NFR-02, NFR-05 | Complete the source flow by keyboard, record AT-SPI behavior, commit real IME text and reload it intact, and measure keystroke-to-paint p95 against 50 ms on 50,000 words in WebKitGTK. Record any required change to the shell or target for explicit review. |
| P1-T05a | WS-01, WS-02, ED-01 | Create and rename files and folders within the workspace. Reject invalid paths and collisions without touching existing entries. An open tab and its unsaved draft follow a rename. |
| P1-T05b | WS-01, WS-02, ED-01 | Move a file or folder within the workspace, retaining open drafts. Duplicate saved files and folder trees independently. Reject collisions and unsupported entries before publishing a copy. |
| P1-T05c | WS-01, WS-02, NFR-03, NFR-04 | Move to private Trash and restore after reopening the workspace. A collision at the original path leaves both entries intact. If the filesystem cannot move the entry to Trash, leave it in place. |
| P1-T05d | WS-01, WS-02, ED-01, NFR-04 | Add multiple ordered roots from File or create a workspace from one or more folders. Preserve drafts when adding roots. Persist the workspace and restore it on normal launch. With no recent workspace, show Create Workspace above any saved tiles. Center the button by itself when the catalog is empty. Keep access scoped to registered roots. |
| P1-T07a | ED-01, NFR-05 | The caret is visible in light and dark themes. Resize each side pane with mouse and arrow keys while keeping the editor usable. Close active and background tabs, with confirmation before discarding a dirty draft. |
| P1-T07b | WS-01, ED-01, NFR-05 | Expand and collapse the Files tree, open row actions with the three-dot button, switch and close document tabs, and operate the File and Edit menus by pointer and keyboard. With no open tabs, hide the strip and center the prompt in the editor pane. Verify commands preserve workspace scope and unsaved drafts. |
| P1-T07c | WS-01, WS-02, NFR-05 | On launch with a workspace and no open document, focus the Files tree. Navigate visible tree rows with arrow keys. Enter opens the selected document. F2 opens rename. Delete confirms a move to Trash. Shift+Delete separately confirms permanent deletion. Open drafts retain their existing protections. |
| P1-T07d | ED-01, NFR-05 | Alt+Left/Right moves through panes and dividers. Files reveals the active document. The editor restores its caret. Details focuses its first available button. Alt+Up focuses File, and Alt+Down returns to the last workspace control. Left/Right moves between dialog buttons. Ctrl+W closes the focused editor pane’s active tab, including files shown with a read-only message, with the existing draft protection. Ctrl+Tab and Ctrl+Shift+Tab cycle through open tabs in display order and wrap in both directions. Rapid presses keep focus in the editor pane. |
| P1-T07e | WS-01, WS-02, NFR-05 | Show folder creation actions on hover and keyboard focus, and create entries in the chosen folder. Keep Trash visible while the tree scrolls. Rename the workspace without disturbing its roots or open drafts, and keep the new name across relaunch and in the workspace chooser. |
| P1-T08 | NFR-01, NFR-06 | Locked builds and fixture checks pass in CI. The development package and notices match the audited dependency set. Record Windows and macOS environments and owners for Phase 2. |

## 9. Risks and open questions

| Risk | Owner | Resolution |
| --- | --- | --- |
| External writers can race the final revision check | Workspace | Define and test the supported guarantee and conflict behavior in P1-T03 and P1-T04. |
| File replacement changes metadata or loses state after a power cut | Workspace | Resolve on Omarchy before Phase 1 exit. Record portability limits for Phase 2. |
| Omarchy composition or paint behavior fails the editor target | Desktop | Resolve at P1-T07. Reopen the desktop ADR if essential criteria fail. |
| Search or packaging needs a new dependency | Workspace or maintainer | Compare maintenance, license and portability before adopting it. Record the audit at P1-T08. |

## 10. Deliverables and definition of done

A runnable Omarchy development build, locked dependencies, automated tests and CI, native desktop evidence, documented save and recovery guarantees, and a Phase 2 handoff. The maintainer reviews P1-T01–T08 and any proposed exceptions before Phase 1 exit.

## 11. Progress tracking

| Task | Status | Evidence or remaining work |
| --- | --- | --- |
| P1-01 | Implemented, native check open | Linux desktop build, folder picker, explorer and three panels. Backend tests pass for scoped reads. The maintainer reported the native Omarchy smoke test worked. The detailed P1-T01 checks remain open. |
| P1-02 | Implemented, native check open | CodeMirror source tabs and Rust saves. Btrfs tests pass for no-op saves, localized LF and CRLF/BOM edits, mode bits and invalid input. The maintainer reported a successful native smoke test. Retain the fixture matrix for formal acceptance. |
| P1-03 | In progress | Open buffers rescan disk periodically. Backend tests pass for stale writes and competing Offish handles. The maintainer confirmed both native conflict resolution actions. Verify clean reload separately. |
| P1-04 | In progress | Btrfs process-exit tests pass before and after replacement. Saves preserve ownership, group, mode, POSIX access ACLs and user attributes in the tested cases. Injected disk-full and permission errors preserve accepted bytes. Actual filesystem fault and power-loss evidence remain open. |
| P1-05 | Implemented, native smoke passed | The maintainer verified create and rename (P1-T05a) and confirmed move, duplicate, and recoverable Trash in the native app (P1-T05b–T05c). The maintainer also confirmed that the Omarchy workspace smoke test passed (P1-T05d and workspace naming in P1-T07e). |
| P1-06 | Implemented, native smoke passed | The maintainer confirmed native search works. Multi-root search, exclusions, revalidation, and index rebuild pass automated checks. Native search navigation, explorer startup, and performance acceptance remain open. |
| P1-07 | In progress | P1-T07b implements the tree, tabs, and application menus. Native verification of remaining shortcuts, IME, assistive technology, and editor performance is open. |
| P1-08 | In progress | Rust and npm lockfiles, local build instructions and Linux CI workflow are present. Remote CI, audit, notices, packaging and cross-platform readiness remain open. |

27 November 2026, Omarchy Btrfs. The locked Rust build and six backend tests passed. The pinned npm
dependencies installed from the local cache and the editor bundle built.
The Wayland launch stopped at GTK initialization in the command sandbox.
Native desktop acceptance remains open.

The maintainer reported a successful native smoke test on Omarchy and
identified caret contrast, pane resizing, and tab closing as usability
follow-ups. P1-T07a covers the follow-up checks.

### P1-T03 and P1-T04 follow-up

- The maintainer confirmed that **Use disk version** and **Keep draft and
  save** both worked in the native Omarchy app.
- On Omarchy Btrfs, a child process exited after journaling and after pathname
  replacement. Reopening recovered the draft in the first case and
  recognized the committed bytes in the second. An external edit after the
  first exit stayed on disk until the draft was explicitly saved.
- Preserved the tested UID, GID, mode, POSIX access ACL and user extended
  attribute across replacement. A separate save into a directory with a
  default ACL left the original file's no-ACL policy intact.
- Injected ENOSPC before writing and EACCES after journaling left the
  original file intact. The latter retained the durable draft.
- A process exit leaves the kernel running. Abrupt power loss still needs a
  disposable VM or equivalent storage test. The session has no QEMU or KVM
  available. Non-cooperating writers retain the final check-to-rename race
  documented in ADR-0002.


### P1-T05a explorer follow-up

The Omarchy Btrfs tests verify creation, rejection of invalid names and
links, no replacement of existing entries, and saving against the original
revision after both file and enclosing-folder renames. An unresolved save
recovery draft blocks a file rename. Frontend tests verify that tab remapping
retains the draft and revision, leaves unrelated tabs alone, and refuses a
destination already open in another tab. The locked desktop build and
frontend bundle passed. The maintainer confirmed the native walkthrough.


### P1-T05b–T05c explorer follow-up

- Move preserves the file revision and keeps open drafts attached to the
  same tab. The backend takes directory locks in stable order and requests
  a no-replacement rename. A destination conflict, invalid path, or move
  into a descendant leaves the source in place.
- Duplicate copies saved bytes into a private staging directory and publishes
  only after the copy succeeds. Linked and special entries in the source
  tree cause duplication to fail. A copied file has its own identity.
- Trash records the original path before moving the entry into private
  recovery storage outside the workspace. Restore works after reopening
  the workspace and refuses to overwrite. Cross-volume Trash and restore
  require filesystem support for atomic rename. An unsupported rename fails
  with the source intact. Trash has no permanent deletion control yet.
- On Omarchy Btrfs, all 13 Rust tests passed, including file and folder
  restore after reopening and collision preservation. Frontend remapping
  tests, editor bundle and locked desktop build passed. Native UI
  verification remains open for the three new actions.


### P1-T05d multi-folder workspaces

Offish saves the ordered root list and last workspace in its local application
configuration. **File → Close Workspace** clears the last selection and
returns to saved workspaces. **File → Exit** retains the last selection.

On 2026-11-27 the locked desktop build and Rust and JavaScript suites passed.
The tests cover restoring the catalog, clearing the last selection while
retaining saved entries, root order and deduplication, same-named files under
different roots, and unavailable roots. A native launch in the coding sandbox
failed during GTK initialization.

A frontend smoke test with a mocked Tauri bridge confirmed the creation
button in both chooser states and confirmed that a restored workspace
skips the chooser.

The maintainer confirmed the native Omarchy workspace smoke test passed.
The smoke test covered creation, rename, and restore after relaunch.

To repeat the check, start with an empty Offish configuration and verify
the centered creation button and multiple folder selection. Open a file and type without
saving, then add a folder and check that its root appears below the existing
roots while the draft remains open. Save, exit, and relaunch without a folder
argument. Confirm the same roots reappear in order. Choose **File → Close
Workspace**, check that the saved tiles appear below **Create Workspace**,
and reopen the workspace from its tile.

### P1-T06 local workspace search

**Edit → Find in Workspace** and Ctrl+Shift+F search saved UTF-8 text in
the available roots. Results show the root, relative path, and line number.
Selecting a result opens its tab and highlights the match if it still
aligns with the open text. Search focuses the first result, Up and Down
move between results, and Enter opens the focused result. Ctrl+Shift+F
returns focus to the query while the dialog is open. **Rebuild Index**
discards cached text and scans the roots again.

The locked build and Rust and frontend suites passed. Rust tests cover
ordered multi-root matches, nested ignore rules, private paths, symlinks,
invalid input, external edits and deletion, and rebuild. Frontend tests
verify UTF-16 coordinates and the fallback when a draft has moved a
match. A mocked-bridge check verified search dialog invocation, result
display, and opening a result. It also verified first-result focus, Up and
Down at both ends of the list, and query focus when there are no matches.

On 27 November 2026, a release backend run on Btrfs and an Intel Core
i5-1135G7 searched 10,000 files of roughly 1 KiB each. An empty index
scanned in about 77 ms. Across 20 alternating warm queries with zero and
100 matches, p50 was 30 ms and p95 was 45 ms. The OS page cache was warm
from fixture creation. These are backend timings. They exclude GTK,
bridge and rendering latency.

Run the timing fixture with
`TMPDIR="$PWD" cargo test --locked --no-default-features --release search_timing_fixture -- --ignored --nocapture`.
GTK initialization blocked the sandbox's native run. The maintainer
confirmed search works on Omarchy and that Ctrl+Shift+F returns focus to
the query from the results. Native Up/Down navigation, end-to-end timing,
peak memory, and cold explorer startup measurement remain open.

### P1-T07b workspace UX

The Files pane expands folders in place and uses one floating menu for each
row’s actions. Tabs carry an active indicator, unsaved marker and close
button. The File and Edit menus use the normal workspace operations.
Opening an outside file requires a workspace switch and refuses to discard
unsaved tabs. Clipboard commands use GTK plain text, and tab switching keeps
the editor’s undo history.

Fourteen Rust tests passed on Btrfs, including picker confinement and link
rejection. Frontend tests cover popup placement, tab remapping and pane
sizing. The editor bundle and locked desktop build passed. Native pointer,
keyboard, picker and clipboard interaction remain to be verified.

A mocked-bridge frontend check verified that the strip starts empty, gains
a tab when the first document opens, and becomes empty again when the last
tab closes. The empty-state CSS hides the strip and centers the editor prompt.


### P1-T07c keyboard follow-up

The Files pane starts at 260 px and still resizes within the available
window width. Arrow navigation follows visible rows and skips unsupported
entries. Enter opens, F2 renames, and Shift+F10 opens row actions. Delete
confirms a move to Trash. Shift+Delete confirms a separate permanent
deletion and requires the affected tabs to be closed. The backend uses
the workspace’s private Trash as its removal stage. If it cannot move
an entry there, the entry stays in place.

Fifteen Rust tests and the frontend tests passed. These include keyboard
command selection, permanent removal with no restore point, and preserving
a linked directory outside the workspace. Native key delivery and focus
behavior remain to be checked on Omarchy.

A mocked-bridge frontend check verified that launch focuses the first
available Files row and that Up and Down work immediately. It also verified
that focus falls back to the pane when all roots are unavailable.


### P1-T07d pane navigation

Alt+Left and Alt+Right step across Files, its divider, the source editor,
the Details divider, and Details. Files focuses the active document and
expands any collapsed ancestors. Editor focus leaves the current selection
intact. Details takes focus only when it has an enabled, visible action.
Alt+Up focuses File and Alt+Down returns to the latest workspace focus stop.

Frontend tests cover navigation order and ancestor paths. The editor bundle
and locked desktop build passed. Native Omarchy shortcut delivery and
caret restoration still need a smoke test.

A mocked-bridge frontend check verified that a read-only message takes
focus in the editor pane and Ctrl+W closes its tab. The check also covered
key repeat and focus in the Files pane.

The GTK window handles Ctrl+Tab and Ctrl+Shift+Tab before focus traversal.
Rust tests verified the forward and shifted key mappings. A mocked-bridge
frontend check sent 30 consecutive requests and verified wraparound and
editor focus. The maintainer confirmed both shortcuts worked on Omarchy
after retesting the rapid-switching and reverse-cycling fix.

### P1-T07e folder actions and workspace naming

The folder row actions create files and folders in the chosen root or
subfolder. The tree scrolls independently of its Trash footer. Workspace
rename saves a display name in the existing catalog without switching
workspaces or reopening documents.

On 2026-11-27 the editor bundle, locked desktop build, 18 Rust tests,
and JavaScript suites passed. The new tests cover Enter on a focused
row action and persisting the name through reopening and adding roots.
GTK initialization blocked the coding sandbox’s native smoke test. On
Omarchy, the maintainer confirmed the workspace naming flow as part of the
workspace smoke test. To repeat the sidebar checks, try the actions on a
hovered root and a keyboard-focused subfolder, scroll the tree with Trash
in view, and rename the workspace
using the pencil at the right edge of the Files pane while a draft is open.
Confirm the name survives relaunch and is shown in the saved workspace chooser.
