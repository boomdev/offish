"""Compare a simple scan and SQLite FTS5 on a disposable 10,000-file corpus."""
from pathlib import Path
from statistics import median, quantiles
from tempfile import TemporaryDirectory
from time import perf_counter
import sqlite3


def benchmark(root):
    paths = []
    for folder in range(100):
        parent = root / f"folder-{folder:03d}"
        parent.mkdir()
        for number in range(100):
            path = parent / f"brief-{number:03d}.md"
            text = "Draft project brief. Review the schedule and current budget.\n" * 16
            if number == 19:
                text += "The octarine milestone is the review deadline.\n"
            path.write_text(text)
            paths.append(path)

    with sqlite3.connect(root / "search.sqlite") as db:
        before = perf_counter()
        db.execute("CREATE VIRTUAL TABLE search USING fts5(path UNINDEXED, content)")
        db.executemany(
            "INSERT INTO search VALUES (?, ?)",
            ((str(path), path.read_text()) for path in paths),
        )
        db.commit()
        print(f"FTS5 on-disk rebuild: {(perf_counter() - before) * 1000:.1f} ms")

        def scan():
            return sorted(str(path) for path in paths if "octarine" in path.read_text())

        def search():
            return sorted(row[0] for row in db.execute("SELECT path FROM search WHERE search MATCH 'octarine'"))

        expected = sorted(str(path) for path in paths if path.name == "brief-019.md")
        for label, method in [("Warm file scan", scan), ("Warm SQLite FTS5", search)]:
            assert method() == expected
            samples = []
            for _ in range(30):
                start = perf_counter()
                result = method()
                samples.append((perf_counter() - start) * 1000)
                assert result == expected
            p95 = quantiles(samples, n=100, method="inclusive")[94]
            print(f"{label}: p50={median(samples):.2f} ms, p95={p95:.2f} ms")


if __name__ == "__main__":
    print(f"SQLite {sqlite3.sqlite_version}")
    with TemporaryDirectory() as directory:
        benchmark(Path(directory))
