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
| Pilot feedback | One maintainer walkthrough and additional early adopters. Current target is four of five completing edit, review and restore unassisted, to be recalibrated once the early adopter count and operating systems are known. | Candidate workflow and fixtures exist. Early adopter count and human walkthroughs remain pending. |

Platform support is tiered. Omarchy provides the Phase 0 and Phase 1 evidence,
and macOS and Windows are gates for the Phase 2 release.

## Acceptance test index

| Test ID | Requirements | Check | Status on Linux |
| --- | --- | --- | --- |
| P0-T01 | CHG-01, NFR-03 | `check_proposals.py`: review is private until accept, exact restore | Pass |
| P0-T02 | CHG-01 | `check_proposals.py`: reject has no filesystem effect | Pass |
| P0-T03 | WS-02, CHG-01 | `check_proposals.py`: external edit blocks a stale proposal | Pass |
| P0-T04 | NFR-03 | `check_proposals.py`: recovery after process exit, divergence detection | Pass |
| P0-T05 | NFR-03 | `check_proposals.py`: injected ENOSPC keeps a recoverable preimage | Pass |
| P0-T06 | CHG-01, NFR-03 | `check_proposals.py`: restore preserves later edits | Pass |
| P0-T07 | AI-02, NFR-04 | `check_proposals.py`: scoped reads and cancellation (stub) | Pass |
| P0-T08 | DOC-01 | `check_proposals.py`: a patch cannot split a UTF-8 code point | Pass |
| P0-T09 | GIT-01 | `check_git.py`: root discovery, status, UTF-8 path, ignore rule | Pass |
| P0-T10 | WS-03, NFR-02 | `check_search.py`: scan versus FTS5 timings | Recorded |
| P0-T11 | DOC-01, ED-01, NFR-05 | `check_editor.py`: offline preview, keyboard edit, review, restore (Chromium) | Pass |
| P0-T12 | DOC-01 | `markdown` Rust spike: validated UTF-8 span and table recognition | Pass |
| P0-T13 | AI-01, AI-02 | `check_codex.py`: signed-in Codex streaming and interrupt | Pass on Omarchy with synthetic text; containment and sign-in terms pending |
| P0-T14 | IO-01 | `check_interop.py`: DOCX structure and PDF text/OCR baselines | Pass |
| P0-T15 | SHEET-01, SLIDE-01 | `check_formats.py` and `ironcalc` spike: storage, recalculation, slide geometry | Pass / recorded |
| P0-T16 | DOC-01, NFR-03 | `check_codemirror.py`: no-op preserves LF, CRLF, mixed and BOM bytes; CRLF edit keeps CRLF | Pass |
| P0-T17 | DOC-01, ED-01 | `check_codemirror.py`: front matter as metadata, comment kept, syntax hidden until the cursor enters, image and table rendered | Pass |
| P0-T18 | DOC-01, NFR-03 | `check_codemirror.py`: keyboard edits replay exactly as UTF-8 byte operations, including multilingual text and a surrogate pair | Pass |
| P0-T19 | DOC-01 | `check_codemirror.py`: table cell edit through revealed source | Pass |
| P0-T20 | CHG-01, NFR-05 | `check_codemirror.py`: inline labelled review, keyboard accept and reject, stale proposal refused, exact restore | Pass |
| P0-T21 | NFR-02 | `check_editor.py` and `check_codemirror.py`: 50,000-word latency proxies | Recorded |
| P0-T22 | NFR-04, DOC-01 | `check_workspace_assets.py` and `tauri-codemirror` Rust tests: workspace asset path containment | Pass |
| P0-T23 | AI-02, NFR-04 | `check_codex_containment.py`: mediated reads/writes, cancellation; optional bubblewrap | Pass (bwrap skipped when user namespaces denied) |
| P0-T24 | DOC-01, NFR-03 | Native WebKitGTK: nested list and blockquote preview, no-op source, bounded edit | Pass on Omarchy |
| P0-T25 | CHG-01, NFR-05 | Native WebKitGTK: keyboard reject and accept with labelled controls | Pass on Omarchy |
| P0-T26 | DOC-01, NFR-05 | Real IME composition, source export and reload | Pending: no composing IME configured in the active Fcitx group |
| P0-T27 | NFR-01, NFR-02 | Native WebKitGTK: 50,000-word input latency proxy runs (paced and burst, p50/p95) | Recorded; paced clears 50 ms, direct paint remains open |

