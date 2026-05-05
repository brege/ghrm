# ASV Benchmarks

This directory contains the ASV contract for ghrm benchmarks.

Run from this directory:

```bash
./bw machine --yes
./bw check
./bw run HEAD^!
./bw run NEW --skip-existing-commits
./bw publish
./bw open
```

The wrapper resolves ASV with UV from `pyproject.toml`; ASV does not need to be
installed globally. ASV machine metadata, UV's environment, UV's cache, and ASV
results stay under `bench/`.

The path-search benchmark builds each tested commit with Cargo, installs the
release binary into ASV's environment, starts ghrm against a generated fixture,
waits for the navigation tree, and times `/_ghrm/path-search`.

Scheduler policy, host-state capture, Monitorat correlation, and long-term
result databases live outside this repository.
