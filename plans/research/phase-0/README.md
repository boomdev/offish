# Phase 0 research evidence

26 November 2026 · [Phase plan](../../phases/00-research-and-architecture/PLAN.md) · [Decisions](../../decisions/)

## Pilot workflow and pass criteria

Use the synthetic [launch brief](fixtures/pilot/brief.md) and [revised assumptions](fixtures/pilot/assumptions.md). The maintainer uses Omarchy for the first hands-on run. Rehearse opening an ordinary folder offline, editing a selection visually, using the assumptions as authorised context, reviewing a proposed date change, accepting or rejecting it, and restoring. Repeat with an external save while a proposal is pending.

| Test | Pass criterion | Evidence to date |
| --- | --- | --- |
| DOC-01 · Source fidelity | No-op preserves bytes. A small visual edit preserves the table, image link, comment, metadata, unrelated formatting and multilingual text. | Browser single-paragraph edit and Rust source-span recognition passed. Full editable table support remains open. |
| CHG-01 · Review and restore | Proposed and rejected edits leave accepted bytes alone. Apply changes only the target. Restore returns exact preimage. A later external change is retained. | Linux filesystem checks and the simulated Chromium workflow passed. Production review and live agent checks remain open. |
| AI-01/02 · Context | The adapter gets approved context and can propose only for the selected span. An unauthorised reference and post-cancellation output are refused. | Stub checks passed. Live provider containment and cancellation remain open. |
| NFR-05 · Interaction | Keyboard reaches editor and review controls. Diffs have accessible labels and keyboard acceptance. Screen readers announce state and IME works. | Chromium keyboard focus, text insertion, synthetic accept and restore controls passed. Native text insert passed. Real screen reader and IME sessions remain untested. |
| Pilot feedback | One maintainer walkthrough and additional early adopters. Target four of five completing edit, review and restore unassisted. | Candidate workflow and fixtures exist. Human walkthroughs remain pending. |

## Reproduce the Linux checks

Observed environment was Debian 13 x86-64, Linux 6.18.57, overlayfs, Xeon Platinum
8370C with three visible CPUs and a two-vCPU quota, and an 8 GiB memory limit.
Rust 1.100.0, Python 3.12.14 and Node 24.22.3 were used. Results from this VM
describe exploratory checks. Use real representative hardware for release
budgets. Collect the first physical desktop baseline on Omarchy with Hyprland
and Wayland. Pin exact OS and WebView versions for macOS and Windows. Their tests are pending.

From this directory, with Python 3.12 and the indicated tools:

~~~sh
python spikes/check_proposals.py
python spikes/check_git.py
python spikes/check_search.py
python spikes/check_interop.py
python spikes/check_formats.py
~~~

The interoperability scripts use Pandoc 3.1.11.1, Poppler 25.03.0, and
Tesseract 5.5.0 with English data and a DejaVu Sans font. Python dependencies
used were python-docx 1.2.0, Pillow 13.0.0, pypdf 6.21.4, reportlab 4.4.9,
python-pptx 1.0.2, and PyYAML 6.0.3. Generated fixture documents are synthetic
and temporary. Run Python tests on Linux for the current fork/directory sync
exercise.

Browser component checks used Playwright 1.62.0 and Chromium 156. The test
routes a synthetic origin in an offline context and blocks any request the
fixture host does not handle.

~~~sh
browser_spike=$(mktemp -d)
cp spikes/editor.mjs "$browser_spike/"
(cd "$browser_spike" && npm install --no-save --ignore-scripts --no-fund --no-audit \
  prosemirror-markdown@1.14.0 prosemirror-view@1.44.0 \
  prosemirror-state@1.4.4 markdown-it@15.0.3 esbuild@0.28.2)
"$browser_spike/node_modules/.bin/esbuild" "$browser_spike/editor.mjs" --bundle "--outfile=$browser_spike/editor.js"
python -B spikes/check_editor.py "$browser_spike/editor.js"
~~~

Run the Rust spikes from temporary copies. Save the absolute fixture path
before changing directories.

~~~sh
research=$(pwd)
for project in markdown ironcalc iced; do
  spike=$(mktemp -d)
  cp "spikes/$project/"*.rs "spikes/$project/Cargo.toml" "$spike/"
  case "$project" in
    markdown) (cd "$spike" && cargo run -- "$research/fixtures/pilot/brief.md");;
    ironcalc) (cd "$spike" && cargo run);;
    iced) (cd "$spike" && cargo run);;
  esac
