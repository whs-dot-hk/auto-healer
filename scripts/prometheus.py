#!/usr/bin/env python3
"""Tiny Prometheus. Scrapes web /health. GET /cpu?v=0.95 to spike CPU."""

from __future__ import annotations

import argparse
import json
import re
import threading
import time
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse

UP = {"web": 0.0}
CPU = {"api": 0.2}
LOCK = threading.Lock()


def scrape(url: str) -> None:
    while True:
        value = 0.0
        try:
            with urllib.request.urlopen(url, timeout=1) as resp:
                if 200 <= resp.status < 300:
                    value = 1.0
        except (urllib.error.URLError, TimeoutError, OSError):
            value = 0.0
        with LOCK:
            UP["web"] = value
        time.sleep(0.35)


def vector(samples: list[float]) -> dict:
    now = time.time()
    return {
        "status": "success",
        "data": {
            "resultType": "vector",
            "result": [{"metric": {}, "value": [now, str(v)]} for v in samples],
        },
    }


def eval_query(q: str) -> dict:
    q = q.strip()
    m = re.search(r'up\{job="([^"]+)"\}\s*==\s*0', q)
    if m:
        job = m.group(1)
        with LOCK:
            up = UP.get(job, 0.0)
        return vector([1] if up == 0.0 else [])

    m = re.search(r'cpu\{job="([^"]+)"\}\s*>\s*([0-9.]+)', q)
    if m:
        job, thr = m.group(1), float(m.group(2))
        with LOCK:
            cpu = CPU.get(job, 0.0)
        return vector([cpu] if cpu > thr else [])

    return vector([])


class Handler(BaseHTTPRequestHandler):
    def log_message(self, fmt: str, *args: object) -> None:
        return

    def do_GET(self) -> None:
        u = urlparse(self.path)
        qs = parse_qs(u.query)
        if u.path == "/api/v1/query":
            q = qs.get("query", [""])[0]
            body = json.dumps(eval_query(q)).encode()
            self.send_response(200)
            self.send_header("content-type", "application/json")
            self.send_header("content-length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        if u.path == "/cpu":
            try:
                v = float(qs.get("v", ["0.2"])[0])
            except ValueError:
                v = 0.2
            with LOCK:
                CPU["api"] = v
            body = f"cpu={v}\n".encode()
            self.send_response(200)
            self.send_header("content-type", "text/plain")
            self.send_header("content-length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        self.send_error(404)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--listen", default="127.0.0.1:19090")
    ap.add_argument("--scrape", default="http://127.0.0.1:18080/health")
    args = ap.parse_args()
    host, port_s = args.listen.rsplit(":", 1)
    threading.Thread(target=scrape, args=(args.scrape,), daemon=True).start()
    httpd = ThreadingHTTPServer((host, int(port_s)), Handler)
    print(f"prometheus mock {host}:{port_s}", flush=True)
    httpd.serve_forever()


if __name__ == "__main__":
    main()
