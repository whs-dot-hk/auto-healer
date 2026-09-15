#!/usr/bin/env python3
"""Same story as demo.sh, unattended."""

from __future__ import annotations

import os
import signal
import subprocess
import sys
import time
import urllib.error
import urllib.request

ROOT = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(ROOT)
WEB = "http://127.0.0.1:18080"
PROM = "http://127.0.0.1:19090"
KIDS: list[subprocess.Popen] = []


def say(msg: str) -> None:
    print(f"\033[36m# {msg}\033[0m", flush=True)
    time.sleep(2.2)


def spawn(argv: list[str], cwd: str | None = None) -> None:
    print(f"\033[32m$ {' '.join(argv)}\033[0m", flush=True)
    env = os.environ.copy()
    env["PYTHONUNBUFFERED"] = "1"
    env["RUST_LOG"] = env.get("RUST_LOG", "info")
    KIDS.append(subprocess.Popen(argv, cwd=cwd or ROOT, env=env))
    time.sleep(0.8)


def wait_http(url: str, timeout: float = 8) -> None:
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            with urllib.request.urlopen(url, timeout=1):
                return
        except Exception:
            time.sleep(0.1)
    raise SystemExit(f"timeout waiting for {url}")


def get(url: str) -> str:
    try:
        with urllib.request.urlopen(url, timeout=1) as r:
            return r.read().decode().strip() or str(r.status)
    except urllib.error.HTTPError as e:
        return str(e.code)
    except Exception:
        return "down"


def web_state() -> str:
    try:
        with urllib.request.urlopen(f"{WEB}/health", timeout=1) as r:
            return "up" if r.status == 200 else "down"
    except Exception:
        return "down"


def cleanup(*_a: object) -> None:
    for p in KIDS:
        if p.poll() is None:
            p.send_signal(signal.SIGTERM)
    time.sleep(0.2)
    for p in KIDS:
        if p.poll() is None:
            p.kill()
    sys.exit(0)


def main() -> None:
    signal.signal(signal.SIGINT, cleanup)
    signal.signal(signal.SIGTERM, cleanup)

    healer = os.path.join(REPO, "target", "debug", "auto-healer")
    if not os.path.isfile(healer):
        subprocess.check_call(["cargo", "build"], cwd=REPO)

    say("restart-web debounce=6s. scale-api debounce=3s. separate loops.")
    spawn([sys.executable, os.path.join(ROOT, "prometheus.py")])
    spawn([sys.executable, os.path.join(ROOT, "web.py")])
    wait_http(f"{WEB}/health")
    wait_http(f"{PROM}/api/v1/query?query=up")
    print(f"web is {web_state()}", flush=True)
    time.sleep(2.0)

    spawn([healer, "--config", os.path.join(ROOT, "auto-healer.toml")], cwd=REPO)
    time.sleep(3.5)

    say("1) crash web. PromQL up==0 matches, heal.py restarts it.")
    get(f"{WEB}/crash")
    time.sleep(5.0)
    print(f"web is {web_state()}", flush=True)
    time.sleep(3.0)

    say("2) crash again during the 6s quiet window. healer skips Prometheus.")
    get(f"{WEB}/crash")
    time.sleep(0.8)
    print(f"web is {web_state()} (quiet window)", flush=True)
    time.sleep(3.5)
    print(f"web is {web_state()} (still quiet)", flush=True)
    time.sleep(2.5)

    say("3) spike api cpu now. scale-api still fires -- own debounce.")
    print(get(f"{PROM}/cpu?v=0.95"), flush=True)
    time.sleep(5.0)

    say("cpu 0.1 is under 0.8. empty vector, no scale.")
    print(get(f"{PROM}/cpu?v=0.1"), flush=True)
    time.sleep(4.0)

    say("6s window over. second restart (backoff 12s). then healthy resets it.")
    time.sleep(6.0)
    print(f"web is {web_state()}", flush=True)
    time.sleep(4.0)
    say("done.")
    time.sleep(6.0)
    cleanup()


if __name__ == "__main__":
    main()
