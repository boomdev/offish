# Offish

Offish is a planned open-source desktop office suite for working with AI across documents, spreadsheets, presentations, and reference files. Open a local folder, edit visually, and review the changes an agent proposes before they become part of your work. Files use open formats and remain accessible to external tools.

The project is implementing the Phase 1 core workspace. Start with the [master plan](plans/MASTER_PLAN.md), the [planning index](plans/README.md), and the [contributor and agent guidance](AGENTS.md).

## Run the Phase 1 development build

On Omarchy, use Rust, Node.js, npm and the Tauri Linux development libraries,
including GTK 3 and WebKitGTK 4.1. From the repository root:

```sh
npm --prefix apps/desktop ci --ignore-scripts --no-audit --no-fund
npm --prefix apps/desktop run build
cargo run --locked -p offish -- /absolute/path/to/a/test-folder
```

Launch without a folder argument to use the folder picker. Use a disposable
copy of the pilot fixtures for the first run. Select a Markdown file, edit
the source, and use the Save button or Ctrl+S. The details panel displays
a conflicting disk version while the draft remains in the editor.
An interrupted save can leave a recovery copy. Reopen the file and select
**Load recovered draft** to inspect it.

The Linux build preserves owner, group, mode, POSIX access ACLs and
`user.*` extended attributes on ordinary local files. It refuses saves when
those attributes cannot be preserved, a file has another extended attribute
type, or a file has multiple hard links. Recovery storage must be outside
the opened folder. Open a project folder rather than your
entire home directory. See the [Phase 1 plan](plans/phases/01-core-workspace/PLAN.md)
for guarantees under test and remaining tasks.

Run the filesystem tests with `cargo test --locked --no-default-features`.

Offish is licensed under the [MIT License](LICENSE).
