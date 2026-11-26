#!/usr/bin/env python3
"""Run the Tauri CodeMirror probes in the current Hyprland Wayland session.

Bundle tauri-codemirror/web/editor.js as documented before invoking this file.
The script focuses only windows belonging to the temporary Offish spike.
"""

import argparse
import json
import os
from pathlib import Path
from queue import Empty, Queue
import subprocess
from threading import Thread
from time import monotonic, sleep


RESEARCH = Path(__file__).resolve().parent.parent
MANIFEST = RESEARCH / "spikes/tauri-codemirror/Cargo.toml"
TARGET_DIR = Path(os.environ.get("CARGO_TARGET_DIR", RESEARCH / "spikes/tauri-codemirror/target"))
BINARY = TARGET_DIR / "debug/offish-tauri-codemirror-spike"
BUNDLE = RESEARCH / "spikes/tauri-codemirror/web/editor.js"
WORKSPACE = RESEARCH / "fixtures/pilot"


def clients():
    output = subprocess.check_output(["hyprctl", "clients", "-j"], text=True)
    return [client for client in json.loads(output)
            if client["class"] == "offish-tauri-codemirror-spike"]


def window_with_title(title, timeout=30):
    deadline = monotonic() + timeout
    while monotonic() < deadline:
        for client in clients():
            if client["title"] == title:
                if client["xwayland"]:
                    raise RuntimeError("probe opened through XWayland, not native Wayland")
                return client["address"]
        sleep(0.1)
    raise TimeoutError(f"window title did not appear: {title}")


def press(address, *keys):
    # Hyprland 0.56+ Lua dispatcher syntax for focusing by window address.
    dispatcher = f'hl.dsp.focus({{ window = "address:{address}" }})'
    subprocess.run(["hyprctl", "dispatch", dispatcher], check=True, capture_output=True)
    subprocess.run(["wtype", *keys], check=True)


def start(mode):
    environment = os.environ.copy()
    environment["OFFISH_WORKSPACE"] = str(WORKSPACE)
    environment["OFFISH_WEBKIT_PROBE"] = mode
    process = subprocess.Popen([str(BINARY)], cwd=RESEARCH, env=environment,
                               stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                               text=True, bufsize=1)
    output = Queue()

    def collect():
        for line in process.stdout:
            output.put(line.strip())
        output.put(None)

    Thread(target=collect, daemon=True).start()
    return process, output


def result(output, mode, expected, timeout=30):
    deadline = monotonic() + timeout
    while monotonic() < deadline:
        try:
            line = output.get(timeout=max(0.1, deadline - monotonic()))
        except Empty:
            break
        if line is None:
            raise RuntimeError(f"{mode} window exited before {expected}")
        if line.startswith(f"OFFISH_WEBKIT_PROBE {mode}:"):
            print(line, flush=True)
            if "FAIL" in line:
                raise AssertionError(line)
            if expected in line:
                return line
    raise TimeoutError(f"{mode} did not report {expected}")


def run(mode, delay_ms=150):
    process, output = start(mode)
    try:
        if mode == "structure":
            result(output, mode, "PASS blockquote")
            window_with_title("Offish structure PASS")
        elif mode == "review":
            address = window_with_title("Offish review reject ready")
            press(address, "-k", "Return")
            result(output, mode, "PASS keyboard reject")
            window_with_title("Offish review accept ready")
            press(address, "-k", "Return")
            result(output, mode, "PASS keyboard accept")
            window_with_title("Offish review PASS")
        elif mode == "latency":
            address = window_with_title("Offish latency ready")
            press(address, "-d", str(delay_ms), "x" * 50)
            result(output, mode, "PASS n=", timeout=120)
            window_with_title("Offish latency PASS")
        else:
            raise ValueError(mode)
    finally:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--latency-runs", type=int, default=3)
    parser.add_argument("--delay-ms", type=int, default=150,
                        help="Delay between wtype keystrokes in ms (default: 150 for paced typing)")
    parser.add_argument("--latency-only", action="store_true",
                        help="Skip structure and review probes and run latency only")
    args = parser.parse_args()
    if args.latency_runs < 1:
        parser.error("--latency-runs must be positive")
    if args.delay_ms < 0:
        parser.error("--delay-ms must be non-negative")
    if not BUNDLE.is_file():
        parser.error(f"bundle the CodeMirror editor first: {BUNDLE}")
    subprocess.run(["cargo", "build", "--locked", "--manifest-path", str(MANIFEST)],
                   check=True, cwd=RESEARCH)
    if not args.latency_only:
        run("structure")
        run("review")
    for index in range(args.latency_runs):
        print(f"Latency run {index + 1}/{args.latency_runs} (delay={args.delay_ms} ms)", flush=True)
        run("latency", delay_ms=args.delay_ms)


if __name__ == "__main__":
    main()
