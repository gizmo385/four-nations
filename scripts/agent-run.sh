#!/usr/bin/env bash
# Starts an agent run (docs/design/agent-play.md): a Claude Code session plays one game through
# `fn_agent serve` while you watch it in the normal TUI with `fn_tui --watch`.
#
#   scripts/agent-run.sh [options]               # agent plays headless; watch it here
#   scripts/agent-run.sh --interactive [options] # chat with the agent here; watch elsewhere
#   scripts/agent-run.sh watch [RUN_DIR]         # watch (or replay) a run; default: the latest
#
# Options:
#   --seed N           map seed (default 7)
#   --nation X         earth, fire, water or air (default earth)
#   --difficulty ID    a data/difficulty.toml preset (default normal)
#   --turns N          how many turns the prompt asks for (default 40)
#   --turn-secs S      game seconds per end_turn (default: fn_agent's own, 20)
#   --pace N           watch pace to start at: 1, 2, 5, 10, 20 or 40 (default 5)
#   --model M          passed to `claude --model`
#   --prompt TEXT      replaces the default prompt
#
# Each run gets its own folder under $FN_AGENT_RUNS (default ~/fn-agent-runs), outside the repo,
# so the agent can't read docs/ or data/. The agent gets only fn_agent's tools: that's the "in-game
# knowledge only" rule (#313), and it's enforced by these claude flags, not by the game. In the
# default mode, quitting the watcher stops the agent too. The agent's own output goes to
# agent.log in the run folder.
set -euo pipefail
repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
runs="${FN_AGENT_RUNS:-$HOME/fn-agent-runs}"
say() { echo "agent-run: $*" >&2; }

watch_run() {
  local dir="$1" pace="$2"
  local log="$dir/run.jsonl"
  # fn_agent writes the log's header as soon as claude starts the server, a few seconds in.
  for _ in $(seq 1 120); do [[ -s "$log" ]] && break; sleep 0.5; done
  [[ -s "$log" ]] || { say "no log at $log yet (did the agent start? see $dir/agent.log)"; return 1; }
  "$repo/target/release/fn_tui" --watch "$log" --pace "$pace"
}

if [[ "${1:-}" == "watch" ]]; then
  dir="${2:-$(ls -1dt "$runs"/*/ 2>/dev/null | head -1)}"
  [[ -n "$dir" ]] || { say "no runs in $runs"; exit 1; }
  cargo build -q --release -p fn_tui --manifest-path "$repo/Cargo.toml"
  watch_run "${dir%/}" 5
  exit
fi

seed=7 nation=earth difficulty=normal turns=40 turn_secs="" pace=5 model="" prompt="" interactive=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --seed) seed="$2"; shift 2 ;;
    --nation) nation="$2"; shift 2 ;;
    --difficulty) difficulty="$2"; shift 2 ;;
    --turns) turns="$2"; shift 2 ;;
    --turn-secs) turn_secs="$2"; shift 2 ;;
    --pace) pace="$2"; shift 2 ;;
    --model) model="$2"; shift 2 ;;
    --prompt) prompt="$2"; shift 2 ;;
    --interactive) interactive=1; shift ;;
    -h | --help) sed -n '2,/^set -euo/p' "$0" | sed '$d; s/^# \{0,1\}//'; exit 0 ;;
    *) say "unknown option $1 (see --help)"; exit 2 ;;
  esac
done
command -v claude > /dev/null || { say "claude (Claude Code) isn't on PATH"; exit 1; }

say "building fn_agent and fn_tui (release)"
cargo build -q --release -p fn_agent -p fn_tui --manifest-path "$repo/Cargo.toml"

dir="$runs/$(date +%Y%m%d-%H%M%S)-$nation-$seed-$difficulty"
mkdir -p "$dir"
serve_args=(serve --seed "$seed" --nation "$nation" --difficulty "$difficulty" --log "$dir/run.jsonl")
[[ -n "$turn_secs" ]] && serve_args+=(--turn "$turn_secs")
python3 - "$dir/mcp.json" "$repo/target/release/fn_agent" "${serve_args[@]}" << 'EOF'
import json, sys
path, command, *args = sys.argv[1:]
config = {"mcpServers": {"four-nations": {"command": command, "args": args}}}
open(path, "w").write(json.dumps(config, indent=2) + "\n")
EOF

if [[ -z "$prompt" ]]; then
  prompt="You're playing The Four Nations, a colony game, through the four-nations tools."
  prompt+=" Call briefing first, then observe. Keep everyone alive and grow the settlement."
  prompt+=" Give orders with act (add a short note saying why) and let time pass with end_turn."
  prompt+=" Play $turns turns, then summarise how it went."
fi

claude_args=(--strict-mcp-config --mcp-config "$dir/mcp.json"
  --allowedTools "mcp__four-nations__*"
  --disallowedTools "Bash,Read,Edit,Write,Glob,Grep,WebFetch,WebSearch,NotebookEdit,Agent")
[[ -n "$model" ]] && claude_args+=(--model "$model")

say "run folder: $dir"
cd "$dir"
if [[ "$interactive" == 1 ]]; then
  say "watch it from another terminal with: $repo/scripts/agent-run.sh watch $dir"
  say "suggested first message: $prompt"
  exec claude "${claude_args[@]}"
fi

claude -p "$prompt" "${claude_args[@]}" > "$dir/agent.log" 2>&1 &
agent=$!
trap 'kill "$agent" 2> /dev/null || true' EXIT
say "agent running (pid $agent); quit the watcher to stop it"
watch_run "$dir" "$pace"
if kill -0 "$agent" 2> /dev/null; then say "stopping the agent"; else say "agent finished; its summary is in $dir/agent.log"; fi
