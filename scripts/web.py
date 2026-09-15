#!/usr/bin/env python3
"""Demo web. /health is 200 until /crash kills the process."""

from __future__ import annotations

import argparse
import os
import signal
import sys
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

HEALTHY = True


class Handler(BaseHTTPRequestHandler):
    def log_message(self, fmt: str, *args: object) -> None:
        return

    def do_GET(self) -> None:
        global HEALTHY
        if self.path == "/crash":
            HEALTHY = False
            body = b"crashing\n"
            self.send_response(200)
            self.send_header("content-type", "text/plain")
            self.send_header("content-length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            threading.Thread(
                target=lambda: os.kill(os.getpid(), signal.SIGTERM), daemon=True
            ).start()
            return
        if self.path == "/health":
            if HEALTHY:
                self.send_response(200)
                self.send_header("content-length", "3")
                self.end_headers()
                self.wfile.write(b"ok\n")
            else:
                self.send_error(503)
            return
        self.send_error(404)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--listen", default="127.0.0.1:18080")
    args = ap.parse_args()
    host, port_s = args.listen.rsplit(":", 1)
    httpd = ThreadingHTTPServer((host, int(port_s)), Handler)
    print(f"web up pid={os.getpid()} {host}:{port_s}", flush=True)
    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        pass
    sys.exit(0)


if __name__ == "__main__":
    main()
