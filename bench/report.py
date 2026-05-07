import json
import shutil
import subprocess
import sys
from pathlib import Path


SORTS = ("name", "size", "lines")
TIME = "path_search.PathSearch.time_query"
ROWS = "path_search.PathSearch.track_rows"


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
                git(repo, "show", "--no-patch", "--format=%s", commit),
                ms(times.get("name")),
                ms(times.get("size")),
                ms(times.get("lines")),
                int(rows.get("name", 0)) if rows else "",
            )
        )

    rows = sorted(out, key=sort_key)
    row_counts = {row[6] for row in rows if row[6] != ""}
    show_rows = len(row_counts) != 1
    if len(row_counts) == 1:
        print(f"rows={row_counts.pop()}")

    header = ("behind", "commit", "name_ms", "size_ms", "lines_ms")
    table = [(row[0], row[1], *row[3:6], row[2]) for row in rows]
    if show_rows:
        header = (*header, "rows")
        table = [(row[0], row[1], *row[3:]) for row in rows]

    print_table((*header, "subject"), table)


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
    body_widths = widths[:-1]
    for row in rows:
        body_widths = [
            max(width, len(cell)) for width, cell in zip(body_widths, row[:-1])
        ]

    terminal_width = shutil.get_terminal_size((100, 20)).columns
    subject_width = max(
        len(header[-1]),
        terminal_width - sum(body_widths) - len(body_widths),
    )
    widths = [*body_widths, subject_width]

    print("\t".join(header))
    for row in rows:
        row = (*row[:-1], truncate(row[-1], subject_width))
        print("\t".join(row))


def truncate(value, width):
    if len(value) <= width:
        return value
    if width <= 2:
        return value[:width]
    return value[: width - 2] + ".."


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
