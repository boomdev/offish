# Offish Markdown profile v1

This is the source contract for P2-01. The visual editor work in P2-02
and P2-03 uses the same contract. See
[ADR-0001](../../decisions/ADR-0001-markdown-source-preservation.md) and the
[Phase 2 plan](PLAN.md).

## Markdown syntax and preview coverage

The syntax baseline is [CommonMark 0.31.2](https://spec.commonmark.org/0.31.2/).
The first preview covers paragraphs, headings, emphasis and strong emphasis,
links, and ordered and unordered lists with nesting. The profile also includes
block quotes, code, pipe tables using the [GFM table extension](https://github.github.com/gfm/#tables-extension-),
and workspace-relative images.

In Simple mode, supported preview markers stay hidden on the active line.
The first in-place table editor covers plain-text cells in top-level tables
with leading and trailing pipes, consistent column counts, and delimiter
rows of three or more hyphens per cell with optional alignment colons.
It accepts blank cells and preserves surrounding cell padding. Shift+Enter
inserts a line break. Rust stores each break as an exact `<br>` tag on
the same source row, including consecutive breaks. The table previews
render those tags as line breaks. Use Source for other inline Markdown,
escapes, and tables whose boundaries the span validator cannot confirm,
including tables following possible code fences or raw HTML blocks.

A complete leading front matter fence is metadata. After an optional UTF-8
BOM, the opening line must be exactly three hyphens. The block ends at a
line containing exactly three hyphens or three dots, with an optional
terminating newline. Preserve the entire span. An unclosed fence remains
in the body. YAML values remain opaque in this contract.

Preserve raw HTML, comments, and unsupported syntax in source mode.
The preview treats raw HTML as opaque and uses a source or read-only
fallback for unsupported structures. Additional GFM extensions and custom
syntax require a profile update. Images resolve relative to the document
through the authorised workspace boundary. The desktop preview accepts
PNG, JPEG, GIF, WebP and SVG images up to 4 MiB. The preview does not
fetch remote images or render scriptable content from raw HTML.

## Source coordinates and edits

- The UTF-8 file is authoritative. Canonical ranges are absolute, zero-based,
  half-open UTF-8 byte ranges on a captured revision. A leading BOM occupies
  three file bytes and is outside the editor's coordinate space.
- Editor positions count UTF-16 code units in the BOM-stripped document.
  A CRLF contributes one editor position and two file bytes. Ranges must
  start and end at scalar boundaries. The mapping rejects a split surrogate
  pair, UTF-8 multibyte scalar, or CRLF.
- The typed batch has `profile: 1` and a list of `edits`. Each edit supplies
  `from`, `to`, `expected`, and `replacement`. Order by `from` against the
  original revision. Overlapping edits and repeated start offsets are
  invalid. Offish checks expected bytes before applying replacements.
- Replacements use the file's LF or CRLF convention. A file without line
  endings defaults to LF for inserted newlines. Mixed or standalone CR
  source stays read-only for this batch contract. An empty batch preserves
  that source byte-for-byte. The leading BOM and the workspace's UTF-8 and
  size limits also apply.
- The desktop Rust command `save_source_edits` takes the workspace-qualified
  path, base revision and batch. It shares the source editor's locked save,
  conflict and recovery path. An empty batch against the current revision
  leaves the file untouched. A stale revision reports a conflict.

The batch validates coordinates and byte identity. P2-02 uses Lezer in
CodeMirror for presentation and edits the underlying source directly.
When a visual control writes through Markdown structure, it must validate
the affected spans with the authoritative parser and enforce opaque ranges.
For P2-03, Lezer locates candidate tables for display. Rust determines
whether the candidate fits the supported table grammar, identifies the
cell's byte range, and checks the same table layout after the splice.
CodeMirror receives the validated replacement as one undoable edit.
