# ADR-0006 — Revalidate a bounded local search index

Status: Implemented for the Phase 1 search slice. Native acceptance pending.\
Date: 2026-11-27\
Requirements and phase: WS-03, NFR-02, NFR-04 · Phase 1 (P1-06)\
Related decisions: [ADR-0003](ADR-0003-desktop-and-first-editor.md)

## Context

Offish needs offline text search across multiple open folders, with clear
scope and freshness. Files can change outside Offish, and some roots can
overlap with its private application storage. The Phase 1 performance target
is warm search p95 below 300 ms. The master plan identifies local databases
for derived indexes that need persistent storage.

## Options

| Option | Value | Cost |
| --- | --- | --- |
| Scan and decode each file per search | Minimal retained data and straightforward freshness | Repeats reads and UTF-8 validation |
| Cache validated text in memory | Avoids repeated reads, supports rebuild, and leaves no additional file contents on disk | Rechecks filesystem metadata each search and spends time scanning after launch |
| Persist a private SQLite index | Enables reuse after launch and supports richer query strategies | Requires cache migration, invalidation, private storage and retention policies, and cross-platform dependency review |

## Decision

For the first Phase 1 slice, cache bounded UTF-8 text in memory and validate
it through the current root handles on each search. Discard the cache when
switching workspaces, closing Offish, or explicitly rebuilding the index.
Track file identity, size, modification and change timestamps. Rescan the
directory tree and its exclusion rules each time. Run scans off the UI thread.
Use the portable MIT or Apache-2.0 licensed `regex` crate already present in
the Cargo lockfile. Making it a direct dependency adds no packages to the
locked dependency graph and keeps pattern evaluation time bounded.

Read in-root `.gitignore` and `.offishignore` files, including nested rules.
Use the [Git pattern syntax](https://git-scm.com/docs/gitignore#_pattern_format)
for root-relative patterns, directory patterns, globs and negation. Apply
Offish exclusions independently from Git rules. Only read regular files
through the same descriptor-relative, no-follow boundary as the editor.
Exclude private application directories, Git metadata, and save staging.

## Consequences and reconsideration

Search covers saved text. Files larger than 2 MiB, binary or invalid UTF-8
files, and unsupported entries can be skipped. Search reports a capped result
set, partial scans and the last scan time. Repository-local Git exclude
files, global Git configuration, content outside registered roots, and
unsaved editor drafts are outside the initial search scope.

Revisit persistent SQLite storage after P1-T06 timings and usability
validation on Omarchy, especially if reconciling files exceeds the warm
search budget or repeated launches make the first search too slow.
