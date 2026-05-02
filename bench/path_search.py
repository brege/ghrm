#!/usr/bin/env python3
import argparse
import json
import os
import pathlib
import socket
import subprocess
import sys
import tempfile
import threading
import time
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass


VERSION = "v1"
FIXTURE_BASE = pathlib.Path(tempfile.gettempdir()) / "ghrm-path-search-bench"


@dataclass(frozen=True)
class Profile:
    dirs: int
    py_per_dir: int
    rs_per_dir: int
    md_per_dir: int
    excluded_py: int
    hidden_py: int
    docs_only_dirs: int
    docs_only_md: int

    @property
    def file_count(self):
        regular = self.dirs * (self.py_per_dir + self.rs_per_dir + self.md_per_dir)
        docs = self.docs_only_dirs * self.docs_only_md
        return regular + docs + self.excluded_py + self.hidden_py

    @property
    def py_count(self):
        return self.dirs * self.py_per_dir + self.excluded_py + self.hidden_py

    @property
    def indexed_py_count(self):
        return self.dirs * self.py_per_dir + self.hidden_py


@dataclass(frozen=True)
class Scenario:
    label: str
    no_excludes: bool
    excluded_dir_rows: int


PROFILES = {
    "smoke": Profile(
        dirs=12,
        py_per_dir=60,
        rs_per_dir=6,
        md_per_dir=6,
        excluded_py=80,
        hidden_py=80,
        docs_only_dirs=4,
        docs_only_md=40,
    ),
    "medium": Profile(
        dirs=80,
        py_per_dir=250,
        rs_per_dir=20,
        md_per_dir=20,
        excluded_py=800,
        hidden_py=800,
        docs_only_dirs=20,
        docs_only_md=80,
    ),
    "large": Profile(
        dirs=180,
        py_per_dir=450,
        rs_per_dir=30,
        md_per_dir=30,
        excluded_py=2000,
        hidden_py=2000,
        docs_only_dirs=40,
        docs_only_md=120,
    ),
}

SCENARIOS = {
    "without-excludes": Scenario(
        label="without-excludes",
        no_excludes=False,
        excluded_dir_rows=0,
    ),
    "with-excludes": Scenario(
        label="with-excludes",
        no_excludes=True,
        excluded_dir_rows=1,
    ),
}


def repo_root():
    return pathlib.Path(__file__).resolve().parents[1]


def write_text_once(path, text):
    if path.exists():
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)


def fixture_root(profile_name):
    return FIXTURE_BASE / f"{profile_name}-{VERSION}"


def ensure_fixture(profile_name):
    profile = PROFILES[profile_name]
    root = fixture_root(profile_name)
    marker = root / ".fixture.json"
    expected = {
        "version": VERSION,
        "profile": profile_name,
        "file_count": profile.file_count,
        "py_count": profile.py_count,
    }
    if marker.is_file():
        try:
            if json.loads(marker.read_text()) == expected:
                return root
        except json.JSONDecodeError:
            pass

    root.mkdir(parents=True, exist_ok=True)
    for dir_idx in range(profile.dirs):
        pkg = root / f"pkg{dir_idx:04d}"
        for file_idx in range(profile.py_per_dir):
            write_text_once(
                pkg / f"module{file_idx:04d}.py",
                f"print({dir_idx * profile.py_per_dir + file_idx})\n",
            )
        for file_idx in range(profile.rs_per_dir):
            write_text_once(
                pkg / f"crate{file_idx:04d}.rs",
                f"fn value_{dir_idx}_{file_idx}() -> usize {{ {file_idx} }}\n",
            )
        for file_idx in range(profile.md_per_dir):
            write_text_once(
                pkg / f"note{file_idx:04d}.md",
                f"# note {dir_idx} {file_idx}\n\ntext\n",
            )

    for file_idx in range(profile.excluded_py):
        write_text_once(
            root / "node_modules" / "pkg" / f"ignored{file_idx:04d}.py",
            "print('excluded')\n",
        )
    for file_idx in range(profile.hidden_py):
        write_text_once(
            root / ".hidden" / f"hidden{file_idx:04d}.py",
            "print('hidden')\n",
        )
    for dir_idx in range(profile.docs_only_dirs):
        doc_dir = root / "docs_only" / f"docgroup{dir_idx:04d}"
        for file_idx in range(profile.docs_only_md):
            write_text_once(
                doc_dir / f"page{file_idx:04d}.md",
                f"# docs only {dir_idx} {file_idx}\n",
            )

    marker.write_text(json.dumps(expected, sort_keys=True))
    return root


