"""Generate synthetic DOCX/PDF samples and evaluate local conversion baselines."""
from pathlib import Path
from tempfile import TemporaryDirectory
import os
import subprocess

from docx import Document
from docx.oxml import OxmlElement
from docx.oxml.ns import qn
from docx.opc.constants import RELATIONSHIP_TYPE
from PIL import Image, ImageDraw, ImageFont
from pypdf import PdfReader
from reportlab.lib.utils import ImageReader
from reportlab.pdfgen import canvas


WORDS = (
    "The pilot date is 14 December.",
    "The pilot has five seats.",
    "The budget is 3,200 dollars.",
)


def run(*args, cwd=None):
    return subprocess.run(args, cwd=cwd, check=True, capture_output=True, text=True).stdout


def fixture_font():
    path = os.environ.get("OFFISH_FIXTURE_FONT")
    if not path:
        try:
            match = run("fc-match", "--format", r"%{family}\n%{file}\n", "DejaVu Sans")
            families, path = match.splitlines()
        except (OSError, subprocess.CalledProcessError, ValueError):
            families, path = "", ""
        if "DejaVu Sans" not in families.split(",") or not path:
            raise SystemExit(
                "Install Fontconfig and DejaVu Sans, or set OFFISH_FIXTURE_FONT to a font file."
            )
    try:
        return ImageFont.truetype(str(Path(path).expanduser().resolve()), 40)
    except OSError as error:
        raise SystemExit(f"Cannot load fixture font {path!r}: {error}") from error


def normalise(text):
    return " ".join(text.split())


def distance(first, second):
    previous = list(range(len(second) + 1))
    for row, left in enumerate(first, start=1):
        current = [row]
        for column, right in enumerate(second, start=1):
            current.append(min(previous[column] + 1, current[-1] + 1,
                               previous[column - 1] + (left != right)))
        previous = current
    return previous[-1]


def check_word(root, image):
    document = Document()
    document.add_heading("Pilot brief", level=1)
    document.add_paragraph(WORDS[0])
    document.add_paragraph("Confirm seats", style="List Bullet")
    document.add_paragraph("Review budget", style="List Bullet")
    table = document.add_table(rows=2, cols=2)
    for row, values in zip(table.rows, [("Item", "Target"), ("Seats", "5")]):
        for cell, value in zip(row.cells, values):
            cell.text = value
    document.add_picture(str(image))
    target = document.add_paragraph()
    relation = target.part.relate_to(
        "https://example.org/assumptions", RELATIONSHIP_TYPE.HYPERLINK, is_external=True
    )
    link = OxmlElement("w:hyperlink")
    link.set(qn("r:id"), relation)
    run_node = OxmlElement("w:r")
    label = OxmlElement("w:t")
    label.text = "See assumptions"
    run_node.append(label)
    link.append(run_node)
    target._p.append(link)
    document.save(root / "pilot.docx")
    original = root.joinpath("pilot.docx").read_bytes()
    out = run("pandoc", "pilot.docx", "-t", "gfm-raw_html", "--extract-media=assets", cwd=root)
    assert root.joinpath("pilot.docx").read_bytes() == original
    assert "# Pilot brief" in out and WORDS[0] in out
    assert any(line.startswith("- ") and "Confirm seats" in line for line in out.splitlines()), out
    assert any(line.startswith("- ") and "Review budget" in line for line in out.splitlines()), out
    assert "https://example.org/assumptions" in out
    assert "| Item" in out and "| Seats" in out
    assert "](assets/" in out and list(root.glob("assets/**/*.png")), out
    print("PASS: DOCX heading, text, list, link, table, extracted image and original bytes")


def check_pdf(root, image):
    path = root / "pilot.pdf"
    pdf = canvas.Canvas(str(path))
    for index, line in enumerate(WORDS):
        pdf.drawString(72, 750 - index * 20, line)
    pdf.showPage()
    pdf.drawImage(ImageReader(str(image)), 48, 620, width=480, height=105)
    pdf.showPage()
    # Paint each column's first line before the second line of either column.
    columns = ("Left column first.", "Left column second.",
               "Right column first.", "Right column second.")
    for x, y, line in [(60, 750, columns[0]), (330, 750, columns[2]),
                       (60, 725, columns[1]), (330, 725, columns[3])]:
        pdf.drawString(x, y, line)
    pdf.showPage()
    pdf.save()
    reader = PdfReader(path)
    expected = " ".join(WORDS)
    native = run("pdftotext", "-f", "1", "-l", "1", str(path), "-")
    assert normalise(native) == expected
    assert normalise(reader.pages[0].extract_text()) == expected
    assert not normalise(reader.pages[1].extract_text())
    assert not normalise(run("pdftotext", "-f", "2", "-l", "2", str(path), "-"))
    run("pdftoppm", "-f", "2", "-l", "2", "-r", "300", "-png",
        "-singlefile", str(path), str(root / "scan"))
    ocr = run("tesseract", str(root / "scan.png"), "stdout", "-l", "eng")
    error = distance(normalise(ocr), expected) / len(expected)
    assert error <= 0.05, ocr
    print(f"PASS: text-layer page and image-only page, English OCR CER {error:.1%}")
    expected_columns = " ".join(columns)
    for name, output in [
        ("Poppler default", run("pdftotext", "-f", "3", "-l", "3", str(path), "-")),
        ("Poppler layout", run("pdftotext", "-layout", "-f", "3", "-l", "3", str(path), "-")),
        ("pypdf", reader.pages[2].extract_text()),
    ]:
        assert all(line in output for line in columns), output
        print(f"OBSERVED: {name} column order {'matches' if normalise(output) == expected_columns else 'differs from'} fixture")
    assert "14 December" in ocr


if __name__ == "__main__":
    font = fixture_font()
    with TemporaryDirectory() as directory:
        root = Path(directory)
        image = root / "scan-source.png"
        scan = Image.new("RGB", (1200, 263), "white")
        draw = ImageDraw.Draw(scan)
        for index, line in enumerate(WORDS):
            draw.text((80, 50 + index * 64), line, fill="black", font=font)
        scan.save(image)
        check_word(root, image)
        check_pdf(root, image)
