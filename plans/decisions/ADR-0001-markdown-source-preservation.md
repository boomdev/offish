# ADR-0001 — Preserve Markdown source during visual edits

Status: Proposed\
Date: 2026-11-26\
Requirements: DOC-01, WS-02, NFR-03

## Context and options

Offish must edit local Markdown visually while allowing external tools to
change the same files. Full-buffer normalisation would simplify the editor
bridge but risks loss and widespread unrelated diffs. A rich canonical format
would add migration and reduce portability. Editable source with tracked spans
supports a portable baseline at the cost of a more precise adapter.

## Proposed decision

Use CommonMark with an explicitly documented compatible pipe-table extension
and workspace-relative assets. Keep Markdown bytes authoritative. Maintain
a source revision and editor ranges tied to validated source spans. Visual
edits emit typed operations for supported ranges. Reparse the revised span
and its affected structural context before saving. Open unsupported ranges
with suitable source or read-only controls. Preserve unknown syntax, comments,
and metadata around supported edits. Decide separately when persistent
cross-document IDs or print metadata justify tags or a sidecar.

Store canonical offsets as UTF-8 byte offsets, versioned by the file revision.
Define transformations across UTF-16 editor offsets, newline encodings, and
Unicode boundaries. An ambiguous mapping requires a new selection or source
mode. A no-op visual save preserves the original bytes.

## Validation and reconsideration

The [Linux browser component spike](../research/phase-0/README.md) preserved
untouched fixture source with a localised paragraph splice. The default
global serialiser changed the body, and its schema lacked tables. These
results support the direction. Validate block boundary edits, repeated
paragraphs, nested lists, tables, CRLF, and marks spanning syntax in a real
candidate editor before accepting the ADR. Assess user-visible coverage of
unsupported spans and whether the mapping cost warrants a different model.
