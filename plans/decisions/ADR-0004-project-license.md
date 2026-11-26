# ADR-0004 — License Offish under MIT

Status: Accepted\
Date: 2026-11-26\
Owner and reviewers: Project maintainer\
Requirements and phase: NFR-06, EXT-01 · Phase 0 (P0-08)\
Related decisions: None

## Context

Offish needs a project license before accepting contributions or publishing
releases. The project aims for broad reuse of its code and formats.

## Options

| Option | Terms | Fit |
| --- | --- | --- |
| MIT | Permissive; retain the notice | Simple and widely understood; favours reuse |
| Apache-2.0 | Permissive with explicit patent terms and notice rules | Favours reuse; more obligations for redistributors |
| GPL-family | Reciprocal terms | Restricts reuse in proprietary products |

## Decision

The maintainer selected MIT. MIT gives users permission to use, modify, and
redistribute the project subject to retaining its notice. The maintainer's
preference and the project's reuse goals favour MIT over Apache-2.0 and
GPL-family alternatives.

## Consequences

- The root license covers Offish contributions. Third-party packages, fonts,
  models, converters, and shipped binaries retain their own terms.
- MIT has no explicit patent grant, unlike Apache-2.0.
- Before a dependency enters a release, review the actual pinned artifacts,
  linking or subprocess arrangement, notice and source obligations,
  distribution rights, and update process. GPL-family tools such as Pandoc
  and Poppler need particular care if bundled.
- Keep candidate research tooling outside the shipped product.

## Validation and reconsideration

The license file is in the repository root. Review this decision if
contributor ownership or release composition changes. Changing the license
would need a superseding ADR and agreement from contributors.
