#!/usr/bin/env python3
"""Measure Phase 1 startup and search, and check reload and keyboard on Omarchy."""

import argparse
from collections import deque
from contextlib import contextmanager
import json
import math
import os
from pathlib import Path
import statistics
import subprocess
import tempfile
import time

import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi, GLib


ROOT = Path(__file__).resolve().parents[3]
FIRST = "file-00000.md"
SECOND = "file-00001.md"
RELOADED = "External replacement for the clean tab"


def hyprland(command):
    response = subprocess.run(
        ["hyprctl", command, "-j"], stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        text=True, timeout=3,
    )
    if response.returncode:
        raise RuntimeError(f"Hyprland {command} unavailable: {response.stdout.strip()}")
    try:
        return json.loads(response.stdout)
    except json.JSONDecodeError as error:
        raise RuntimeError(f"Hyprland {command} returned invalid JSON: {response.stdout.strip()}") from error


def press(process, *keys):
    # Leave other windows alone if the user moves focus while the run is active.
    if hyprland("activewindow").get("pid") != process.pid:
        raise RuntimeError("The temporary Offish window does not have keyboard focus.")
    subprocess.run(["wtype", *keys], check=True, timeout=5)


def wait_for(description, check, process=None, timeout=15):
    deadline = time.monotonic() + timeout
    context = GLib.MainContext.default()
    last_error = None
    while time.monotonic() < deadline:
        while context.pending():
            context.iteration(False)
        if process is not None and process.poll() is not None:
            raise RuntimeError(f"Offish exited before {description}.")
        try:
            result = check()
        except GLib.Error as error:
            last_error = error
            result = None
        if result:
            return result
        time.sleep(0.01)
    raise TimeoutError(f"Timed out waiting for {description}.") from last_error


def application(pid):
    desktop = Atspi.get_desktop(0)
    for index in range(desktop.get_child_count()):
        app = desktop.get_child_at_index(index)
        if app is not None and app.get_process_id() == pid:
            return app
    return None


def descendants(root):
    pending = deque([root])
    while pending:
        node = pending.popleft()
        if node is None:
            continue
        yield node
        # The relevant controls and the first two matches occur early in each
        # branch. Avoid fetching all 10,000 accessible file rows on every poll.
        for index in range(min(node.get_child_count(), 12)):
            pending.append(node.get_child_at_index(index))


def with_name(root, name):
    return next((node for node in descendants(root) if name in (node.get_name() or "")), None)


def text_of(node):
    interface = node.get_text_iface()
    return (interface.get_text(0, -1) or "") if interface else ""


def focused(root, name, *, editable=False):
    for node in descendants(root):
        state = node.get_state_set()
        if not state.contains(Atspi.StateType.FOCUSED) or (editable and not state.contains(Atspi.StateType.EDITABLE)):
            continue
        if name in (node.get_name() or "") or name in text_of(node):
            return node
    return None


def search_status(dialog, expected):
    for node in descendants(dialog):
        if node.get_role() == Atspi.Role.STATUS_BAR:
            for label in descendants(node):
                text = f"{label.get_name() or ''} {text_of(label)}"
                if expected in text and "Scanned at" in text:
                    return text
    return None


def prepare_fixture(path):
    path.mkdir()
    text = "Local workspace search fixture. Topic: example.\n" * 20
    for index in range(10_000):
        extra = "needle\n" if index % 100 == 0 else ""
        (path / f"file-{index:05}.md").write_text(text + extra, encoding="utf-8")


@contextmanager
def start(binary, scratch, fixture):
    env = os.environ.copy()
    env.update(GDK_BACKEND="wayland")
    env.pop("NO_AT_BRIDGE", None)
    env.pop("WEBKIT_DISABLE_ACCESSIBILITY", None)
    for kind in ("CONFIG", "DATA", "CACHE"):
        env[f"XDG_{kind}_HOME"] = str(scratch / kind.lower())
    with (scratch / "offish.log").open("w+", encoding="utf-8") as log:
        started = time.monotonic()
        process = subprocess.Popen([str(binary), str(fixture)], env=env, stdout=log, stderr=log)
        try:
            yield started, process
        except (RuntimeError, TimeoutError, GLib.Error) as error:
            log.seek(0)
            raise RuntimeError(f"{error}\n{log.read().strip()}".strip()) from error
        finally:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=3)


def editor(process, app, file):
    return wait_for(f"focused editor for {file}", lambda: focused(app, file, editable=True), process)