def count_fixture_files(root):
    files = 0
    py_files = 0
    for _, _, names in os.walk(root):
        for name in names:
            if name == ".fixture.json":
                continue
            files += 1
            if name.endswith(".py"):
                py_files += 1
    return files, py_files


def verify_fixture(profile_name, root):
    profile = PROFILES[profile_name]
    files, py_files = count_fixture_files(root)
    if files != profile.file_count or py_files != profile.py_count:
        raise RuntimeError(
            "fixture count mismatch: "
            f"files={files}/{profile.file_count} "
            f"py_files={py_files}/{profile.py_count}"
        )


def run_checked(cmd, cwd):
    result = subprocess.run(cmd, cwd=cwd, text=True)
    if result.returncode != 0:
        raise SystemExit(result.returncode)


def build_release(root):
    run_checked(["cargo", "build", "--release"], root)


def choose_port():
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def fetch_json(port, path, timeout=120):
    url = f"http://127.0.0.1:{port}{path}"
    start = time.perf_counter()
    with urllib.request.urlopen(url, timeout=timeout) as response:
        body = response.read()
    elapsed_ms = (time.perf_counter() - start) * 1000
    return json.loads(body), len(body), elapsed_ms


def wait_server(port, timeout=30):
    deadline = time.monotonic() + timeout
    last_error = None
    while time.monotonic() < deadline:
        try:
            fetch_json(port, "/_ghrm/tree", timeout=2)
            return
        except (urllib.error.URLError, TimeoutError, json.JSONDecodeError) as err:
            last_error = err
            time.sleep(0.05)
    raise RuntimeError(f"server did not become ready: {last_error}")


def tree_root_entries(port):
    data, body_bytes, elapsed_ms = fetch_json(port, "/_ghrm/tree")
    root = data.get("dirs", {}).get("", {})
    entries = root.get("entries", [])
    return {
        "entries": len(entries),
        "dirs": len(data.get("dirs", {})),
        "bytes": body_bytes,
        "elapsed_ms": elapsed_ms,
    }


def wait_nav_ready(port, timeout):
    deadline = time.monotonic() + timeout
    last = None
    while time.monotonic() < deadline:
        last = tree_root_entries(port)
        if last["entries"] > 0:
            return True, last
        time.sleep(0.05)
    return False, last


def proc_rss_kib(pid):
    status = pathlib.Path("/proc") / str(pid) / "status"
    try:
        for line in status.read_text().splitlines():
            if line.startswith("VmRSS:"):
                return int(line.split()[1])
    except FileNotFoundError:
        return None
    return None


class RssSampler:
    def __init__(self, pid, interval):
        self.pid = pid
        self.interval = interval
        self.start = None
        self.peak = None
        self.stop_event = threading.Event()
        self.thread = threading.Thread(target=self._run, daemon=True)

    def __enter__(self):
        self.start = proc_rss_kib(self.pid)
        self.peak = self.start
        self.thread.start()
        return self

    def __exit__(self, exc_type, exc, tb):
        self.stop_event.set()
        self.thread.join(timeout=1)
        current = proc_rss_kib(self.pid)
        for value in (current, self.start):
            if value is not None and (self.peak is None or value > self.peak):
                self.peak = value

    def _run(self):
        while not self.stop_event.is_set():
            value = proc_rss_kib(self.pid)
            if value is not None and (self.peak is None or value > self.peak):
                self.peak = value
            self.stop_event.wait(self.interval)

    @property
    def delta_kib(self):
        if self.start is None or self.peak is None:
            return None
        return self.peak - self.start


def query_path(params):
    encoded = urllib.parse.urlencode(params)
    return f"/_ghrm/path-search?{encoded}"


def bench_query(proc, port, label, params, phase, sample_interval):
    with RssSampler(proc.pid, sample_interval) as rss:
        data, body_bytes, elapsed_ms = fetch_json(port, query_path(params))
    results = data.get("results", [])
    return {
        "phase": phase,
        "label": label,
        "elapsed_ms": elapsed_ms,
        "rss_start_kib": rss.start,
        "rss_peak_kib": rss.peak,
        "rss_delta_kib": rss.delta_kib,
        "body_bytes": body_bytes,
        "results": len(results),
        "truncated": bool(data.get("truncated")),
        "pending": bool(data.get("pending")),
        "max_rows": data.get("max_rows"),
        "first": results[0].get("display") if results else None,
    }


