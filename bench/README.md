# ASV Benchmarks

This directory contains the [ASV](https://asv.readthedocs.io/en/latest/) (airspeed velocity) contract for ghrm benchmarks.

## Post-commit check

This is the general two-command workflow for running benchmarks after a commit.

```bash
uv run asv run HEAD^!
uv run python report.py
```

## Example

```
./bench$ uv run python report.py
commit  	tip_ago	name_ms	size_ms	lines_ms	rows	subject                             
616603c6	0      	84.2   	83.6   	83.9    	1000	bench: make asv path search the ben.
666a7633	6      	84.6   	83.5   	84.0    	1000	refactor: group explorer boundary m.
663ea9c8	7      	83.7   	82.8   	83.7    	1000	refactor: group http boundary modul.
6bcfbdbd	17     	83.2   	85.9   	82.6    	1000	refactor: move search rendering out.
3142d61d	18     	87.1   	86.4   	84.0    	1000	fix: avoid double reads during line.
be990db1	19     	134.1  	140.3  	133.8   	1000	refactor: move explorer rendering o.
e801de3b	22     	128.6  	125.8  	127.1   	1000	fix: rendering SVG inside markdown  
a95248a3	24     	84.4   	83.7   	84.1    	1000	bump version to v0.4.1              
59439eb6	32     	85.3   	85.7   	83.8    	1000	Merge branch 'brege/assets'         
52232e63	37     	84.6   	83.6   	84.5    	1000	chore: bump version to v0.4.0       
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
