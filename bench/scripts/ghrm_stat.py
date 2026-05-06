#!/usr/bin/env python3
import argparse
import json
import random
import shutil
import statistics
import subprocess
import time
from collections import defaultdict
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path


REPOS = {
    "fd": "https://github.com/sharkdp/fd.git",
    "ripgrep": "https://github.com/BurntSushi/ripgrep.git",
    "tokei": "https://github.com/XAMPPRocky/tokei.git",
    "onefetch": "https://github.com/o2sh/onefetch.git",
}
ONEFETCH_ARGS = (
    "--output",
    "json",
    "--no-art",
    "--no-color-palette",
    "--no-bold",
    "--true-color",
    "never",
    "--churn-pool-size",
    "1000000",
)


@dataclass(frozen=True)
class Sample:
    repo: Path
    tool: str
    run: int


def parse_args():
    root = Path(__file__).resolve().parents[2]
    parser = argparse.ArgumentParser()
    parser.add_argument("--refs", type=Path, default=root / "refs")
    parser.add_argument("--runs", type=int, default=10)
    parser.add_argument("--warmups", type=int, default=1)
    parser.add_argument("--seed", type=int, default=0)
    parser.add_argument("--out", type=Path)
    parser.add_argument("--skip-build", action="store_true")
    return parser.parse_args()


def main():
    args = parse_args()
    root = Path(__file__).resolve().parents[2]
    stat_root = root / "ghrm-stat"
    out = args.out or root / "bench" / "data" / "snapshots" / timestamped_name()
    onefetch = shutil.which("onefetch")
    if onefetch is None:
        raise SystemExit("missing onefetch")

    repos = bootstrap_refs(args.refs)
    if not args.skip_build:
        subprocess.run(
            [
                "cargo",
                "build",
                "--manifest-path",
                str(stat_root / "Cargo.toml"),
                "--release",
                "--locked",
            ],
            cwd=root,
            check=True,
        )
    binary = stat_root / "target" / "release" / "ghrm-stat"
    if not binary.exists():
        raise SystemExit(f"missing {binary}")

    warmup(repos, binary, onefetch, args.warmups)

    samples = [
        Sample(repo=repo, tool=tool, run=run)
        for repo in repos
        for run in range(args.runs)
        for tool in ("ghrm-stat", "onefetch")
    ]
    random.Random(args.seed).shuffle(samples)

    out.parent.mkdir(parents=True, exist_ok=True)
    rows = []
    with out.open("w", encoding="utf-8") as handle:
        for sample in samples:
            row = run_sample(sample, binary, onefetch)
            rows.append(row)
            handle.write(json.dumps(row, sort_keys=True) + "\n")

    print_table(rows)
    print(f"\nresults\t{out}")


def timestamped_name():
    stamp = datetime.now().strftime("%Y%m%d-%H%M%S")
    return f"stat-compare-{stamp}.jsonl"


def bootstrap_refs(refs: Path):
    refs.mkdir(parents=True, exist_ok=True)
    repos = []
    for name, url in REPOS.items():
        repo = refs / name
        if (repo / ".git").exists():
            repos.append(repo)
            continue
        if repo.exists():
            raise SystemExit(f"{repo} exists but is not a git repository")
        subprocess.run(["git", "clone", url, str(repo)], check=True)
        repos.append(repo)
    return repos


def warmup(repos, binary, onefetch, count):
    for _ in range(count):
        for repo in repos:
            run_command(command("ghrm-stat", repo, binary, onefetch))
            run_command(command("onefetch", repo, binary, onefetch))


def run_sample(sample, binary, onefetch):
    started = time.perf_counter_ns()
    result = run_command(command(sample.tool, sample.repo, binary, onefetch))
    elapsed_ms = (time.perf_counter_ns() - started) / 1_000_000
    return {
        "repo": sample.repo.name,
        "repo_path": str(sample.repo),
        "tool": sample.tool,
        "run": sample.run,
        "elapsed_ms": round(elapsed_ms, 3),
        "exit_code": result.returncode,
    }


def command(tool, repo, binary, onefetch):
    if tool == "ghrm-stat":
        return [str(binary), "--json", str(repo)]
    if tool == "onefetch":
        return [onefetch, *ONEFETCH_ARGS, str(repo)]
    raise ValueError(tool)


def run_command(cmd):
    return subprocess.run(
        cmd,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )


def print_table(rows):
    stats = defaultdict(list)
    for row in rows:
        if row["exit_code"] == 0:
            stats[(row["repo"], row["tool"])].append(row["elapsed_ms"])

    print("repo      \tghrm_ms\tonefetch_ms\tratio\tsamples")
    for repo in REPOS:
        ghrm = stats.get((repo, "ghrm-stat"), [])
        onefetch = stats.get((repo, "onefetch"), [])
        if not ghrm or not onefetch:
            print(f"{repo:<10}\t-\t-\t-\t0")
            continue
        ghrm_ms = statistics.median(ghrm)
        onefetch_ms = statistics.median(onefetch)
        ratio = onefetch_ms / ghrm_ms if ghrm_ms else 0
        samples = min(len(ghrm), len(onefetch))
        print(f"{repo:<10}\t{ghrm_ms:.1f}\t{onefetch_ms:.1f}\t{ratio:.1f}x\t{samples}")


if __name__ == "__main__":
    main()
