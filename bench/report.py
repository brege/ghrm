import json
import subprocess
import sys
from pathlib import Path


SORTS = ("name", "size", "lines")
TIME = "path_search.PathSearch.time_query"
ROWS = "path_search.PathSearch.track_rows"
SUBJECT_WIDTH = 36


def main():
    tip = parse_tip()
    root = Path(__file__).resolve().parent
    repo = root.parent
    out = []

    for path in (root / ".asv" / "results").glob("*/*.json"):
        if path.name == "machine.json":
            continue

        data = json.loads(path.read_text(encoding="utf-8"))
        commit = data.get("commit_hash")
        times = values(data, TIME)
        rows = values(data, ROWS)
        if not commit or not times:
            continue

        out.append(
            (
                ago(repo, commit, tip),
                commit[:8],
                truncate(
                    git(repo, "show", "--no-patch", "--format=%s", commit),
                    SUBJECT_WIDTH,
                ),
                ms(times.get("name")),
                ms(times.get("size")),
                ms(times.get("lines")),
                int(rows.get("name", 0)) if rows else "",
            )
        )

    print_table(
        ("commit", "tip_ago", "name_ms", "size_ms", "lines_ms", "rows", "subject"),
        ((row[1], row[0], *row[3:], row[2]) for row in sorted(out, key=sort_key)),
    )


def parse_tip():
    if len(sys.argv) == 1:
        return "HEAD"
    if len(sys.argv) == 3 and sys.argv[1] == "--tip":
        return sys.argv[2]
    raise SystemExit("usage: python report.py [--tip REF]")


def values(data, key):
    result = data.get("results", {}).get(key)
    if not result:
        return {}
    return dict(zip(SORTS, result[0]))


def ago(repo, commit, tip):
    proc = subprocess.run(
        ["git", "merge-base", "--is-ancestor", commit, tip],
        cwd=repo,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    if proc.returncode:
        return ""
    return git(repo, "rev-list", "--count", f"{commit}..{tip}")


def ms(value):
    return "" if value is None else f"{value * 1000:.1f}"


def print_table(header, rows):
    rows = [tuple(str(cell) for cell in row) for row in rows]
    widths = [len(cell) for cell in header]
    for row in rows:
        widths = [max(width, len(cell)) for width, cell in zip(widths, row)]

    print("\t".join(cell.ljust(width) for cell, width in zip(header, widths)))
    for row in rows:
        print("\t".join(cell.ljust(width) for cell, width in zip(row, widths)))


def truncate(value, width):
    if len(value) <= width:
        return value
    return value[: width - 1] + "."


def sort_key(item):
    return int(item[0]) if item[0] else 10**9


def git(repo, *args):
    return subprocess.check_output(
        ["git", *args],
        cwd=repo,
        stderr=subprocess.DEVNULL,
        text=True,
    ).strip()


if __name__ == "__main__":
    main()
