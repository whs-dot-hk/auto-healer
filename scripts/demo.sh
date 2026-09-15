#!/usr/bin/env bash
# Typed walkthrough: two queries, vars, 6s debounce, independent loops.
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
export PATH="$ROOT/target/debug:/usr/bin:/bin"
export TERM=xterm-256color
export LANG=C
export LC_ALL=C
export PYTHONUNBUFFERED=1
export RUST_LOG=info
unset PROMPT_COMMAND
PS1=''

fuser -k 18080/tcp 19090/tcp >/dev/null 2>&1 || true

KIDS=()
cleanup() {
  for p in "${KIDS[@]:-}"; do
    kill "$p" 2>/dev/null || true
  done
  fuser -k 18080/tcp 19090/tcp >/dev/null 2>&1 || true
}
trap cleanup EXIT

say() { printf '\033[36m# %s\033[0m\n' "$1"; sleep 2.2; }

slow_type() {
  local s="$1"
  printf '\033[32m$\033[0m %s\n' "$s"
  sleep 1.8
}

web_state() {
  if curl -sf --max-time 1 http://127.0.0.1:18080/health >/dev/null; then
    echo up
  else
    echo down
  fi
}

printf '\033[2J\033[H'
sleep 0.4

say "restart-web debounce=6s. scale-api debounce=3s. separate loops."
slow_type "python3 scripts/prometheus.py &"
python3 scripts/prometheus.py >/tmp/ah-prom.log 2>&1 &
KIDS+=($!)
sleep 0.8
slow_type "python3 scripts/web.py &"
python3 scripts/web.py >/tmp/ah-web.log 2>&1 &
KIDS+=($!)
sleep 1.2

say "web is up."
slow_type "curl -s http://127.0.0.1:18080/health"
curl -s http://127.0.0.1:18080/health
echo
sleep 2.0

say "vars: job from printf web, threshold from scripts/cpu-threshold."
slow_type "auto-healer --config scripts/auto-healer.toml &"
auto-healer --config scripts/auto-healer.toml &
KIDS+=($!)
sleep 3.5

say "1) crash web. PromQL up==0 matches, heal.py restarts it."
slow_type "curl -s http://127.0.0.1:18080/crash"
curl -s http://127.0.0.1:18080/crash || true
echo
sleep 5.0
printf 'web is %s\n' "$(web_state)"
sleep 3.0

say "2) crash again during the 6s quiet window. healer skips Prometheus."
slow_type "curl -s http://127.0.0.1:18080/crash"
curl -s http://127.0.0.1:18080/crash || true
echo
sleep 0.8
printf 'web is %s (quiet window)\n' "$(web_state)"
sleep 3.5
printf 'web is %s (still quiet)\n' "$(web_state)"
sleep 2.5

say "3) spike api cpu now. scale-api still fires -- own debounce."
slow_type "curl -s 'http://127.0.0.1:19090/cpu?v=0.95'"
curl -s 'http://127.0.0.1:19090/cpu?v=0.95'
echo
sleep 5.0

say "cpu 0.1 is under 0.8. empty vector, no scale."
slow_type "curl -s 'http://127.0.0.1:19090/cpu?v=0.1'"
curl -s 'http://127.0.0.1:19090/cpu?v=0.1'
echo
sleep 4.0

say "6s window over. second restart (backoff 12s). then healthy resets it."
sleep 6.0
printf 'web is %s\n' "$(web_state)"
sleep 4.0
say "done."
sleep 6.0
