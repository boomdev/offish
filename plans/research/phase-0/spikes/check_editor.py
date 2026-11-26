"""Chromium editor component experiment. No desktop shell is started."""
from pathlib import Path
from statistics import median, quantiles
from tempfile import TemporaryDirectory
import argparse

from playwright.sync_api import sync_playwright

from check_proposals import Session, accept, restore


FIXTURE = Path(__file__).resolve().parent.parent / "fixtures" / "pilot"
OLD = "Keep the first release on the current schedule."
NEW = "Check the release date before publishing."
PROPOSED = b"Move the pilot launch to 14 December."


def exercise_review(page, visual_edit):
    before = visual_edit.encode()
    selection = NEW.encode()
    start = before.index(selection)
    agent = Session(
        {"assumptions": (FIXTURE / "assumptions.md").read_bytes()},
        before, start, start + len(selection),
    )
    # A deterministic adapter keeps this graphical trial offline.
    assert b"14 December" in agent.read("assumptions")
    proposal = agent.propose(PROPOSED)
    preview = proposal.preview(before)
    with TemporaryDirectory() as directory:
        working = Path(directory) / "brief.md"
        journal = Path(directory) / "checkpoint.json"
        working.write_bytes(before)
        page.evaluate("parts => window.renderReview(...parts)", [NEW, PROPOSED.decode()])
        assert page.get_by_text("Removed: " + NEW).is_visible()
        assert page.get_by_text("Added: " + PROPOSED.decode()).is_visible()
        assert working.read_bytes() == before
        page.get_by_role("button", name="Accept proposal").press("Enter")
        assert page.evaluate("window.reviewAction") == "accept"
        accept(working, journal, proposal)
        assert working.read_bytes() == preview
        page.get_by_role("button", name="Restore checkpoint").press("Enter")
        assert page.evaluate("window.reviewAction") == "restore"
        restore(working, journal)
        assert working.read_bytes() == before



def run(bundle):
    source = (FIXTURE / "brief.md").read_text()
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(
            executable_path="/usr/bin/chromium", args=["--no-sandbox", "--disable-background-networking"]
        )
        context = browser.new_context(offline=True)
        page = context.new_page()
        context.route("**/*", lambda route: route.abort())
        context.route(
            "https://offish.invalid/**",
            lambda route: route.fulfill(
                path=FIXTURE / "assets" / "timeline.svg", content_type="image/svg+xml"
            ) if route.request.url.endswith("/assets/timeline.svg") else route.fulfill(
                content_type="text/html",
                body="<!doctype html><title>Pilot editor</title><article></article><main></main>"
            ),
        )
        page.goto("https://offish.invalid/")
        page.add_script_tag(path=bundle)
        before = page.evaluate(
            """source => {
                window.result = window.exercise(source);
                return {
                    fullRoundTrip: result.defaultRoundTripPreservesBytes,
                    tableInDefaultSchema: result.defaultSchemaHasTable,
                    noOp: result.savedSource() === source
                };
            }""",
            source,
        )
        assert before == {"fullRoundTrip": False, "tableInDefaultSchema": False, "noOp": True}
        assert page.get_by_role("table").count() == 1
        assert page.get_by_role("link", name="launch assumptions").count() == 1
        image = page.get_by_role("img", name="Pilot timeline")
        assert image.evaluate("element => element.complete && element.naturalWidth > 0")
        editor = page.get_by_role("textbox", name="Selected paragraph")
        page.keyboard.press("Tab")
        assert page.get_by_role("link", name="launch assumptions").evaluate(
            "element => element === document.activeElement"
        )
        page.keyboard.press("Tab")
        assert editor.evaluate("element => element === document.activeElement")
        page.keyboard.press("Control+a")
        page.keyboard.insert_text(NEW)
        visual_edit = page.evaluate("result.savedSource()")
        assert visual_edit == source.replace(OLD, NEW, 1)
        exercise_review(page, visual_edit)
        page.goto("https://offish.invalid/")
        page.add_script_tag(path=bundle)
        page.evaluate("window.samples = window.mountScaleFixture(); undefined")
        page.get_by_role("textbox", name="Scale fixture").focus()
        for count in range(1, 31):
            page.keyboard.insert_text("x")
            page.wait_for_function("count => window.samples.length === count", arg=count)
        samples = page.evaluate("window.samples")
        p95 = quantiles(samples, n=100, method="inclusive")[94]
        print(f"50,000-word Chromium input-to-second-frame proxy: p50={median(samples):.1f} ms, p95={p95:.1f} ms")
        browser.close()

    print("PASS: offline preview, image load, keyboard edit, stub proposal, acceptance and exact restore")
    print("OBSERVED: default full-buffer round trip changes bytes and lacks table nodes")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("bundle", type=Path)
    run(parser.parse_args().bundle.resolve())
