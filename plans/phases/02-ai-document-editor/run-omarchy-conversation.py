#!/usr/bin/env python3
"""Run P2-T04 with the real Offish WebView and a disposable workspace."""

import argparse
from pathlib import Path
import runpy
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[3]
HELPERS = runpy.run_path(str(ROOT / "plans/phases/01-core-workspace/run-omarchy-acceptance.py"))
start, wait_for, application, with_name, focused, press, descendants, text_of, hyprland = (
    HELPERS[name] for name in
    ("start", "wait_for", "application", "with_name", "focused", "press", "descendants", "text_of", "hyprland")
)

PASSAGE = "Review the first release schedule after the pilot."
REVISED = "Plan for the pilot on 14 December."
PROMPT = "Revise the launch schedule from our references."
CANCEL_PROMPT = "This turn should be cancelled before it stages anything."
DENY_PROMPT = "This request should never appear in history."
CANARY = "Hidden text should stay outside the snapshot."


def with_text(app, text):
    return next((node for node in descendants(app) if text in text_of(node)), None)


def focus_window(process):
    if hyprland("activewindow").get("pid") == process.pid:
        return
    window = wait_for("temporary Offish window", lambda: next(
        (item for item in hyprland("clients") if item["pid"] == process.pid), None), process)
    address = window["address"]
    subprocess.run(
        ["hyprctl", "dispatch", f'hl.dsp.focus({{ window = "address:{address}" }})'],
        check=True, capture_output=True, text=True, timeout=5,
    )
    wait_for("temporary window focus", lambda: hyprland("activewindow").get("pid") == process.pid, process)


def check(binary, scratch, fixture):
    brief = fixture / "brief.md"
    original = brief.read_bytes()
    updated = original.replace(PASSAGE.encode(), REVISED.encode(), 1)
    with start(binary, scratch, fixture) as (_, process):
        app = wait_for("Offish accessibility tree", lambda: application(process.pid), process)

        def node(name, role=None):
            return wait_for(name, lambda: next(
                (item for item in descendants(app) if item.get_name() == name
                 and (role is None or item.get_role_name() == role)), None), process)

        def activate(name):
            assert node(name, "button").get_action_iface().do_action(0), name

        def show_text(text):
            return wait_for(text, lambda: with_text(app, text), process)

        def enter_prompt(message):
            prompt = node("Message to offline test provider", "entry")
            assert prompt.get_component_iface().grab_focus()
            wait_for("prompt focus", lambda: focused(app, "Message to offline test provider"), process)
            press(process, "-M", "ctrl", "-k", "a", "-m", "ctrl", message)

        def select_passage(target):
            activate("brief.md")
            editor = wait_for("focused editor", lambda: focused(app, "brief.md", editable=True), process)
            offset = text_of(editor).index(target)
            selection = editor.get_text_iface()
            if selection.get_n_selections():
                assert selection.set_selection(0, offset, offset + len(target))
            else:
                assert selection.add_selection(offset, offset + len(target))

        activate("◇ brief.md")
        focus_window(process)

        select_passage(PASSAGE)
        enter_prompt(DENY_PROMPT)
        activate("Review context")
        dialog = node("Approve conversation context", "dialog")
        assert with_text(dialog, PASSAGE)
        activate("Cancel")
        show_text("Context was declined.")
        assert not with_name(app, "Conversation history")
        assert brief.read_bytes() == original

        activate("@ Add reference")
        activate("Browse pilot")
        activate("Mention assumptions.md")
        activate("@ Add reference")
        activate("Browse pilot")
        activate("Browse references")
        activate("Mention this folder")
        enter_prompt(PROMPT)
        activate("Review context")
        dialog = node("Approve conversation context", "dialog")
        assert with_text(dialog, "The pilot date moves to 14 December.")
        assert with_text(dialog, "Included folder reference.")
        assert not with_text(dialog, CANARY)
        activate("Approve and send")
        show_text("prepared a revision for your selection")
        show_text("Received 2 nonempty text references.")
        node("Review proposed edit", "button")
        assert brief.read_bytes() == original
        activate("Review proposed edit")
        wait_for("inline proposal focus", lambda: focused(app, "Accept selection change"), process)
        activate("Accept selection change")
        wait_for("accepted bytes", lambda: brief.read_bytes() == updated, process)
        print("PASS approval, file and folder mentions, streaming and staged acceptance", flush=True)

        select_passage(REVISED)
        enter_prompt(CANCEL_PROMPT)
        activate("Review context")
        activate("Approve and send")
        wait_for("cancel turn focus", lambda: focused(app, "Cancel turn"), process)
        activate("Cancel turn")
        show_text("Turn cancelled.")
        assert not with_name(app, "Review proposed edit")
        assert brief.read_bytes() == updated
        print("PASS declined and cancelled grants leave the file unchanged", flush=True)

    with start(binary, scratch, fixture) as (_, process):
        app = wait_for("reopened accessibility tree", lambda: application(process.pid), process)
        wait_for("stored prompt", lambda: with_text(app, PROMPT), process)
        wait_for("stored cancellation", lambda: with_text(app, "Cancelled"), process)
        assert not with_text(app, DENY_PROMPT)
        clear = wait_for("clear history", lambda: with_name(app, "Clear history"), process)
        assert clear.get_action_iface().do_action(0)
        confirm = wait_for("clear conversation confirmation", lambda: next(
            (node for node in descendants(app) if node.get_name() == "Clear conversation"
             and node.get_role_name() == "button"), None), process)
        assert confirm.get_action_iface().do_action(0)
        wait_for("cleared conversation", lambda: with_text(app, "Conversation cleared."), process)
        assert not list((scratch / "data").rglob("chat/*.json"))
        print("PASS local transcript survives restart and clears on request", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/debug/offish")
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="offish-p206-") as temporary:
        scratch = Path(temporary)
        fixture = scratch / "pilot"
        references = fixture / "references"
        references.mkdir(parents=True)
        (fixture / "brief.md").write_text(
            f"{PASSAGE}\n\nAn unselected paragraph stays in the file.\n", encoding="utf-8",
        )
        (fixture / "assumptions.md").write_text("The pilot date moves to 14 December.\n", encoding="utf-8")
        (references / "included.txt").write_text("Included folder reference.\n", encoding="utf-8")
        (references / "ignored.txt").write_text(CANARY, encoding="utf-8")
        (references / ".offishignore").write_text("ignored.txt\n", encoding="utf-8")
        check(args.binary.resolve(), scratch, fixture)


if __name__ == "__main__":
    main()
