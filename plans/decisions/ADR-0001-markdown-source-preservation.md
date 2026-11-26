# ADR-0001 — Preserve Markdown source during visual edits

Status: Proposed\
Date: 2026-11-26\
Owner and reviewers: Project maintainer; reviewers TBD\
Requirements and phase: DOC-01, IO-01, WS-02, NFR-03 · Phase 0 (P0-04), applies from Phase 1\
Related decisions: [ADR-0002](ADR-0002-proposals-and-recovery.md), [ADR-0003](ADR-0003-desktop-and-first-editor.md)

## Context

Offish must edit local Markdown visually while allowing external tools to
change the same files. Users must not see unrelated diffs or lose constructs
the editor does not understand. The decision is needed before Phase 1 source
editing and Phase 2 visual saves. The MVP may use a source-preserving live
preview. A syntax-hiding WYSIWYG mode is required before the first official
release, so the decision must keep that path open.

## Options

| Option | Fidelity and diffs | Portability | Editor cost |
| --- | --- | --- | --- |
| Full-buffer parse and re-serialise | Normalises unrelated content. The ProseMirror Markdown spike changed fixture bytes. | High | Low |
| Rich canonical format with Markdown export | Exact in the canonical format, lossy in Markdown | Low; needs migration | Medium |
| Markdown bytes authoritative with tracked source spans | Unchanged bytes outside edited spans | High | High; needs precise span mapping |

Criteria: source fidelity, readable diffs, compatibility with external tools,
and the cost of the editor bridge.

## Decision

Keep Markdown bytes authoritative and edit through tracked source spans.

- Define an **Offish Markdown profile**: CommonMark plus explicitly listed
  extensions, initially pipe tables and workspace-relative assets. Evaluate
  whether other GFM features such as strikethrough, autolinks and task lists
  join the profile. Importers such as DOCX-to-Markdown target the same
  profile, so imported files do not open as unsupported.
- Treat a leading YAML front matter block as metadata. Never parse it as
  document content. In pure CommonMark, `---` front matter would read as a
  thematic break and a setext heading. Preserve HTML comments, raw HTML and
  unknown syntax as opaque spans.
- Make one parser authoritative for source spans. The editor and Rust
  validation must not run independent parsers that could disagree. Options
  are a Rust parser that sends a span-annotated tree to the editor, or the
  same parser running in the editor as WebAssembly. P0-04 selects one.
  A live preview that edits source text directly may use its own parser for
  presentation only, because its edits are byte splices that Rust
  revalidates. Any mode that writes through document structure, such as
  WYSIWYG, must use the authoritative parser's spans.
- Maintain a source revision and editor ranges tied to validated source
  spans. Visual edits emit typed operations for supported ranges. Reparse the
  revised span and its affected structural context before saving. Open
  unsupported ranges with source or read-only controls.
- Store canonical offsets as UTF-8 byte offsets, versioned by the file
  revision. Define transformations across UTF-16 editor offsets, newline
  encodings, and Unicode boundaries. An ambiguous mapping requires a new
  selection or source mode. A no-op visual save preserves the original bytes.
- Decide separately when persistent cross-document IDs or print metadata
  justify tags or a sidecar.

## Consequences

- External tools and Git see minimal, readable diffs.
- A source-first live preview fits this model directly. A later WYSIWYG mode
  must emit the same span-bound operations and show unsupported spans
  explicitly instead of normalising them.
- The span mapping layer is the main editor cost and risk. Coverage of
  unsupported spans is visible to users and must be measured.
- The profile becomes a versioned format contract (NFR-06).

## Validation and reconsideration

The [Linux browser component spike](../research/phase-0/README.md) preserved
untouched fixture source with a localised paragraph splice. The default
global serialiser changed the body, and its schema lacked tables. The Rust
spike recognised the table and produced a valid span. The two spikes used
different parsers (markdown-it and pulldown-cmark), and neither handled
front matter as metadata. A CodeMirror 6 live preview spike then parsed the
front matter as metadata, preserved bytes for LF, CRLF, mixed endings and a
BOM, and replayed every keyboard edit, including a table cell and a
surrogate pair, as exact UTF-8 byte operations. Before accepting, validate in a real candidate
editor with the authoritative parser: block boundary edits, repeated
paragraphs, nested lists, tables, CRLF, front matter, comments, and marks
spanning syntax. Revisit if unsupported-span coverage is too high for users
or the mapping cost warrants a different model.
