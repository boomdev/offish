"""Check the cost of dense values and preservation of positioned slide elements."""
from io import StringIO
from pathlib import Path
from tempfile import TemporaryDirectory
from time import perf_counter
import csv
import json

from pptx import Presentation
from pptx.util import Inches
from yaml import CSafeLoader, CSafeDumper, dump, load


def storage_comparison():
    dense = [[row, f"item-{row}", row * 11] for row in range(10_000)]
    manifest = {"worksheets": [
        {"id": "forecast", "formulas": {"B2": "=SUM(Data!C1:C10000)"}, "data": "values.csv"},
        {"id": "data", "columns": ["id:integer", "item:string", "total:integer"]},
    ]}
    buffer = StringIO(newline="")
    csv.writer(buffer, lineterminator="\n").writerows(dense)
    csv_source = buffer.getvalue()
    dense_yaml = dump({**manifest, "values": dense}, Dumper=CSafeDumper, sort_keys=True)
    sparse_yaml = dump(manifest, Dumper=CSafeDumper, sort_keys=True)
    start = perf_counter()
    assert load(dense_yaml, Loader=CSafeLoader)["values"] == dense
    yaml_ms = (perf_counter() - start) * 1000
    start = perf_counter()
    parsed = [(int(a), b, int(c)) for a, b, c in csv.reader(StringIO(csv_source))]
    assert parsed == [tuple(row) for row in dense]
    csv_ms = (perf_counter() - start) * 1000
    assert load(sparse_yaml, Loader=CSafeLoader) == manifest
    print(f"Dense YAML: {len(dense_yaml)} bytes, warm parse {yaml_ms:.1f} ms")
    print(f"Typed CSV + YAML manifest: {len(csv_source) + len(sparse_yaml)} bytes, CSV parse {csv_ms:.1f} ms")
    print(f"One formula can be transmitted with its manifest ({len(sparse_yaml)} bytes)")


def slide_comparison(root):
    deck = Presentation()
    slide = deck.slides.add_slide(deck.slide_layouts[6])
    for x, y, label in [(1, 1, "Pilot launch"), (4, 3, "14 December")]:
        box = slide.shapes.add_textbox(Inches(x), Inches(y), Inches(3), Inches(1))
        box.text = label
    slide.notes_slide.notes_text_frame.text = "Check revised assumptions."
    path = root / "pilot.pptx"
    deck.save(path)
    imported = Presentation(path).slides[0]
    positioned = [{"id": f"shape-{shape.shape_id}", "text": shape.text, "left": int(shape.left),
                   "top": int(shape.top), "width": int(shape.width), "height": int(shape.height)}
                  for shape in imported.shapes]
    payload = {"units": "EMU", "elements": positioned, "notes": imported.notes_slide.notes_text_frame.text}
    assert json.loads(json.dumps(payload)) == payload
    assert positioned[1]["left"] == Inches(4)
    assert payload["notes"] == "Check revised assumptions."
    narrative = "\n\n".join(shape["text"] for shape in positioned)
    assert all(shape["text"] in narrative for shape in positioned)
    print("PASS: candidate structured slide form retains text box coordinates and speaker notes")
    print("OBSERVED: plain Markdown outline retains the text only on this fixture")


if __name__ == "__main__":
    storage_comparison()
    with TemporaryDirectory() as directory:
        slide_comparison(Path(directory))