## Reproduce the Linux checks

Observed environment was Debian 13 x86-64, Linux 6.18.57, overlayfs, Xeon Platinum
8370C with three visible CPUs and a two-vCPU quota, and an 8 GiB memory limit.
Rust 1.100.0, Python 3.12.14 and Node 24.22.3 were used. Results from this VM
describe exploratory checks. Use real representative hardware for release
budgets. Collect the first physical desktop baseline on Omarchy with Hyprland
and Wayland. Pin exact OS and WebView versions for macOS and Windows. Their tests are pending.

From this directory, with Python 3.12 and the indicated tools. The pinned
Python packages are in `spikes/requirements.txt`. `spikes/run_linux_checks.sh` runs
the checks that need only Python and Git, and runs the interoperability and
format checks when their requirements are installed.

~~~sh
python -m venv .venv && . .venv/bin/activate
pip install -r spikes/requirements.txt
python spikes/check_proposals.py
python spikes/check_git.py
python spikes/check_search.py
python spikes/check_interop.py
python spikes/check_formats.py
~~~

Cargo lockfiles are not yet committed for the Rust spikes, so transitive
crate versions can drift between runs.

The interoperability scripts use Pandoc 3.1.11.1, Poppler 25.03.0, and
Tesseract 5.5.0 with English (`eng`) data. The spike uses both Poppler tools
(`pdftotext` and `pdftoppm`) and finds DejaVu Sans through Fontconfig. Set
`OFFISH_FIXTURE_FONT` to an absolute font file path to override discovery.
Python dependencies used were python-docx 1.2.0, Pillow 13.0.0, pypdf 6.21.4,
reportlab 4.4.9, python-pptx 1.0.2, and PyYAML 6.0.3. Generated fixture documents are synthetic
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
python -B spikes/check_editor.py "$browser_spike/editor.js" --chromium /usr/bin/chromium
~~~

Omit `--chromium` to use the browser that Playwright installed.

The CodeMirror 6 live preview check uses the same offline host. It reuses
`check_proposals.py` for acceptance and restore, and `check_editor.py` for
latency reporting.

~~~sh
codemirror_spike=$(mktemp -d)
cp spikes/codemirror.mjs "$codemirror_spike/"
(cd "$codemirror_spike" && npm install --no-save --ignore-scripts --no-fund --no-audit \
  @codemirror/view@6.44.1 @codemirror/state@6.7.6 @codemirror/commands@6.11.2 \
  @codemirror/language@6.13.1 @codemirror/lang-markdown@6.5.2 \
  @codemirror/lang-yaml@6.1.3 @lezer/markdown@1.8.2 esbuild@0.28.2)
"$codemirror_spike/node_modules/.bin/esbuild" "$codemirror_spike/codemirror.mjs" --bundle "--outfile=$codemirror_spike/codemirror.js"
python -B spikes/check_codemirror.py "$codemirror_spike/codemirror.js" --chromium /usr/bin/chromium
~~~

The editor comparison runs on 26 November 2026 used Ubuntu 24.04.5,
Linux 6.12.113, four vCPUs of an Intel Xeon, 16 GiB of memory, Node 22.23.4,
Python 3.12.3, Playwright 1.65.2, and Google Chrome 157.0.8103.50. The
ProseMirror check was rerun in the same environment.

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
  Chromium baseline.

### 26 November 2026 Omarchy run

