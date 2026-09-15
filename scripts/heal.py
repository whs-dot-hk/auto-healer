#!/usr/bin/env python3
"""Heal command: bring web.py back if /health is down."""

from __future__ import annotations

import os
import subprocess
import sys
import time
import urllib.request

ROOT = os.path.dirname(os.path.abspath(__file__))
WEB = os.path.join(ROOT, "web.py")
LISTEN = os.environ.get("WEB_LISTEN", "127.0.0.1:18080")
LOG = os.environ.get("HEAL_LOG", os.path.join(ROOT, "heal.log"))


def healthy() -> bool:
    try:
        with urllib.request.urlopen(f"http://{LISTEN}/health", timeout=1) as r:
            return r.status == 200
    except Exception:
        return False


def main() -> None:
    if healthy():
        print("heal: web already up", flush=True)
        return
    logf = open(LOG, "ab")
    proc = subprocess.Popen(
        [sys.executable, WEB, "--listen", LISTEN],
        stdout=logf,
        stderr=subprocess.STDOUT,
        start_new_session=True,
    )
    for _ in range(25):
        time.sleep(0.12)
        if healthy():
            print(f"heal: restarted web pid={proc.pid}", flush=True)
            return
    sys.exit("heal: web did not come back")


if __name__ == "__main__":
    main()
