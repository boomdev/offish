#!/bin/sh
# Runs the Phase 0 Python checks from the research directory.
set -eu
python=${PYTHON:-python3}
cd "$(dirname "$0")/.."

"$python" -B spikes/check_proposals.py
"$python" -B spikes/check_git.py
"$python" -B spikes/check_search.py

if "$python" -c 'import docx, pypdf, PIL, reportlab' 2>/dev/null \
    && command -v pandoc >/dev/null && command -v pdftotext >/dev/null \
    && command -v tesseract >/dev/null; then
  "$python" -B spikes/check_interop.py
else
  echo "SKIP: check_interop.py needs spikes/requirements.txt, Pandoc, Poppler and Tesseract"
fi

if "$python" -c 'import pptx, yaml' 2>/dev/null; then
  "$python" -B spikes/check_formats.py
else
  echo "SKIP: check_formats.py needs spikes/requirements.txt"
fi