Reference desktop: Omarchy 4.5.0-3, Hyprland 0.56.2 in Wayland, Linux
7.2.15-1-omarchy, Intel Core i5-1135G7 (4 cores/8 threads), 16 GiB RAM,
WebKitGTK 2.54.1-1, Rust/Cargo 1.99.1, Node 26.12.0, Python 3.14.8,
Chromium 155.0.8059.84-1. The internal 2256×1504 display ran at 1.6 scale;
the external 1920×1080 display ran at 1.0 scale, both at 60 Hz. Hyprland
reported the Tauri window as native Wayland (`xwayland: false`).

- `run_linux_checks.sh` passed P0-T01–T09 and P0-T14, and recorded P0-T10
  and P0-T15 with the pinned Python requirements. Pandoc 3.11, Poppler
  26.10.0 and Tesseract 5.5.3 were installed. This desktop lacks DejaVu Sans,
  so the run set
  `OFFISH_FIXTURE_FONT=/usr/share/fonts/liberation/LiberationSans-Regular.ttf`.
  DOCX structure and image extraction passed; text and image-only PDF pages
  passed with 0.0% English OCR character error on the synthetic fixture.
  Poppler and pypdf still returned a different column order from the fixture.
  Warm FTS5 search p95 was 0.10 ms; warm file scan p95 was 85.44 ms. These
  are local spike measurements, not release benchmarks.
- The pinned CodeMirror and ProseMirror browser checks passed in local
  Chromium. CodeMirror passed P0-T16–T20. On the 50,000-word Chromium fixture,
  its input-to-work-and-layout proxy was p50 3.5 ms, p95 18.3 ms; the
  input-to-second-frame proxy was p50 31.8 ms, p95 32.5 ms. ProseMirror's
  corresponding p95 values were 20.3 ms and 35.6 ms. These proxies do not
  measure WebKitGTK or production keystroke-to-paint latency.
- The documented Tauri/ProseMirror host built and opened; its Rust invocation
  set “Offish WebView ready” and a keyboard edit set “Offish WebView edit
  confirmed”. A temporary copy of the host was then adapted to mount the
  existing CodeMirror bundle and pilot brief. CodeMirror rendered headings,
  lists and a table, and a keyboard edit set “Offish CodeMirror edit
  confirmed” in native WebKitGTK. That temporary adapter is superseded by the
  committed `spikes/tauri-codemirror` host, which serves relative assets
  through a `workspace:` URI scheme scoped to `OFFISH_WORKSPACE` (see
  reproduction below). Path escape unit tests pass (P0-T22). In the Omarchy
  rerun, `OFFISH_WORKSPACE="$PWD/fixtures/pilot" cargo run --manifest-path
  spikes/tauri-codemirror/Cargo.toml` opened a native Wayland window titled
  “Offish CodeMirror ready”. The pilot timeline SVG rendered as a graphic in
  WebKitGTK, rather than alt text. Typing `x` into the editor changed the
  window title to “Offish CodeMirror edit confirmed”.
- `check_codex.py` passed against the signed-in Codex CLI 0.172.0: the
  synthetic answer arrived in nine streamed text events and interruption was
  acknowledged with final status `interrupted`. The test ran in an empty
  temporary directory, refused tool calls and exposed no workspace files.
  It establishes transport behavior only.
