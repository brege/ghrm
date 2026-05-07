# ASV Benchmarks

This directory contains the [ASV](https://asv.readthedocs.io/en/latest/) (airspeed velocity) contract for ghrm benchmarks.

## Benchmark Types

ASV benchmarks in `bench/benchmarks/` are commit-history contracts. They build selected ghrm commits, run against deterministic fixtures, and record results that can be compared over time.

Snapshot scripts in `bench/scripts/` are current-tree comparisons. They may bootstrap real reference repositories, compare ghrm-adjacent tools against outside tools, and write JSONL captures under `bench/data/snapshots/`. These outputs are ignored by default and should only be promoted when a specific result needs to be preserved.

## Post-commit check

This is the general two-command workflow for running benchmarks after a commit.

```bash
uv run asv run HEAD^!
uv run python report.py
```

## Example

```
$ uv run python bench/report.py 
rows=1000
behind	commit	name_ms	size_ms	lines_ms	subject
53	6bcfbdbd	83.2	85.9	82.6	refactor: move search rendering out of api
54	3142d61d	87.1	86.4	84.0	fix: avoid double reads during line-count sorting
55	be990db1	134.1	140.3	133.8	refactor: move explorer rendering out of server
58	e801de3b	128.6	125.8	127.1	fix: rendering SVG inside markdown
60	a95248a3	84.4	83.7	84.1	bump version to v0.4.1
68	59439eb6	85.3	85.7	83.8	Merge branch 'brege/assets'
```

## Commands 

```bash
uv run asv machine --yes                 # record host metadata
uv run asv check                         # validate benchmark discovery
uv run asv run HEAD^!                    # benchmark the latest commit
uv run asv run HASHFILE:hashes.txt       # benchmark selected commits
uv run asv publish                       # build the ASV web report
uv run asv preview                       # serve the ASV web report
uv run python report.py                  # print the compact table
```

The path-search benchmark builds each tested commit with Cargo, installs the release binary into ASV's environment, starts ghrm against a generated fixture, waits for the navigation tree, and times `/_ghrm/path-search`.

## Reporting

The report script reads ASV result JSON and prints a compact table with commit distance from the selected tip. It defaults to `HEAD`; pass `--tip main` when you want the table relative to main.

ASV can record any reachable commit. For repo-wide backfills, generate a hashfile from the commits you want to test and run `uv run asv run HASHFILE:hashes.txt`. The configured ASV branch is only the default graph spine for ASV's own publish output.
