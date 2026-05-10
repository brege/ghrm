import json
import os
import socket
import subprocess
import time
import urllib.parse
import urllib.request
from pathlib import Path


REF_SCAN_REPOS = ("fd", "ripgrep", "tokei", "onefetch")
NAV_SCAN_TARGET_ENV = "GHRM_NAV_SCAN_TARGET"
NAV_SCAN_TARGET_CASE = "target"


def path_search_medium():
    root = Path("fixture").resolve()
    if root.exists():
        return str(root)

    for pkg in range(80):
        pkg_dir = root / f"pkg{pkg:04d}"
        pkg_dir.mkdir(parents=True, exist_ok=True)
        for module in range(270):
            path = pkg_dir / f"module{module:04d}.py"
            lines = 4 + ((pkg * 7 + module * 13) % 80)
            body = "\n".join(
                f"def function_{pkg}_{module}_{line}(): return {line}"
                for line in range(lines)
            )
            path.write_text(body + "\n", encoding="utf-8")

    docs = root / "docs"
    docs.mkdir(parents=True, exist_ok=True)
    for idx in range(4800):
        (docs / f"note{idx:04d}.md").write_text("# note\n", encoding="utf-8")

    return str(root)


def nav_scan_cases():
    cases = list(REF_SCAN_REPOS)
    if os.environ.get(NAV_SCAN_TARGET_ENV):
        cases.append(NAV_SCAN_TARGET_CASE)
    return tuple(cases)


def nav_scan_config():
    root = Path(__file__).resolve().parents[2]
    refs = root / "refs"
    missing = [name for name in REF_SCAN_REPOS if not (refs / name / ".git").exists()]
    if missing:
        raise RuntimeError(f"missing ref repositories: {', '.join(missing)}")

    if os.environ.get(NAV_SCAN_TARGET_ENV):
        nav_scan_path(NAV_SCAN_TARGET_CASE)

    config = Path("ref-scan-excludes.toml").resolve()
    config.write_text('[walk]\nexclude_names = [".git"]\n', encoding="utf-8")
    return str(config)


def nav_scan_path(name):
    if name == NAV_SCAN_TARGET_CASE:
        raw = os.environ.get(NAV_SCAN_TARGET_ENV)
        if not raw:
            raise ValueError(name)
        path = Path(raw).expanduser().resolve()
    elif name in REF_SCAN_REPOS:
        path = Path(__file__).resolve().parents[2] / "refs" / name
    else:
        raise ValueError(name)

    if not path.is_dir():
        raise RuntimeError(f"missing nav scan target: {path}")
    return path


def path_search_url(base, sort):
    params = {
        "q": "module",
        "sort": sort,
        "filter": "0",
        "hidden": "1",
        "excludes": "1",
        "size": "1",
        "lines": "1",
    }
    return f"{base}/_ghrm/path-search?{urllib.parse.urlencode(params)}"


def start_server(root, port, extra_args=()):
    env = os.environ.copy()
    env["GHRM_OPEN"] = "0"
    return subprocess.Popen(
        [
            ghrm_binary(),
            "--no-browser",
            "--bind",
            "127.0.0.1",
            "--port",
            str(port),
            "--max-rows",
            "1000",
            *extra_args,
            str(root),
        ],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        env=env,
    )


def stop_server(proc):
    proc.terminate()
    try:
        proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait(timeout=5)


def free_port():
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def wait_for_nav(base):
    deadline = time.monotonic() + 20
    tree_url = f"{base}/_ghrm/tree?filter=0&hidden=1&excludes=1"
    while time.monotonic() < deadline:
        try:
            payload = fetch_json(tree_url)
            if payload.get("ready"):
                return
        except Exception:
            pass
        time.sleep(0.05)
    raise RuntimeError("ghrm nav did not become ready")


def fetch_json(url):
    with urllib.request.urlopen(url, timeout=10) as response:
        return json.loads(response.read().decode("utf-8"))


def ghrm_binary():
    env_dir = os.environ.get("ASV_ENV_DIR")
    if env_dir:
        path = Path(env_dir) / "bin" / "ghrm"
        if path.exists():
            return str(path)
        raise RuntimeError("missing ASV-installed ghrm binary")

    build_dir = os.environ.get("ASV_BUILD_DIR")
    if build_dir:
        path = Path(build_dir) / "target" / "release" / "ghrm"
        if path.exists():
            return str(path)
        raise RuntimeError("missing ASV-built ghrm binary")

    path = Path(__file__).resolve().parents[2] / "target" / "debug" / "ghrm"
    if path.exists():
        return str(path)

    raise RuntimeError("missing ghrm binary")
