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

Launch without a folder argument to restore the last workspace. If no
workspace was active, choose a saved workspace or select **Create Workspace**
to pick one or more folders. **File → Add Folder to Workspace** appends folders
to the Files tree and saves the workspace automatically in Offish’s local
configuration. **File → Close Workspace** returns to the saved workspace
chooser. **File → Open Folder** starts a single-folder workspace, and
**File → Open File** opens a document directly.

Use **Edit → Find in Workspace** or Ctrl+Shift+F to search saved UTF-8 text
across all open roots. Select a result to jump to the matching line.
Search rechecks disk changes and reads each root’s nested `.gitignore` and
`.offishignore` rules. **Rebuild Index** clears the in-memory cache and scans
the roots again. The dialog shows when the scan ran and reports scan limits.

Use a disposable copy of the pilot fixtures for the first run. Select a Markdown file, edit
the source, and use **File → Save** or Ctrl+S. Hover or focus a folder row
in the Files pane to show its **New file** and **New folder** icons. Use the
pencil next to the workspace name to rename it. Each file or subfolder’s **⋯** menu
offers Rename, Move, Duplicate, and Move to Trash. Expand folders directly
in the tree. With focus on a tree row, use the arrow keys to navigate, Enter
to open, F2 to rename, Delete to move to Trash, and Shift+Delete to confirm
permanent deletion. Shift+F10 opens the row’s actions. Use Alt+Left or
Alt+Right to move focus between panes and dividers. Alt+Up focuses File,
and Alt+Down returns focus to the workspace from the menu bar. Left and
Right move between dialog buttons. Ctrl+W closes the focused editor tab.
Ctrl+Tab cycles right through open tabs and Ctrl+Shift+Tab cycles left, wrapping
at either end. **Edit** includes undo, redo, and plain-text clipboard actions. Move takes a folder path
relative to the file’s root folder. Duplicate copies the saved contents.
Close affected tabs before moving an entry to Trash, then use the
**Trash** line at the bottom of the Files pane to restore it to its original path. Trash requires the
workspace and private recovery storage to support atomic renames between
them. An existing entry at the original path blocks restore. The details panel displays
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