def keyboard_and_reload(process, app, fixture):
    press(process, "-k", "Down")
    wait_for("first focused file row", lambda: focused(app, FIRST), process)
    press(process, "-k", "Return")
    wait_for("original text in the clean tab", lambda: focused(app, "Topic: example.", editable=True), process)
    path = fixture / FIRST
    replacement = f"# {RELOADED}\n\nneedle\n"
    path.write_text(replacement, encoding="utf-8")
    wait_for("external edit in the clean tab", lambda: focused(app, RELOADED, editable=True), process)
    if path.read_text(encoding="utf-8") != replacement:
        raise RuntimeError("The clean reload altered the fixture on disk.")

    for _ in range(2):
        press(process, "-M", "alt", "-k", "Left", "-m", "alt")
    wait_for("active file in the tree", lambda: focused(app, FIRST), process)
    press(process, "-k", "Down")
    wait_for("second focused file row", lambda: focused(app, SECOND), process)
    press(process, "-k", "Return")
    editor(process, app, SECOND)
    press(process, "-M", "ctrl", "-M", "shift", "-k", "Tab", "-m", "shift", "-m", "ctrl")
    editor(process, app, FIRST)
    press(process, "-M", "ctrl", "-k", "Tab", "-m", "ctrl")
    editor(process, app, SECOND)
    marker = "Keyboard save marker"
    second = fixture / SECOND
    original = second.read_text(encoding="utf-8")
    press(process, "-M", "ctrl", "-k", "End", "-m", "ctrl", marker)
    press(process, "-M", "ctrl", "-k", "s", "-m", "ctrl")
    wait_for("keyboard save on disk", lambda: second.read_text(encoding="utf-8") == original + marker, process)
    press(process, "-M", "ctrl", "-k", "w", "-m", "ctrl")
    editor(process, app, FIRST)
    print("Native clean reload and keyboard navigation passed. AT-SPI exposed focus throughout.", flush=True)


def measure_search(process, app, count):
    shortcut = ("-M", "ctrl", "-M", "shift", "-k", "f", "-m", "shift", "-m", "ctrl")
    press(process, *shortcut)
    dialog = wait_for("search dialog", lambda: with_name(app, "Find in Workspace"), process)
    wait_for("focused search field", lambda: focused(dialog, "Text to find"), process)
    times = []
    for index in range(count + 1):
        query, expected = ("needle", "100 matches") if index % 2 == 0 else ("absent", "No matches")
        press(process, "-M", "ctrl", "-k", "a", "-m", "ctrl", query)
        started = time.monotonic()
        press(process, "-k", "Return")
        wait_for(expected, lambda: search_status(dialog, expected), process)
        if index:
            times.append((time.monotonic() - started) * 1000)
        if query == "needle":
            wait_for("focused first match", lambda: focused(dialog, FIRST), process)
            if index == 0:
                press(process, "-k", "Down")
                wait_for("focused second match", lambda: focused(dialog, "file-00100.md"), process)
                press(process, "-k", "Up")
                wait_for("focused first match again", lambda: focused(dialog, FIRST), process)
            press(process, *shortcut)
            wait_for("search field again", lambda: focused(dialog, "Text to find"), process)
    press(process, "-M", "ctrl", "-k", "a", "-m", "ctrl", "needle")
    press(process, "-k", "Return")
    wait_for("focused search result", lambda: focused(dialog, FIRST), process)
    press(process, "-k", "Return")
    editor(process, app, FIRST)
    print("Native search result navigation and opening passed.", flush=True)
    return times


def report(label, samples, target):
    p95 = sorted(samples)[math.ceil(len(samples) * 0.95) - 1]
    outcome = "pass" if p95 < target else "above target"
    print(f"{label}: n={len(samples)} p50={statistics.median(samples):.1f} ms "
          f"p95={p95:.1f} ms ({outcome}, target < {target} ms)", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/release/offish")
    parser.add_argument("--startup-runs", type=int, default=5)
    parser.add_argument("--search-runs", type=int, default=20)
    args = parser.parse_args()
    if not args.binary.is_file() or args.startup_runs < 1 or args.search_runs < 2:
        parser.error("Build Offish first and use positive startup and search sample counts.")
    try:
        hyprland("clients")
        Atspi.init()
        if Atspi.get_desktop(0) is None:
            raise RuntimeError("AT-SPI desktop unavailable.")
        with tempfile.TemporaryDirectory(prefix="offish-acceptance-") as temporary:
            scratch = Path(temporary)
            fixture = scratch / "search"
            prepare_fixture(fixture)
            starts = []
            for index in range(args.startup_runs):
                run = scratch / f"run-{index}"
                run.mkdir()
                with start(args.binary.resolve(), run, fixture) as (started, process):
                    app = wait_for("Offish in AT-SPI", lambda: application(process.pid), process)
                    wait_for("first explorer file", lambda: with_name(app, FIRST), process)
                    starts.append((time.monotonic() - started) * 1000)
                    if index == args.startup_runs - 1:
                        report("Cold process to usable explorer", starts, 3000)
                        keyboard_and_reload(process, app, fixture)
                        warm = measure_search(process, app, args.search_runs)
                        report("Warm search to AT-SPI status (includes input and polling)", warm, 300)
                        status = Path(f"/proc/{process.pid}/status").read_text(encoding="utf-8")
                        print("Offish host peak RSS:", next(line for line in status.splitlines() if line.startswith("VmHWM:")))
    except (OSError, ValueError, RuntimeError, TimeoutError, GLib.Error, subprocess.SubprocessError) as error:
        parser.exit(1, f"{error}\n")


if __name__ == "__main__":
    main()
