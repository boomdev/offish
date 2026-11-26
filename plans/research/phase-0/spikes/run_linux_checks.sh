#!/bin/sh
# Runs the Phase 0 Python checks from the research directory.
set -eu
python=${PYTHON:-python3}
cd "$(dirname "$0")/.."

"$python" -B spikes/check_proposals.py
"$python" -B spikes/check_git.py
"$python" -B spikes/check_search.py

interop_prerequisites() {
  if ! "$python" -c 'import docx, pypdf, PIL, reportlab' 2>/dev/null; then
    echo "SKIP: check_interop.py needs the Python packages in spikes/requirements.txt"
    return 1
  fi
  for tool in pandoc pdftotext pdftoppm tesseract; do
    if ! command -v "$tool" >/dev/null; then
      echo "SKIP: check_interop.py needs $tool"
      return 1
    fi
  done
  if ! languages=$(tesseract --list-langs 2>/dev/null); then
    echo "SKIP: check_interop.py could not list Tesseract languages"
    return 1
  fi
  if ! printf '%s\n' "$languages" | grep -qx eng; then
    echo "SKIP: check_interop.py needs Tesseract English (eng) language data"
    return 1
  fi
}

if interop_prerequisites; then
  "$python" -B spikes/check_interop.py
fi

if "$python" -c 'import pptx, yaml' 2>/dev/null; then
  "$python" -B spikes/check_formats.py
else
  echo "SKIP: check_formats.py needs spikes/requirements.txt"
fi