def start_server(root, fixture, port, max_rows, log_path, scenario):
    binary = root / "target" / "release" / "ghrm"
    log = log_path.open("w")
    config_path = FIXTURE_BASE / "config-py.toml"
    config_path.write_text('[walk]\nextensions = ["py"]\n')
    cmd = [
        str(binary),
        "--bind",
        "127.0.0.1",
        "--port",
        str(port),
        "--config",
        str(config_path),
        "--no-browser",
        "--max-rows",
        str(max_rows),
        "--no-ignore",
    ]
    if scenario.no_excludes:
        cmd.append("--no-excludes")
    cmd.append(str(fixture))
    proc = subprocess.Popen(
        cmd,
        cwd=root,
        stdin=subprocess.DEVNULL,
        stdout=log,
        stderr=subprocess.STDOUT,
        text=True,
    )
    return proc, log


def stop_server(proc, log):
    if proc.poll() is None:
        proc.terminate()
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait(timeout=5)
    log.close()


def print_result(result):
    elapsed = f"{result['elapsed_ms']:.1f}ms"
    rss = result["rss_delta_kib"]
    rss_text = "unknown" if rss is None else f"{rss} KiB"
    body = result["body_bytes"]
    suffix = "+" if result["truncated"] else ""
    pending = " pending" if result["pending"] else ""
    print(
        f"{result['phase']:5s} {result['label']:14s} "
        f"{elapsed:>10s} rss_delta={rss_text:>10s} "
        f"rows={result['results']}{suffix}/{result['max_rows']}{pending} "
        f"bytes={body} first={result['first']}"
    )


def result_for(results, phase, label):
    for result in results:
        if result["phase"] == phase and result["label"] == label:
            return result
    return None


def validate_results(results, profile, scenario, args):
    failures = []
    expected_many = min(args.max_rows, profile.indexed_py_count)
    expected_truncated = profile.indexed_py_count > args.max_rows
    for label in ("many-py-name", "many-py-size", "many-py-lines"):
        result = result_for(results, "warm", label)
        if result is None:
            failures.append(f"missing warm result for {label}")
            continue
        if result["results"] != expected_many:
            failures.append(
                f"{label} returned {result['results']} rows, expected {expected_many}"
            )
        if result["truncated"] != expected_truncated:
            failures.append(
                f"{label} truncated={result['truncated']}, expected {expected_truncated}"
            )
        if result["pending"]:
            failures.append(f"{label} is still pending after nav ready")
        if args.max_warm_ms and result["elapsed_ms"] > args.max_warm_ms:
            failures.append(
                f"{label} took {result['elapsed_ms']:.1f}ms, "
                f"limit is {args.max_warm_ms:.1f}ms"
            )
        rss = result["rss_delta_kib"]
        if args.max_rss_delta_kib and rss is not None and rss > args.max_rss_delta_kib:
            failures.append(
                f"{label} RSS delta was {rss} KiB, "
                f"limit is {args.max_rss_delta_kib} KiB"
            )

    for phase in ("cold", "warm"):
        result = result_for(results, phase, "filtered-dirs")
        if result is None:
            failures.append(f"missing {phase} result for filtered-dirs")
            continue
        if result["results"] != 0:
            failures.append(
                f"{phase} filtered-dirs returned {result['results']} rows, expected 0"
            )
        if result["truncated"]:
            failures.append(f"{phase} filtered-dirs reported truncation")

    for phase in ("cold", "warm"):
        result = result_for(results, phase, "excluded-root")
        if result is None:
            failures.append(f"missing {phase} result for excluded-root")
            continue
        if result["results"] != scenario.excluded_dir_rows:
            failures.append(
                f"{phase} excluded-root returned {result['results']} rows, "
                f"expected {scenario.excluded_dir_rows}"
            )
        if result["truncated"]:
            failures.append(f"{phase} excluded-root reported truncation")

    if failures:
        raise RuntimeError("benchmark validation failed:\n- " + "\n- ".join(failures))