done
~~~

To reproduce the Tauri shell build, install the usual Linux WebKitGTK
prerequisites on a Linux desktop, then reuse the editor bundle from the
browser check:

~~~sh
host=$(mktemp -d)
cp -R spikes/tauri/. "$host/"
cp "$browser_spike/editor.js" "$host/web/editor.js"
mkdir "$host/icons"
python -c 'from PIL import Image; import sys; Image.new("RGBA", (32, 32), (53, 81, 111, 255)).save(sys.argv[1])' "$host/icons/icon.png"
(cd "$host" && cargo run)
~~~

For this command, `browser_spike` points to the browser bundle directory. A successful
runtime check shows “Offish WebView ready” after a Rust invocation. Clicking
the editor, moving to the end of the line, then inserting x should show
“Offish WebView edit confirmed”.

Iced needs an X11 session for this pinned configuration. Type an x in the
editor and verify its fixture-text log. The check used an Xorg dummy display
and injected a click and key press.

## Omarchy desktop validation

Use an updated Omarchy installation in its Hyprland Wayland session. For
the Tauri candidate, check the Arch package `webkit2gtk-4.1` and the other
build prerequisites in Tauri's Arch Linux instructions. Record the Omarchy
release, Hyprland, kernel, WebKitGTK package version, hardware and display
scale for the run. Record whether the application uses native Wayland or
XWayland, since a successful XWayland run provides separate compatibility
evidence. [Omarchy](https://omarchy.org/) · [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) · [Arch WebKitGTK package](https://archlinux.org/packages/extra/x86_64/webkit2gtk-4.1/)

- Run the Tauri spike above. Verify that the native window opens, its WebView
  renders, the Rust invocation succeeds, and keyboard input reaches the editor.
- On an integrated candidate, use the synthetic pilot fixtures. Verify
  file chooser, focus and shortcuts under Hyprland, clipboard, scaling,
  IME composition and screen-reader behavior. Complete edit, review and
  restore on a native workspace and check an external save while a proposal
  is pending.
- Record failures and any workarounds separately from the Debian X11 and
  Chromium baseline. No hands-on Omarchy results are available yet.

## P0-01 · Product and reuse

- Cursor's documented diff and review workflow supports the idea of giving
  feedback while an agent works. Offish's pilot tests whether a document
  selection and an explicit accept/reject step provide understandable control
  to a knowledge worker. [Cursor review guidance](https://cursor.com/docs/agent/review)
- Microsoft 365 Copilot and Gemini in Workspace document AI-assisted drafting
  or analysis inside existing office flows. Follow-up walkthroughs should test
  how users supply context and recover an unwanted edit. Vendor product pages
  alone cannot establish what their review flows cover. [Microsoft](https://www.microsoft.com/en-us/microsoft-365/copilot) · [Google](https://workspace.google.com/solutions/ai/)
- A full fork would inherit editor behavior, packaging and upstream divergence.
  Collabora Online's documented architecture includes a web front end and
  LibreOffice-based rendering service. Reuse as a whole would require proving
  deployment fit and compatibility with Offish's Markdown workspace.
  Compare selective reuse of individual converters and calculation components
  with the cost of maintaining a full office/editor fork. [Collabora Online](https://github.com/CollaboraOnline/online) · [Initial ecosystem review](../INITIAL_TECHNOLOGY_ASSESSMENT.md)

Independent Rust workspace and document modules with selective reuse
remain the proposed direction. Confirm through the remaining pilot evidence.

## P0-03/04 · Desktop and Markdown

| Candidate | Observed | Remaining cost or test |
| --- | --- | --- |
| Iced 0.14.0, MIT | Built on Linux with tiny-skia, X11 and thread pool. Native text widget recorded an inserted character. | Rich-text structure and edit mapping need a separate implementation. Test accessible review UI, IME, Wayland and other OSs. [Pinned crate](https://crates.io/crates/iced/0.14.0) |
| GPUI | The upstream standalone guide describes Linux Wayland/X11, macOS Metal, and Windows backends. Its Cargo manifest declares Apache-2.0. | Pre-1.0 churn and rich-text/editor component effort remain important. No GPUI build or cross-platform validation was run. [Upstream guide](https://github.com/zed-industries/zed/tree/main/crates/gpui) |
| Tauri 2.12.7 and ProseMirror | Rust host built against WebKitGTK 2.52.6 using temporary Debian packages. Launch stopped before rendering because the image has no WebKitNetworkProcess at WebKit’s system path. Separately, Chromium rendered the table and image, accepted keyboard edits and preserved all unrelated Markdown. The default ProseMirror Markdown schema has no table node. Its full-buffer serialiser changed fixture bytes. | Need a source-mapped table schema and selection bridge. Retest runtime in a fully provisioned desktop. Browser component behavior does not establish embedded WebView parity, IME, screen reader quality or installer viability. [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) · [ProseMirror Markdown](https://github.com/ProseMirror/prosemirror-markdown) |
| pulldown-cmark 0.13.0 | Rust offset iterator selected the intended UTF-8 byte range and recognised the fixture table. A local splice kept others unchanged. | Test nested structures, CRLF, marks, ambiguous anchors, and shifts across UTF-16 UI selection offsets. [Pinned crate](https://crates.io/crates/pulldown-cmark/0.13.0) |

The browser harness also ran edit, preview, keyboard acceptance and
exact restore end to end with a deterministic adapter and temporary files.

Thirty Chromium text insertions into a generated 50,000-word model gave p50
30.5 ms and p95 31.6 ms from beforeinput to the second animation-frame
callback. This is a browser-only timing proxy. It does not measure physical
keystroke-to-paint latency, Rust IPC, full parse on write, startup, or memory.

## P0-05 · Workspace and proposal guarantees

The eight Linux tests covered review isolation, rejection, exact restore,
stale detection, an actual child exit after journaling, simulated ENOSPC,
later edits during restore, scoped context and cancellation, and byte-range
validation. The prototype uses a journaled preimage and staged file replacement.
It compares observed content revisions. It has no filesystem-wide atomic
compare-and-swap and does not model a power loss or multi-file recovery.

SQLite 3.53.1 FTS5 rebuilt a 10,000-small-file synthetic index in about 660 ms.
A warm query measured p95 0.28 ms versus 168.80 ms for reading all files
(30 measurements each). Both found the same 100 hits. Use a simple scan as a
baseline and prototype FTS5 when indexing is needed. Search ordering, UI,
incremental indexing, privacy exclusion and multilingual segmentation were
outside these timings. Tantivy remains a candidate if scale or tokenisation
needs justify another dependency. [SQLite FTS5](https://www.sqlite.org/fts5.html) · [Tantivy](https://github.com/quickwit-oss/tantivy)

Git 2.56.1 found a nested repository root and reported UTF-8 paths, ignore
rules, and staged-plus-working changes through porcelain v2. Prefer this
low-risk starting point for status if Git is installed. Verify packaging,
authentication, signing, hooks, filters, and Git-disabled behavior before
write actions. Keep gitoxide and libgit2 in consideration for distribution
and performance. [git status](https://git-scm.com/docs/git-status)

## P0-06 · Agent integration

| Candidate | Available integration evidence | Trial status and limitation |
| --- | --- | --- |
| Codex app-server | JSON-RPC stdio, streamed output and turn interruption are documented. The installed 0.172.0-alpha.2 acknowledged initialise, thread and turn requests. Its runtime rejected the docs' older restricted-read encoding. Pin and test protocol/capabilities. | Local CLI reported a ChatGPT login. The isolated synthetic turn failed with error category `unauthorized`. Response streaming, live cancellation and provider entitlement are unverified. [App-server](https://learn.chatgpt.com/docs/app-server) |
| Cursor CLI | Documents login through a browser or user API key and stream-json output. Headless print mode can write within its workspace. Permission config helps restrict tools, while Offish still needs an enforceable boundary. | Documentation only. Binary, account and embedding permissions for the pilot remain unverified. [Authentication](https://cursor.com/docs/cli/reference/authentication) · [Headless use](https://cursor.com/docs/cli/headless) · [Permissions](https://cursor.com/docs/cli/reference/permissions) |
| Claude | The Claude Agent SDK is an integration surface. Zed's upstream ACP adapter provides a useful permission and session reference. The SDK refers downstream products to Anthropic's commercial terms. Confirm the intended account and distribution before integration. | No live session. Subscription access for an Offish workflow remains unverified. [Agent SDK](https://github.com/anthropics/claude-agent-sdk-python) · [ACP adapter](https://github.com/zed-industries/claude-agent-acp) |

A signed-in, writable Codex profile is required for the optional
`python spikes/check_codex.py` rerun. It starts in a temporary empty
directory, submits synthetic text, disables shell and configured MCP
servers, and rejects server-to-client tool calls. The trial checks transport
only. Read-only mode is an insufficient containment boundary for real
workspaces. Use ACP for interoperable session semantics and MCP for mediated
typed tools if the chosen adapter supports them. Keep local or direct
model endpoints as fallback candidates. [ACP session setup](https://agentclientprotocol.com/protocol/v1/session-setup) · [MCP security practices](https://modelcontextprotocol.io/specification/2025-11-25/basic/security_best_practices)

## P0-07 · Interoperability and future formats

- Pandoc converted a synthetic DOCX to editable Markdown, including headings,
  lists, links, tables and a local image reference. Pandoc's default GFM output
  used raw HTML to retain image size. Using `gfm-raw_html` yielded Markdown
  image syntax on this fixture. Preserve source bytes and report lost
  sizing, comments, tracked changes and other unsupported layout. This is
  a candidate converter, with wider fixtures and distribution checks deferred.
- Poppler and pypdf recovered the clean text-layer page. The scanned page
  required OCR. Local Tesseract recovered it at 0% character error rate on
  this clean English sample. Poppler default, Poppler layout and pypdf all
  produced an incorrect order for the two-column control. Calibrate on
  actual layouts, track page provenance and flag uncertain order. Evaluate
  layout-aware extraction and optional AI verification using pre/post
  completeness, order and introduced-error measures. No model-based PDF
  corrections were tested.
- IronCalc 0.9.1 recalculated a cross-sheet cost per seat from 500 to 640
  after an input change and returned a divide-by-zero error as expected.
  XLSX round trips, reference rewrites, date/locale handling, and broader
  formula agreement remain Phase 4 research. A generated 30,000-cell dense
  YAML file was 327,843 bytes and parsed in 182.7 ms. Typed CSV plus a
  separate YAML manifest was 207,835 bytes, and its CSV parse was 7.1 ms in
  the single warmed run. A targeted formula used the 158-byte manifest.
  Results are directional and compare distinct encodings and parsers.
- A structured slide model retained text-box positions and notes for a
  generated PPTX. A plain narrative Markdown outline retained the text with
  no positional fidelity. Use this as a constraint for Phase 5 format
  evaluation. Layout round-trip, export and theme fidelity are open.

## P0-08/09 · Distribution and handoff

The project uses MIT. Iced and the exercised ProseMirror packages carry MIT
terms. Tauri 2.12.7 declares Apache-2.0 OR MIT. IronCalc 0.9.1 and its base crate declare MIT OR Apache-2.0. The Debian
Pandoc and Poppler package copyright files list GPL-family terms. Tesseract
and English training data carry Apache-2.0 entries. These are preliminary
component-level observations. Audit the exact release artifact, notices,
bundling vs subprocess arrangements, optional engines and models,
dependencies, font redistribution and updates when the stack is selected.
Research dependencies stay outside the application tree.

~~~mermaid
flowchart LR
    UI["Desktop editor and review"] --> CORE["Rust document and permission layer"]
    CORE --> FS["Local accepted files"]
    CORE --> J["Private proposal and checkpoint state"]
    CORE --> A["Scoped agent adapter"]
    A --> E["Agent or model endpoint"]
    CORE --> C["Isolated import/export"]
    C --> F["External files and generated assets"]
~~~

| Gate | Remaining evidence | Owner for next validation |
| --- | --- | --- |
| G0-1 | Run the pilot walkthroughs and record the Omarchy reference desktop hardware. | Maintainer and Phase 0 research owner |
| G0-2 | Validate real WebViews and native comparator where useful, source-mapped table editing, keyboard review, screen reader and IME on target systems. | P0-03/P0-04 |
| G0-3 | Verify safe save, metadata and recovery on real target filesystems, including external conflicts and interruption. | P0-05 |
| G0-4 | Establish a usable supported live authentication path, prove cancellation and containment, and document fallback. | P0-06 |
| G0-5 | Review the proposed ADRs, confirm dependencies and package terms, and record explicit deferrals. Draft Phase 1 when the foundation gates permit it. | P0-08/P0-09 |
