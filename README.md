# auto-healer

Each `[[queries]]` row is its own loop:

1. wait for **cron**
2. if still in the **quiet window** after the last fire → skip Prometheus
3. fill **`${vars}`** (literal or `command` stdout)
4. run the PromQL **as the condition** (empty vector = healthy, any sample = fire)
5. run **command**; quiet period starts at `debounce` and **doubles** until `debounce_max`
6. empty result later: backoff **resets**

```
auto-healer --config auto-healer.toml
```

## Config

```toml
[prometheus]
url = "http://127.0.0.1:9090"

[[queries]]
name = "restart-web"
query = 'up{job="${job}"} == 0'   # put the threshold in PromQL
cron = "*/30 * * * * *"           # 6 fields = seconds
debounce = "5m"                   # skip Prometheus this long after a fire
debounce_max = "40m"              # cap for 5m → 10m → 20m → 40m
command = "systemctl restart ${unit}"

[queries.vars]
job = "web"                       # literal
unit = { command = "hostname -s" }  # stdout, trimmed; default timeout 10s
```

| field | meaning |
| --- | --- |
| `query` | PromQL instant query; `${var}` each tick |
| `cron` | 5 fields (`min hour …`) or 6 (`sec min hour …`) |
| `debounce` / `debounce_max` | quiet after fire; doubles on repeat fires |
| `command` | `sh -c`; `${var}` each tick |
| `command_timeout` / `cwd` | optional |
| `vars` | `name = "lit"` or `name = { command = "…", timeout = "2s" }` |

Unknown `${name}` skips the tick. In PromQL, quoted `${var}` is escaped; unquoted must be a number, duration, or identifier. The heal command gets the raw value.

`RUST_LOG=debug` for PromQL text every tick.

## Demo

Two queries at once: restart a crashed web process, and “scale” API when CPU is over `scripts/cpu-threshold` (`0.8`). The CPU query still fires while web is in its quiet window.

```bash
python3 scripts/run.py          # unattended
bash scripts/demo.sh            # typed walkthrough
bash scripts/record_demo.sh     # writes docs/demo.mp4
```

<video src="docs/demo.mp4" controls width="720"></video>
