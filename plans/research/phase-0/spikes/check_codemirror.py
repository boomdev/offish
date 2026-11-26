"""CodeMirror 6 live preview experiment on the pilot fixture. No desktop shell is started."""
from pathlib import Path
from tempfile import TemporaryDirectory
import argparse

from playwright.sync_api import sync_playwright

from check_editor import report_latency
from check_proposals import Session, accept, restore


FIXTURE = Path(__file__).resolve().parent.parent / "fixtures" / "pilot"
OLD = "Keep the first release on the current schedule."
NEW = "Check the release date before publishing."
PROPOSED = "Move the pilot launch to 14 December."


def apply_operations(source, operations):
    """Replay editor operations as byte splices, as a Rust validator would."""
    data = source.encode()
    for operation in operations:
        start, end = operation["start"], operation["end"]
        data = data[:start] + operation["replacement"].encode() + data[end:]
    return data.decode()


def open_page(context, bundle):
    page = context.new_page()
    page.goto("https://offish.invalid/")
    page.add_script_tag(path=bundle)
    return page


def check_line_endings(page, source):
    variants = {
        "LF": source,
        "CRLF": source.replace("\n", "\r\n"),
        "mixed": source.replace("\n", "\r\n", 3),
        "BOM": "\ufeff" + source,
    }
    results = page.evaluate("sources => window.lineEndingCheck(sources)", list(variants.values()))
    outcome = dict(zip(variants, results))
    assert all(result["configuredPreserves"] for result in outcome.values()), outcome
    assert outcome["LF"]["defaultPreserves"] and outcome["BOM"]["defaultPreserves"], outcome
    assert not outcome["CRLF"]["defaultPreserves"] and not outcome["mixed"]["defaultPreserves"], outcome

    crlf = variants["CRLF"]
    page.evaluate("source => { window.crlf = window.mountDocument(source); }", crlf)
    page.evaluate("text => window.crlf.select(text, 0, true)", OLD)
    page.keyboard.press("Enter")
    page.keyboard.insert_text("Second line.")
    revised = page.evaluate("window.crlf.source()")
    assert revised == crlf.replace(OLD, OLD + "\r\nSecond line.", 1), repr(revised)
    assert apply_operations(crlf, page.evaluate("window.crlf.operations()")) == revised
    page.evaluate("window.editor.destroy()")
    print("PASS P0-T16: no-op load preserves LF, CRLF, mixed and BOM bytes with a detected line separator; "
          "a CRLF edit inserts CRLF and its byte operations replay exactly")
    print("OBSERVED: CodeMirror's default line handling normalises CRLF and mixed endings")


def check_preview(page, editor):
    names = page.evaluate("editor => editor.nodeNames()", editor)
    assert "Frontmatter" in names and "SetextHeading2" not in names, names
    assert "frontmatter" in page.evaluate("e => e.lineClass('status: draft')", editor)
    assert "cm-comment" in page.evaluate("e => e.lineClass('Preserve this comment')", editor)
    assert page.evaluate("e => e.lineText('Checklist')", editor) == "Checklist"
    assert page.evaluate("e => e.lineText('Review the')", editor) == "• Review the launch assumptions."
    assert page.get_by_role("table").count() == 1
    assert page.get_by_role("cell", name="2,500").count() == 1
    image = page.get_by_role("img", name="Pilot timeline")
    assert image.evaluate("element => element.complete && element.naturalWidth > 0")
    page.evaluate("e => e.select('Checklist')", editor)
    assert page.evaluate("e => e.lineText('Checklist')", editor) == "## Checklist"
    page.evaluate("e => e.select('Review the')", editor)
    assert page.evaluate("e => e.lineText('Review the')", editor) == \
        "* Review the [launch assumptions](assumptions.md)."
    print("PASS P0-T17: front matter as metadata, comment kept, syntax hidden until the cursor enters, "
          "image and table rendered")


def check_edits(page, editor, source):
    page.evaluate("e => e.operations()", editor)
    page.get_by_role("textbox", name="Pilot brief").focus()
    page.evaluate("text => window.result.select(text)", OLD)
    page.keyboard.insert_text(NEW)
    expected = source.replace(OLD, NEW, 1)
    assert page.evaluate("e => e.source()", editor) == expected
    operations = page.evaluate("e => e.operations()", editor)
    assert apply_operations(source, operations) == expected

    page.evaluate("e => e.select('日本語', 0, true)", editor)
    page.keyboard.insert_text(" 📅")
    page.evaluate("e => e.select('café', 0, true)", editor)
    page.keyboard.press("Backspace")
    page.keyboard.insert_text("é")
    revised = page.evaluate("e => e.source()", editor)
    assert revised == expected.replace("日本語", "日本語 📅", 1)
    assert apply_operations(expected, page.evaluate("e => e.operations()", editor)) == revised
    print("PASS P0-T18: keyboard edits change only their spans; UTF-8 byte operations replay exactly, "
          "including a surrogate pair")
    return revised


