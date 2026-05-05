# ASV Benchmarks

This directory contains the [ASV](https://asv.readthedocs.io/en/latest/) (airspeed velocity) contract for ghrm benchmarks.

## Post-commit check

This is the general two-command workflow for running benchmarks after a commit.

```bash
uv run asv run HEAD^!
uv run python report.py
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
