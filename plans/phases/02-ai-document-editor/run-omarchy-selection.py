#!/usr/bin/env python3
"""Check pointer-to-text geometry in the native WebKitGTK editor."""

from pathlib import Path
import subprocess
import tempfile

import gi

gi.require_version("Gtk", "3.0")
gi.require_version("WebKit2", "4.1")
from gi.repository import GLib, Gtk, WebKit2


DESKTOP = Path(__file__).resolve().parents[3] / "apps/desktop"
PROBE = r"""
import {EditorState} from "@codemirror/state";
import {EditorView} from "@codemirror/view";
import {markdownPreview} from "./markdown-preview.mjs";

const source = [
    "# Pointer selection", "",
    "Select the row above the table.", "",
    "| Item | Quantity |", "| --- | ---: |", "| Pilot | 5 |", "",
    "Select the first row below the rendered table.",
    "Select the next row below the rendered table.", "",
].join("\n");
let view;

window.mount = mode => {
    view?.destroy();
    view = new EditorView({
        parent: document.getElementById("editor"),
        state: EditorState.create({doc: source, extensions: [
            EditorView.lineWrapping,
            mode === "source" ? [] : markdownPreview(() => {}, {simple: mode === "simple"}),
        ]}),
    });
};

function point(line, index, height) {
    const range = document.createRange();
    range.setStart(line.firstChild, index);
    range.setEnd(line.firstChild, index + 1);
    const rect = range.getBoundingClientRect();
    return {clientX: rect.left + 0.25, clientY: rect.top + rect.height * height};
}

function mouse(target, type, position, buttons) {
    target.dispatchEvent(new MouseEvent(type, {bubbles: true, detail: 1, button: 0, buttons, ...position}));
}

window.check = () => {
    for (const line of view.contentDOM.querySelectorAll(".cm-line")) {
        if (!line.textContent.startsWith("Select")) continue;
        const lineStart = source.indexOf(line.textContent);
        for (const height of [0.2, 0.5, 0.8]) {
            const start = point(line, 2, height);
            const end = point(line, 15, height);
            mouse(line, "mousedown", start, 1);
            mouse(document, "mouseup", start, 0);
            if (view.state.selection.main.head !== lineStart + 2) {
                return `Click missed "${line.textContent}" at height ${height}.`;
            }
            mouse(line, "mousedown", start, 1);
            mouse(document, "mousemove", end, 1);
            mouse(document, "mouseup", end, 0);
            const {anchor, head} = view.state.selection.main;
            if (anchor !== lineStart + 2 || head !== lineStart + 15) {
                return `Drag missed "${line.textContent}" at height ${height}.`;
            }
            view.dispatch({selection: {anchor: 0}});
        }
    }
    return "pass";
};
"""


def run(script):
    window = Gtk.Window()
    window.set_default_size(900, 600)
    web = WebKit2.WebView.new_with_context(WebKit2.WebContext.new_ephemeral())
    window.add(web)
    window.show_all()
    failures = []
    modes = iter(("live", "simple", "source"))

    def evaluate(expression, done):
        def finish(view, result, _):
            try:
                done(view.evaluate_javascript_finish(result))
            except Exception as error:
                failures.append(str(error))
                Gtk.main_quit()
        web.evaluate_javascript(expression, -1, None, None, None, finish, None)

    def next_mode():
        mode = next(modes, None)
        if mode is None:
            Gtk.main_quit()
            return

        def checked(result):
            message = result.to_string()
            if message != "pass":
                failures.append(f"{mode}: {message}")
            else:
                print(f"PASS {mode}: clicks and drags above and below the table")
            next_mode()

        def mounted(_):
            GLib.timeout_add(150, lambda: (evaluate("check()", checked), False)[1])

        evaluate(f"mount('{mode}')", mounted)

    def loaded(_view, event):
        if event == WebKit2.LoadEvent.FINISHED:
            next_mode()

    def timeout():
        failures.append("WebKit selection probe timed out.")
        Gtk.main_quit()
        return False

    web.connect("load-changed", loaded)
    css = (DESKTOP / "web/offish.css").read_text()
    web.load_html(
        f"<!doctype html><style>{css}</style>"
        f'<main id="editor" style="width:700px;height:550px"></main><script>{script}</script>',
        None,
    )
    GLib.timeout_add_seconds(10, timeout)
    Gtk.main()
    if failures:
        raise SystemExit("\n".join(failures))


if __name__ == "__main__":
    with tempfile.TemporaryDirectory(prefix="offish-selection-") as temp:
        bundle = Path(temp) / "probe.js"
        subprocess.run(
            [str(DESKTOP / "node_modules/.bin/esbuild"), "--bundle", "--format=iife",
             "--sourcefile=selection-probe.mjs", f"--outfile={bundle}"],
            cwd=DESKTOP, input=PROBE, text=True, check=True,
        )
        run(bundle.read_text())