def run_scenario(root, fixture, profile, scenario, args):
    port = choose_port()
    log_path = FIXTURE_BASE / f"server-{args.profile}-{scenario.label}-{port}.log"
    proc, log = start_server(root, fixture, port, args.max_rows, log_path, scenario)
    try:
        wait_server(port)
        before = tree_root_entries(port)
        before_rss = proc_rss_kib(proc.pid)
        cold_likely = before["entries"] == 0

        queries = [
            (
                "many-py-name",
                {
                    "q": ".py",
                    "hidden": "1",
                    "excludes": "1",
                    "ignore": "0",
                    "filter": "0",
                    "sort": "name",
                },
            ),
            (
                "many-py-size",
                {
                    "q": ".py",
                    "hidden": "1",
                    "excludes": "1",
                    "ignore": "0",
                    "filter": "0",
                    "size": "1",
                    "sort": "size",
                },
            ),
            (
                "many-py-lines",
                {
                    "q": ".py",
                    "hidden": "1",
                    "excludes": "1",
                    "ignore": "0",
                    "filter": "0",
                    "lines": "1",
                    "sort": "lines",
                },
            ),
            (
                "filtered-dirs",
                {
                    "q": "docs_only",
                    "hidden": "0",
                    "excludes": "0",
                    "ignore": "0",
                    "filter": "1",
                    "sort": "name",
                },
            ),
            (
                "excluded-root",
                {
                    "q": "node_modules",
                    "hidden": "1",
                    "excludes": "1",
                    "ignore": "0",
                    "filter": "0",
                    "sort": "name",
                },
            ),
        ]

        print(f"scenario={scenario.label} no_excludes={scenario.no_excludes}")
        print(f"fixture={fixture}")
        print(
            f"profile={args.profile} files={profile.file_count} "
            f"py_files={profile.py_count} indexed_py={profile.indexed_py_count} "
            f"max_rows={args.max_rows}"
        )
        print(
            f"tree_before entries={before['entries']} dirs={before['dirs']} "
            f"elapsed={before['elapsed_ms']:.1f}ms rss={before_rss}KiB "
            f"cold_fallback_likely={cold_likely}"
        )

        results = []
        for label, params in queries:
            result = bench_query(
                proc,
                port,
                label,
                params,
                "cold",
                args.sample_interval,
            )
            results.append(result)
            print_result(result)

        nav_ready, after = wait_nav_ready(port, args.nav_timeout)
        after_rss = proc_rss_kib(proc.pid)
        print(
            f"tree_after entries={after['entries'] if after else 'unknown'} "
            f"dirs={after['dirs'] if after else 'unknown'} "
            f"rss={after_rss}KiB nav_ready={nav_ready}"
        )

        for label, params in queries:
            result = bench_query(
                proc,
                port,
                label,
                params,
                "warm",
                args.sample_interval,
            )
            results.append(result)
            print_result(result)

        validate_results(results, profile, scenario, args)

        return {
            "scenario": scenario.label,
            "no_excludes": scenario.no_excludes,
            "fixture": str(fixture),
            "profile": args.profile,
            "file_count": profile.file_count,
            "py_count": profile.py_count,
            "indexed_py_count": profile.indexed_py_count,
            "max_rows": args.max_rows,
            "tree_before": before,
            "tree_after": after,
            "rss_before_kib": before_rss,
            "rss_after_kib": after_rss,
            "cold_fallback_likely": cold_likely,
            "nav_ready": nav_ready,
            "results": results,
            "server_log": str(log_path),
        }
    finally:
        stop_server(proc, log)


def selected_scenarios(raw):
    if raw == "all":
        return list(SCENARIOS.values())
    return [SCENARIOS[raw]]


def run_bench(args):
    root = repo_root()
    fixture = ensure_fixture(args.profile)
    profile = PROFILES[args.profile]
    if not args.skip_fixture_check:
        verify_fixture(args.profile, fixture)
    if not args.skip_build:
        build_release(root)

    summaries = []
    for scenario in selected_scenarios(args.scenario):
        summaries.append(run_scenario(root, fixture, profile, scenario, args))

    if args.json:
        print(json.dumps({"scenarios": summaries}, indent=2, sort_keys=True))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--profile",
        choices=sorted(PROFILES),
        default="medium",
    )
    parser.add_argument("--max-rows", type=int, default=1000)
    parser.add_argument("--skip-build", action="store_true")
    parser.add_argument("--skip-fixture-check", action="store_true")
    parser.add_argument("--json", action="store_true")
    parser.add_argument(
        "--scenario",
        choices=["all", *sorted(SCENARIOS)],
        default="all",
    )
    parser.add_argument("--nav-timeout", type=float, default=60)
    parser.add_argument("--sample-interval", type=float, default=0.005)
    parser.add_argument("--max-warm-ms", type=float, default=0)
    parser.add_argument("--max-rss-delta-kib", type=int, default=0)
    args = parser.parse_args()
    run_bench(args)


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        sys.exit(130)
