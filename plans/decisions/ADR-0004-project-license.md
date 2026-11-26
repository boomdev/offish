# ADR-0004 — License Offish under MIT

Status: Accepted\
Date: 2026-11-26\
Requirements: NFR-06, EXT-01

The maintainer selected MIT for Offish. MIT gives users permission to use,
modify, and redistribute the project subject to retaining its notice. Apache-2.0
was an alternative permissive license with explicit patent terms. GPL-family
alternatives would add reciprocal terms. The maintainers' preference and the
project's reuse goals favour MIT.

The root license covers Offish contributions. Third-party packages, fonts,
models, converters, and shipped binaries retain their own terms. Before a
dependency enters a release, review the actual pinned artifacts, linking or
subprocess arrangement, notice and source obligations, distribution rights,
and update process. Keep candidate research tooling outside the shipped product.
Review this decision if contributor ownership or release composition changes.
