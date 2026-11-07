#!/usr/bin/env bash
# Checks agent-run.sh's OTEL_RESOURCE_ATTRIBUTES tagging (#494) without a real `claude` run: a
# stub `claude` on PATH dumps the attributes it's called with instead of talking to a model, and
# a stub `cargo` is a no-op (the stub never touches the real fn_agent/fn_tui binaries, so there's
# nothing to build). Covers an --interactive run (one call, no --chunk) and a --chunk-turns run
# (two calls, --chunk 1 then --chunk 2); the stub plays along by appending a real "turn" line to
# the run's own log after each call, so agent-run.sh's own turn-counting ends the chunked run
# after exactly two chunks.
set -euo pipefail
repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

bin="$tmp/bin"
mkdir -p "$bin"

cat > "$bin/cargo" << 'EOF'
#!/usr/bin/env bash
exit 0
EOF
chmod +x "$bin/cargo"

cat > "$bin/claude" << 'EOF'
#!/usr/bin/env python3
# Stub `claude`: logs the OTEL_RESOURCE_ATTRIBUTES it was called with, and (standing in for the
# real agent ending a turn) appends one "turn" line to whatever log agent-run.sh's own
# --mcp-config points at, so a --chunk-turns run's own turn-counting advances.
import json
import os
import sys

args = sys.argv[1:]


def after(flag):
    if flag in args:
        i = args.index(flag)
        if i + 1 < len(args):
            return args[i + 1]
    return None


log_path = None
mcp_config = after("--mcp-config")
if mcp_config and os.path.exists(mcp_config):
    try:
        server_args = json.load(open(mcp_config))["mcpServers"]["four-nations"]["args"]
    except (KeyError, json.JSONDecodeError):
        server_args = []
    if "--log" in server_args:
        log_path = server_args[server_args.index("--log") + 1]
    elif "--resume" in server_args:
        log_path = server_args[server_args.index("--resume") + 1]

if log_path:
    turns = 0
    if os.path.exists(log_path):
        for line in open(log_path):
            line = line.strip()
            if not line:
                continue
            try:
                obj = json.loads(line)
            except json.JSONDecodeError:
                continue
            if obj.get("type") == "turn":
                turns += 1
    with open(log_path, "a") as f:
        f.write(json.dumps({"type": "turn", "index": turns + 1, "tick": (turns + 1) * 1000}) + "\n")

stub_log = os.environ.get("STUB_LOG")
if stub_log:
    with open(stub_log, "a") as f:
        f.write(os.environ.get("OTEL_RESOURCE_ATTRIBUTES", "") + "\n")

print("stub claude: chunk done")
EOF
chmod +x "$bin/claude"

export PATH="$bin:$PATH"
export FN_AGENT_RUNS="$tmp/runs"

fail() { echo "FAIL: $*" >&2; exit 1; }

echo "command -v claude resolves to the stub:"
command -v claude
[[ "$(command -v claude)" == "$bin/claude" ]] || fail "claude didn't resolve to the stub"

echo
echo "-- --interactive run (one call, no --chunk) --"
export STUB_LOG="$tmp/interactive.log"
: > "$STUB_LOG"
# Already set in the caller's own OTEL_RESOURCE_ATTRIBUTES: kept, not replaced (second
# acceptance criterion).
OTEL_RESOURCE_ATTRIBUTES="caller.tag=keep-me" \
  "$repo/scripts/agent-run.sh" --interactive --seed 11 --nation fire --difficulty hard \
  > "$tmp/interactive.out" 2>&1 || fail "--interactive run failed; see $tmp/interactive.out"
unset OTEL_RESOURCE_ATTRIBUTES

[[ "$(wc -l < "$STUB_LOG")" == 1 ]] || fail "expected exactly 1 claude call, got $(cat "$STUB_LOG")"
line="$(cat "$STUB_LOG")"
echo "$line"
case "$line" in
  caller.tag=keep-me,four_nations.run=*,four_nations.seed=11,four_nations.nation=fire,four_nations.difficulty=hard,four_nations.mode=interactive) ;;
  *) fail "unexpected attributes: $line" ;;
esac
case "$line" in *four_nations.chunk=*) fail "unchunked run shouldn't carry four_nations.chunk" ;; esac

echo
echo "-- --chunk-turns run (two calls, --chunk 1 then --chunk 2) --"
export STUB_LOG="$tmp/chunked.log"
: > "$STUB_LOG"
"$repo/scripts/agent-run.sh" --chunk-turns 1 --turns 2 --seed 3 --nation water --difficulty normal \
  > "$tmp/chunked.out" 2>&1 || fail "--chunk-turns run failed; see $tmp/chunked.out"

[[ "$(wc -l < "$STUB_LOG")" == 2 ]] || fail "expected exactly 2 claude calls, got:\n$(cat "$STUB_LOG")"
mapfile -t lines < "$STUB_LOG"
printf '%s\n' "${lines[@]}"
case "${lines[0]}" in
  four_nations.run=*,four_nations.seed=3,four_nations.nation=water,four_nations.difficulty=normal,four_nations.mode=mcp-chunked,four_nations.chunk=1) ;;
  *) fail "chunk 1: unexpected attributes: ${lines[0]}" ;;
esac
case "${lines[1]}" in
  four_nations.run=*,four_nations.seed=3,four_nations.nation=water,four_nations.difficulty=normal,four_nations.mode=mcp-chunked,four_nations.chunk=2) ;;
  *) fail "chunk 2: unexpected attributes: ${lines[1]}" ;;
esac

echo
echo "OK: OTEL_RESOURCE_ATTRIBUTES set for interactive (1 call) and chunked (2 calls, chunk 1 then 2) runs."
