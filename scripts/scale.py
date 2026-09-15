#!/usr/bin/env python3
"""Scale command for the CPU query (prints only)."""

from __future__ import annotations

import sys

job = sys.argv[1] if len(sys.argv) > 1 else "api"
print(f"scale: would add capacity for {job}", flush=True)