- OpenAI documents an official [Sign in with ChatGPT](https://developers.openai.com/siwc/quickstart)
  path for open-source apps and a [Codex app-server token-sharing guide](https://developers.openai.com/siwc/token-sharing-open-source/codex-app-server).
  Shipping Offish should use that OAuth flow and pass `ACCESS_TOKEN` into
  app-server, rather than relying on a pre-existing `codex login` session.
  Host-side mediation checks (P0-T23) cover authorised reads, proposal-only
  writes and cancellation. `bubblewrap` is installed on this desktop, but the
  containment spike could not create a user namespace here; evaluate Landlock
  or a confirmed bwrap policy on Omarchy before enabling filesystem-capable
  adapters.

### Native WebKitGTK follow-up

On the same Omarchy 4.5.0-3 / Hyprland 0.56.2 / WebKitGTK 2.54.1 reference
desktop, the repeatable `run_omarchy_webkit.py` driver passed P0-T24 and
P0-T25. It launched a new native Wayland Tauri process per check and used
`wtype` for keyboard input. The [nested fixture](fixtures/pilot/nested.md)
parsed four list items and a blockquote. A no-op load preserved its source;
a selected text edit changed only that span. The blockquote now displays a
quote border and hides `>` syntax away from the cursor; the active list line
still reveals its source marker as designed. A [window capture](evidence/omarchy-webkit-structure.png)
records the rendered state. Keyboard Enter rejected a proposal without
changing source, then accepted a fresh proposal with only the selected range
changed. These are editor-buffer checks, not disk persistence tests.

P0-T27 used three new app processes per mode after the build, with warm OS
caches and both displays at 60 Hz. Fifty `wtype` keystrokes were sent per
run, and WebKitGTK produced 50 measured input events in each run. The probe
waited for input to stop before calculating percentiles.

- Initial spike baseline (burst typing at 20 ms keystroke delay, before
  removing two O(N) spike shortcuts—prefix UTF-8 re-encoding on every
  keystroke and a full-tree table walk): input-to-work-and-layout p95 was
  29 ms in all three runs; input-to-second-frame p95 was 56, 54 and 55 ms
  (p50 was not saved from that earlier run).
- After replacing both O(N) paths with an incremental line-byte index and
  range-bounded table updates:
  - Paced typing (`--delay-ms 150`, ~6.7 chars/sec): work p50 was 4.0, 4.0
    and 4.0 ms; work p95 was 26.0, 25.0 and 7.0 ms; second-frame p50 was
    25.0, 25.0 and 25.0 ms; second-frame p95 was 32.0, 32.0 and 33.0 ms.
  - Burst typing (`--delay-ms 20`, 50 chars/sec): work p50 was 3.0, 4.0 and
    4.0 ms; work p95 was 12.0, 27.0 and 30.0 ms; second-frame p50 was 26.0,
    29.0 and 28.0 ms; second-frame p95 was 32.0, 50.0 and 63.0 ms.

With the O(N) spike bottlenecks removed, paced typing sits at a 4.0 ms work
median and 32.0–33.0 ms second-frame p95 (right at the 60 Hz two-frame
floor), clearing the proposed 50 ms budget in all three runs. Occasional
25–30 ms tail spikes in fresh processes still queue up under 20 ms bursts.
These numbers remain refresh-quantised proxies on plain paragraphs (without
dense tables/decorations or Rust IPC) and do not establish direct
keystroke-to-paint latency. Keep direct paint measurement and broader
warm/cold sampling open before accepting ADR-0003.

P0-T26 remains pending. Fcitx5 currently has only `keyboard-be` in its active
group. A temporary attempt to add the already installed Mozc input method
did not select it; the original Fcitx profile was restored byte-for-byte.
No synthetic key or composition event was counted as a real IME test.

The integrated edit–review–restore pilot, external-save conflict, real IME,
screen reader, clipboard, scaling and direct WebKitGTK paint measurement
remain pending. Keep P0-03 and P0-06 in progress.

### Reproduce the Tauri CodeMirror host

From `plans/research/phase-0`, with the WebKitGTK prerequisites already
installed:

~~~sh
research=$(pwd)
codemirror_spike=$(mktemp -d)
cp spikes/codemirror.mjs "$codemirror_spike/"
(cd "$codemirror_spike" && npm install --no-save --ignore-scripts --no-fund --no-audit \
  @codemirror/view@6.44.1 @codemirror/state@6.7.6 @codemirror/commands@6.11.2 \
  @codemirror/language@6.13.1 @codemirror/lang-markdown@6.5.2 \
  @codemirror/lang-yaml@6.1.3 @lezer/markdown@1.8.2 esbuild@0.28.2)
"$codemirror_spike/node_modules/.bin/esbuild" "$codemirror_spike/codemirror.mjs" \
  --bundle "--outfile=$codemirror_spike/editor.js"

host=$(mktemp -d)
cp -R spikes/tauri-codemirror/. "$host/"
cp "$codemirror_spike/editor.js" "$host/web/editor.js"
# Cargo.lock is copied with the spike for reproducible builds.
(cd "$host" && OFFISH_WORKSPACE="$research/fixtures/pilot" cargo run --locked)
~~~

A successful run shows “Offish CodeMirror ready”. The pilot timeline image
should render (not alt text only). Typing into the brief so the buffer
contains `x` should show “Offish CodeMirror edit confirmed”. Path containment
without a window:

~~~sh
python -B spikes/check_workspace_assets.py
cargo test --manifest-path spikes/tauri-codemirror/Cargo.toml
~~~

To reproduce P0-T24, P0-T25 and P0-T27 in an active Omarchy Hyprland
Wayland session with `wtype` installed, bundle the same editor into the
committed host and run the driver. It opens and closes temporary windows and
focuses them while typing. Its latency samples are proxies as described
above.

~~~sh
cp "$codemirror_spike/editor.js" spikes/tauri-codemirror/web/editor.js
python3 -B spikes/run_omarchy_webkit.py --latency-runs 3 --delay-ms 150
python3 -B spikes/run_omarchy_webkit.py --latency-runs 3 --delay-ms 20 --latency-only
~~~

For P0-T26, configure and activate a real composing IME, then launch with
`OFFISH_WEBKIT_PROBE=ime` and enter a composed phrase in the focused editor.
The probe requires `compositionstart` and `compositionend`, a changed source
buffer and byte-identical reload before it reports PASS.

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

The browser probe parses with markdown-it and the Rust probe with
pulldown-cmark, so their spans are not yet guaranteed to agree. Neither treats
the fixture's front matter as metadata: the browser probe strips it with a
regular expression, and the Rust probe does not enable pulldown-cmark's YAML
metadata block option. Under plain CommonMark the block reads as a thematic
break followed by a setext heading. ADR-0001 now requires one authoritative
parser and an Offish Markdown profile.

### Editor comparison: CodeMirror 6 live preview and ProseMirror

Both editors ran on the pilot fixture in the same offline Chromium host
(P0-T11 and P0-T16 to P0-T21). The CodeMirror candidate edits the Markdown
source directly. It uses `@lezer/markdown` with CommonMark, pipe tables and
YAML front matter, hides inline syntax outside the lines holding the cursor,
and renders images and tables as widgets that reveal their source when
entered. All packages listed are MIT.

| Scenario | CodeMirror 6 live preview | ProseMirror (default Markdown schema) |
| --- | --- | --- |
| No-op save | Exact bytes for LF, CRLF, mixed endings and BOM once the file's line separator is configured | Exact only because the spike re-serialises one paragraph; full-buffer serialisation changed bytes |
| Front matter and comment | Parsed as front matter and shown as metadata; comment kept and styled | Removed by a regular expression before parsing |
| Paragraph edit by keyboard | Only the edited span changes; UTF-8 byte operations replay exactly | Only for a unique single-line paragraph through a special-case splice |
| Multilingual text and emoji | Byte operations replay exactly, including a surrogate pair | Not tested |
| Table | Rendered as a table; a cell edit through revealed source changes only that cell | No table node in the schema |
| Inline review | Labelled `del`/`ins` decorations in the document, keyboard accept and reject, stale proposal refused | Separate labelled panel outside the editor |
| Syntax visibility | Markdown syntax appears on the line holding the cursor | Hidden (true WYSIWYG) |
| Latency proxies, p95 over four runs | 6.1–11.8 ms to work and layout; 32.3–32.4 ms to second frame | 3.3–4.5 ms to work and layout; 32.2–32.4 ms to second frame |
| Spike size | 407 source lines; 176 KB minified and gzipped, including the HTML, CSS and JavaScript code-block languages that lang-markdown pulls in | 89 source lines; 105 KB minified and gzipped, without a source-mapping layer |

Findings:

- Byte preservation in the live preview does not depend on parser agreement.
  Edits are source splices, so `@lezer/markdown` only affects presentation,
  and a disagreement shows up as a rendering error rather than changed bytes.
  Rust still revalidates each operation. A WYSIWYG mode that writes through
  structure would bring back the single-authority requirement of ADR-0001.
- CodeMirror normalises CRLF and mixed line endings unless the line separator
  is configured from the file. With a CRLF separator, a browser-handled Enter
  inserted a bare LF and produced mixed endings. Binding Enter to
  `insertNewline` and normalising typed or pasted line breaks fixed it.
- Editor undo reverted an accepted proposal in the buffer while the accepted
  file stayed unchanged. ADR-0002 must define undo across acceptance.
- The original latency proxy, two animation frames after input, is bounded by
  the 60 Hz display refresh (about 33 ms) and cannot separate editors. The
  work-and-layout proxy ends after pending tasks and a forced layout. Both are
  browser-only proxies. They do not measure physical keystroke-to-paint
  latency, WebKitGTK, Rust IPC, full parse on write, startup, or memory. The
  scale fixture is plain paragraphs, so decoration cost for dense markup is
  not measured.
- Untested: nested lists and blockquotes, marks spanning lines, IME
  composition, screen reader behavior for hidden syntax and the table widget,
  and WebKitGTK on Omarchy.

The earlier run in the Debian environment gave p50 30.5 ms and p95 31.6 ms
for ProseMirror on the frame-bounded proxy.

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

Git 2.56.1 resolved the repository root from a subdirectory and reported
UTF-8 paths, ignore rules, and staged-plus-working changes through porcelain
v2. Nested repositories and submodules were not tested. Prefer this
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
typed tools if the chosen adapter supports them.

Codex with ChatGPT sign-in is the first integration targeted for release.
Next steps: run `check_codex.py` on the maintainer's signed-in Omarchy
session, confirm that ChatGPT sign-in is supported for an Offish
integration, and show that Offish controls which files the agent sees and
which changes reach disk. Cursor sign-in is the fallback if Codex is blocked.
API-key, direct and local model endpoints are later-release work. [ACP session setup](https://agentclientprotocol.com/protocol/v1/session-setup) · [MCP security practices](https://modelcontextprotocol.io/specification/2025-11-25/basic/security_best_practices)

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
| G0-1 | Record early adopters and their operating systems, and run the pilot walkthroughs. Omarchy reference desktop hardware is recorded above. | Maintainer and Phase 0 research owner |
| G0-2 | CodeMirror opens, renders workspace assets, preserves a nested fixture, and supports keyboard review in native WebKitGTK on Omarchy. Real IME composition, direct paint latency, broader accessibility and the WYSIWYG path remain open. Screen readers on macOS and Windows are Phase 2 release gates. | P0-03/P0-04 |
| G0-3 | Verify safe save, metadata, hunks with partial acceptance, checkpoint history and recovery on Omarchy filesystems, including external conflicts and interruption. | P0-05 |
| G0-4 | Streaming/interrupt passed; SIWC+Codex app-server is the documented shipping sign-in path. Finish OS sandbox evidence (bwrap/Landlock), late-output cleanup and redaction; keep Cursor sign-in as the fallback. | P0-06 |
| G0-5 | Review the proposed ADRs, confirm dependencies and package terms, and record explicit deferrals. Draft Phase 1 when the foundation gates permit it. | P0-08/P0-09 |
