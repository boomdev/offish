# Initial technology assessment

26 November 2026. GitHub repository metadata was checked for the linked repositories in the candidate tables except where marked documentation-only. Recent push activity is an activity signal. Phase 0 must verify releases, issue response, operating system support for pinned builds, actual license files and commercial feature boundaries, transitive dependencies, security handling, and benchmark results. Integration costs below are hypotheses. License labels from repository metadata may describe only part of a distribution.

## Application and editors

| Candidate and upstream | Coverage and integration assessment | Validation |
| --- | --- | --- |
| [Zed / GPUI](https://github.com/zed-industries/zed) | Native Rust UI and a reference for workspace/editor interactions. Zed is a mixed-license repository. Audit GPUI, bundled code, platform backends and assets by path. | Test standalone use, accessibility, IME, styled rich text, large virtual grids, platform licensing and maintenance burden. A full Zed fork would inherit code-editor assumptions. |
| [Iced](https://github.com/iced-rs/iced) | MIT-labeled Rust UI candidate. Reuse general controls and application shell. Rich office editors may require significant custom interaction and text layout work. | Measure accessibility, text selection, clipboard, IME and editor extensibility on all three platforms. |
| [Tauri](https://github.com/tauri-apps/tauri) | Cross-platform WebView design with a Rust host. Check MIT/Apache terms for shipped components. Enables mature web editors with IPC and OS WebView variance as integration costs. | Test WebView portability, offline assets, startup/memory, IPC isolation and packages. |
| [Tiptap](https://github.com/ueberdosis/tiptap) / [ProseMirror](https://prosemirror.net/) | Visual editing candidates. Tiptap repository metadata is MIT. Verify licenses of selected extensions and underlying packages. The top-level ProseMirror umbrella repository is archived. Check individual module maintenance. | Compare fidelity, selection mapping, supported schema, accessibility, keyboard behavior and dependency portability. |
| [Univer](https://github.com/dream-num/univer) | Apache-2.0 metadata for core repository. Evaluate grid/editor coverage and integration with a Rust-owned model. | Identify commercial-feature boundaries, export rights, recalculation ownership, data bridge cost, keyboard support and sparse grid performance. |

## Document processing and infrastructure

| Candidate and upstream | Coverage and integration assessment | Validation |
| --- | --- | --- |
| [IronCalc](https://github.com/ironcalc/IronCalc) | Apache-2.0 Rust spreadsheet engine candidate. Reusing evaluation internals may reduce formula work. | Compare supported functions and Excel semantics, cache and dependency invalidation, structural edits, XLSX scope, memory and engine/model adaptation cost. |
| [Typst](https://github.com/typst/typst) | Apache-2.0 typesetting candidate for PDF and reusable print templates. Needs a mapping from the working document model. | Test fonts, styling, pagination, deterministic output, offline availability and template safety. |
| [Pandoc](https://github.com/jgm/pandoc) | GPL-family conversion tool. Check exact version terms and redistribution obligations. Candidate sidecar for selected DOCX, ODT, HTML, and slide paths. | Validate import/export independently, conversion loss, offline binaries, packaging and installer obligations. |
| [Docling](https://github.com/docling-project/docling) | MIT metadata for the project. Candidate PDF extraction including layout-heavy cases. Python and model dependencies could dominate package size and runtime. | Verify model terms and offline availability, scanned pages, memory, timeouts, and reproducibility. |
| [MarkItDown](https://github.com/microsoft/markitdown) | MIT-labeled extraction to Markdown. Candidate for context and low-fidelity imports. | Compare output against structure/assets fixtures and assess Python dependencies and optional network features. |
| [Marp CLI](https://github.com/marp-team/marp-cli) | MIT-labeled Markdown slide renderer candidate. Narrative slide source may be portable, while arbitrary positioning and editable office export need independent validation. | Check browser packaging, fonts, notes, positioning and editable PPTX fidelity. |
| [gitoxide](https://github.com/GitoxideLabs/gitoxide), [libgit2](https://github.com/libgit2/libgit2), and [Git CLI](https://git-scm.com/) | Pure Rust candidate, native C integration, and subprocess option respectively. Review gitoxide's selected crate licenses, libgit2 linking exception, and Git redistribution terms. | Compare status/ignore parity, auth helpers, signing, worktree/conflict behavior, OS packaging and performance. |
| [Tantivy](https://github.com/quickwit-oss/tantivy) and [SQLite](https://www.sqlite.org/copyright.html) | MIT metadata for Tantivy. SQLite's upstream declares public-domain source. Compare search-specialised indexing with SQLite FTS and a simple scan baseline. | Measure bulk/incremental indexing, multilingual search, crash recovery, memory, per-OS builds, cleanup of excluded content, and update burden. |

## Agent integration

Phase 0 trial priorities are Codex, Cursor, and Claude. For Cursor and Claude, identify the officially supported integration surfaces and verify whether the available accounts cover them.

| Candidate and upstream | Proposed use and evidence still needed |
| --- | --- |
| [ACP](https://agentclientprotocol.com/) ([repository](https://github.com/agentclientprotocol/agent-client-protocol)) | Editor-to-agent sessions. Apache-2.0 metadata for the protocol repository. Validate stable protocol versions, optional capabilities, transport and cancellation behavior with actual agents. |
| [MCP](https://modelcontextprotocol.io/) ([repository](https://github.com/modelcontextprotocol/modelcontextprotocol)) | Expose typed context and proposal tools. Validate transport, permissions, tool schema evolution, pinned SDK licenses and security guidance. |
| [Codex](https://github.com/openai/codex), [Claude Code](https://code.claude.com/docs/en/overview), [Gemini CLI](https://github.com/google-gemini/gemini-cli), [OpenCode](https://github.com/anomalyco/opencode) | Candidate external agents. Repository labels for Codex/Gemini CLI are Apache-2.0, OpenCode MIT. Claude Code is a documentation review candidate. Verify supported embedding, login and API-key flows, terms, non-interactive execution, model access, context disclosure, working-directory isolation, and tool behavior per pinned version. Source license cannot establish entitlement to a commercial service. |
| [OpenAI API](https://platform.openai.com/docs/overview), [Ollama](https://docs.ollama.com/openai-compatibility), direct vendor APIs and custom agents | Documentation-only examples of model endpoints. An Offish adapter would need its own tool loop and session behavior. Test streaming/tool schema interoperability and local resource use. Verify model licenses independently of runtimes. |

## Adjacent products and build strategy

- Compare [ONLYOFFICE Document Server](https://github.com/ONLYOFFICE/DocumentServer) and [Collabora Online](https://github.com/CollaboraOnline/online) for office editor coverage, interoperability fixtures, and maintenance patterns. The Document Server repository carries an AGPL-3.0 metadata label. Collabora's GitHub metadata did not identify a license. Audit exact components and deployability before considering reuse.
- Review [Microsoft 365 Copilot](https://www.microsoft.com/en-us/microsoft-365/copilot) and [Google Workspace with Gemini](https://workspace.google.com/solutions/ai/) as documentation-only workflow references. Compare explicit context selection, transparency, change approval, and cross-document work using current, accessible product flows in Phase 0. Competitive claims and undocumented behaviors remain open.
- Review [Obsidian file storage](https://help.obsidian.md/Files+and+folders/How+Obsidian+stores+data) and [Cursor documentation](https://cursor.com/docs) for local file ownership, workspace navigation and contextual collaboration patterns. Offish's hypothesis is that these patterns plus editable office semantics and recoverable proposals solve a distinct workflow. Validate with users.
- Provisional preference is an independent Rust core with selective reuse. Adopting a complete editor could advance fidelity and UI maturity while increasing dependence on its data model and deployment constraints. A fork needs audit of license obligations, bootstrapping effort, model fit, upstream divergence costs, and evidence it materially shortens the first release. Compare all three strategies before selecting.