def check_table_edit(page, editor, source):
    page.get_by_role("cell", name="2,500").click()
    assert page.get_by_role("table").count() == 0
    assert page.evaluate("e => e.lineText('Budget')", editor) == "| Budget (USD) | 2,500 |"
    page.evaluate("e => e.select('2,500')", editor)
    page.keyboard.insert_text("3,200")
    revised = page.evaluate("e => e.source()", editor)
    assert revised == source.replace("| 2,500 |", "| 3,200 |", 1)
    assert apply_operations(source, page.evaluate("e => e.operations()", editor)) == revised
    page.evaluate("e => e.select('Notes use')", editor)
    assert page.get_by_role("cell", name="3,200").count() == 1
    print("PASS P0-T19: table cell edited through revealed source; preview returns when the cursor leaves")
    return revised


def check_review(page, editor, source):
    data = source.encode()
    selection = NEW.encode()
    start = data.index(selection)
    agent = Session({"assumptions": (FIXTURE / "assumptions.md").read_bytes()},
                    data, start, start + len(selection))
    assert b"14 December" in agent.read("assumptions")
    proposal = agent.propose(PROPOSED.encode())
    preview = proposal.preview(data)

    with TemporaryDirectory() as directory:
        working = Path(directory) / "brief.md"
        journal = Path(directory) / "checkpoint.json"
        working.write_bytes(data)

        def show():
            page.evaluate("args => window.result.showProposal(...args)",
                          [proposal.start, proposal.end, PROPOSED])

        show()
        assert page.locator("del[aria-label='Removed: " + NEW + "']").count() == 1
        assert page.locator("ins", has_text="Added: " + PROPOSED).count() == 1
        page.get_by_role("button", name="Reject proposal").press("Enter")
        assert page.evaluate("window.reviewAction") == "reject"
        assert page.evaluate("e => e.source()", editor) == source
        assert working.read_bytes() == data

        show()
        page.evaluate("e => e.select('Notes use', 0, true)", editor)
        page.keyboard.insert_text(" ")
        assert page.evaluate("e => e.proposalState()", editor) == "stale"
        page.get_by_role("button", name="Accept proposal").press("Enter")
        assert page.evaluate("window.reviewAction") == "stale"
        page.get_by_role("textbox", name="Pilot brief").focus()
        page.keyboard.press("Backspace")
        assert page.evaluate("e => e.source()", editor) == source
        page.evaluate("e => e.operations()", editor)

        show()
        page.get_by_role("button", name="Accept proposal").press("Enter")
        assert page.evaluate("window.reviewAction") == "accept"
        assert page.evaluate("e => e.source()", editor).encode() == preview
        operations = page.evaluate("e => e.operations()", editor)
        assert apply_operations(source, operations).encode() == preview
        accept(working, journal, proposal)
        assert working.read_bytes() == preview

        page.get_by_role("textbox", name="Pilot brief").focus()
        page.keyboard.press("Control+z")
        page.keyboard.press("Control+y")
        assert page.evaluate("e => e.source()", editor).encode() == working.read_bytes() == preview
        assert not page.evaluate("e => e.operations()", editor)

        page.evaluate("e => e.select('Notes use', 0, true)", editor)
        page.keyboard.insert_text(" extra")
        assert page.evaluate("e => e.source()", editor).encode() != preview
        page.keyboard.press("Control+z")
        assert page.evaluate("e => e.source()", editor).encode() == working.read_bytes() == preview
        page.keyboard.press("Control+z")
        assert page.evaluate("e => e.source()", editor).encode() == preview

        restore(working, journal)
        assert working.read_bytes() == data
    print("PASS P0-T20: inline labelled removal and addition, keyboard reject and accept, "
          "stale proposal refused after a concurrent edit, exact restore")
    print("PASS P0-T29: CodeMirror undo stops at acceptance and still undoes later edits")


def check_latency(context, bundle):
    page = open_page(context, bundle)
    page.evaluate("window.samples = window.mountScaleFixture(); undefined")
    print("RECORDED P0-T21:")
    report_latency(page, "CodeMirror live preview")


def run(bundle, chromium=None):
    source = (FIXTURE / "brief.md").read_text()
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(
            executable_path=chromium, args=["--no-sandbox", "--disable-background-networking"]
        )
        context = browser.new_context(offline=True)
        context.route("**/*", lambda route: route.abort())
        context.route(
            "https://offish.invalid/**",
            lambda route: route.fulfill(
                path=FIXTURE / "assets" / "timeline.svg", content_type="image/svg+xml"
            ) if route.request.url.endswith("/assets/timeline.svg") else route.fulfill(
                content_type="text/html",
                body="<!doctype html><title>Pilot editor</title><main></main><aside></aside>"
            ),
        )
        page = open_page(context, bundle)
        check_line_endings(page, source)
        editor = page.evaluate_handle("source => window.result = window.mountDocument(source)", source)
        assert page.evaluate("e => e.source()", editor) == source
        assert page.evaluate("e => e.operations().length", editor) == 0
        check_preview(page, editor)
        edited = check_edits(page, editor, source)
        edited = check_table_edit(page, editor, edited)
        check_review(page, editor, edited)
        check_latency(context, bundle)
        browser.close()


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("bundle", type=Path)
    parser.add_argument("--chromium", help="Chromium executable; defaults to Playwright's browser")
    arguments = parser.parse_args()
    run(arguments.bundle.resolve(), arguments.chromium)
