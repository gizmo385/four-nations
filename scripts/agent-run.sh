#!/usr/bin/env bash
# Starts an agent run (docs/design/agent-play.md): a Claude Code session plays one game through
# `fn_agent serve` while you watch it in the normal TUI with `fn_tui --watch`.
#
#   scripts/agent-run.sh [options]               # agent plays headless; watch it here
#   scripts/agent-run.sh --interactive [options] # chat with the agent here; watch elsewhere
#   scripts/agent-run.sh --chunk-turns N [options]  # play as successive short claude sessions
#   scripts/agent-run.sh watch [RUN_DIR]         # watch (or replay) a run; default: the latest
#
# Options:
#   --seed N           map seed (default 7)
#   --nation X         earth, fire, water or air (default earth)
#   --difficulty ID    a data/difficulty.toml preset (default normal)
#   --turns N          how many turns the prompt asks for (default 40)
#   --turn-secs S      game seconds per end_turn (default: fn_agent's own, 20)
#   --pace N           watch pace to start at: 1, 2, 5, 10, 20 or 40 (default 20: a turn's ticks
#                      now play at the watch pace instead of landing all at once (#328), so the
#                      default needs to play a 20-60s turn well under the agent's own ~5s of
#                      thinking between turns -- at x20 that's 1-3s, comfortably inside that --
#                      or the watcher falls behind)
#   --model M          passed to `claude --model`
#   --prompt TEXT      replaces the default prompt (every chunk, under --chunk-turns, verbatim)
#   --chunk-turns N    plays --turns as successive `claude -p` sessions of N turns each, every
#                      one a fresh process against the same log (`fn_agent serve --resume`,
#                      #341): the context any one of them reads stays about one chunk's worth
#                      instead of growing over the whole run, so looping playtests after a
#                      balance change stays cheap. Each chunk after the first is handed the
#                      previous chunk's own final report as a short, in-game-terms reminder that
#                      it's continuing the same settlement (#313: no mention of sessions, chunks
#                      or context -- only what a player would already know). Headless (no live
#                      watcher; replay the finished log afterwards with `... watch RUN_DIR`), and
#                      not combined with --interactive.
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
  watch_run "${dir%/}" 20
  exit
fi

seed=7 nation=earth difficulty=normal turns=40 turn_secs="" pace=20 model="" prompt="" interactive=0
chunk_turns=0 prompt_given=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --seed) seed="$2"; shift 2 ;;
    --nation) nation="$2"; shift 2 ;;
    --difficulty) difficulty="$2"; shift 2 ;;
    --turns) turns="$2"; shift 2 ;;
    --turn-secs) turn_secs="$2"; shift 2 ;;
    --pace) pace="$2"; shift 2 ;;
    --model) model="$2"; shift 2 ;;
    --prompt) prompt="$2"; prompt_given=1; shift 2 ;;
    --chunk-turns) chunk_turns="$2"; shift 2 ;;
    --interactive) interactive=1; shift ;;
    -h | --help) sed -n '2,/^set -euo/p' "$0" | sed '$d; s/^# \{0,1\}//'; exit 0 ;;
    *) say "unknown option $1 (see --help)"; exit 2 ;;
  esac
done
command -v claude > /dev/null || { say "claude (Claude Code) isn't on PATH"; exit 1; }
if [[ "$chunk_turns" != 0 && "$interactive" == 1 ]]; then
  say "--interactive and --chunk-turns can't be combined"
  exit 2
fi

say "building fn_agent and fn_tui (release)"
cargo build -q --release -p fn_agent -p fn_tui --manifest-path "$repo/Cargo.toml"

dir="$runs/$(date +%Y%m%d-%H%M%S)-$nation-$seed-$difficulty"
mkdir -p "$dir"

write_mcp_config() {
  python3 - "$dir/mcp.json" "$repo/target/release/fn_agent" "$@" << 'EOF'
import json, sys
path, command, *args = sys.argv[1:]
config = {"mcpServers": {"four-nations": {"command": command, "args": args}}}
open(path, "w").write(json.dumps(config, indent=2) + "\n")
EOF
}

serve_args=(serve --seed "$seed" --nation "$nation" --difficulty "$difficulty" --log "$dir/run.jsonl")
[[ -n "$turn_secs" ]] && serve_args+=(--turn "$turn_secs")
write_mcp_config "${serve_args[@]}"

default_prompt() {
  local n="$1"
  printf '%s' "You're playing The Four Nations, a colony game, through the four-nations tools." \
    " Call briefing first, then observe. Keep everyone alive and grow the settlement." \
    " Give orders with act (add a short note saying why) and let time pass with end_turn." \
    " Play $n turns, then summarise how it went."
}

# A resumed chunk's own prompt (#313, #341's acceptance criteria: in-game terms only -- no
# mention of sessions, chunks or context, just what a player picking the game back up would
# already know): the previous chunk's own final report, handed back as a short reminder.
handoff_prompt() {
  local n="$1" done_turns="$2" report="$3"
  printf '%s\n\n%s\n\n%s' \
    "You're continuing the same settlement from where you left off (turn $done_turns so far), not starting a new one. Here is your own last report:" \
    "$report" \
    "Call observe to see where things stand now, then carry on: give orders with act (add a short note saying why) and let time pass with end_turn. Play $n more turns, then summarise how it went."
}

[[ -z "$prompt" ]] && prompt="$(default_prompt "$turns")"

claude_args=(--strict-mcp-config --mcp-config "$dir/mcp.json"
  --allowedTools "mcp__four-nations__*"
  --disallowedTools "Bash,Read,Edit,Write,Glob,Grep,WebFetch,WebSearch,NotebookEdit,Agent")
[[ -n "$model" ]] && claude_args+=(--model "$model")

# Claude Code stops its MCP servers with a signal when it exits, so fn_agent rarely gets to
# write the log's `end` line itself. This writes it once claude has exited, so the watcher shows
# "run finished" instead of waiting forever. A log that already ends with one is left alone.
finish_log() {
  sleep 1
  python3 - "$dir/run.jsonl" "$1" << 'PY'
import json, sys
path, reason = sys.argv[1:]
try:
    lines = [json.loads(l) for l in open(path) if l.strip()]
except FileNotFoundError:
    sys.exit(0)
if not lines or lines[-1].get("type") == "end":
    sys.exit(0)
tick = next((l["tick"] for l in reversed(lines) if l.get("type") == "turn"), 0)
with open(path, "a") as f:
    f.write(json.dumps({"type": "end", "tick": tick, "reason": reason}) + "\n")
PY
}

# How many turns `run.jsonl` has completed so far (the highest `turn` line's own `index`, 0 with
# none yet): `--chunk-turns`'s own stopping condition.
turns_done() {
  [[ -s "$dir/run.jsonl" ]] || { echo 0; return; }
  python3 - "$dir/run.jsonl" << 'PY'
import json, sys
turns = []
for line in open(sys.argv[1]):
    line = line.strip()
    if not line:
        continue
    obj = json.loads(line)
    if obj.get("type") == "turn":
        turns.append(obj["index"])
print(max(turns) if turns else 0)
PY
}

# `--chunk-turns N`: plays `$turns` as successive `claude -p` sessions of at most `$chunk_turns`
# turns each, every one but the first resuming `run.jsonl` (`fn_agent serve --resume`, #341)
# instead of starting a fresh game, so no single session's context grows past one chunk's worth.
run_chunked() {
  local target="$turns" size="$chunk_turns" n=1 report="" done_turns remaining this_chunk
  # Generous: the exact chunk count, plus headroom for a chunk that undershoots (an agent error,
  # say) and needs another attempt, so a stuck run still gives up instead of looping forever.
  local max_chunks=$(( (target + size - 1) / size + 5 ))
  say "chunked run: $target turns total, $size per chunk, log at $dir/run.jsonl"
  while true; do
    done_turns="$(turns_done)"
    if (( done_turns >= target )); then
      say "reached turn $done_turns of $target turns; done"
      break
    fi
    if (( n > max_chunks )); then
      say "giving up after $max_chunks chunks (only reached turn $done_turns of $target); see $dir/agent.log"
      break
    fi
    remaining=$(( target - done_turns ))
    this_chunk=$(( remaining < size ? remaining : size ))

    if (( n == 1 )); then
      write_mcp_config "${serve_args[@]}"
    else
      write_mcp_config serve --resume "$dir/run.jsonl"
    fi

    local this_prompt
    if (( prompt_given )); then
      this_prompt="$prompt"
    elif (( n == 1 )); then
      this_prompt="$(default_prompt "$this_chunk")"
    else
      this_prompt="$(handoff_prompt "$this_chunk" "$done_turns" "$report")"
    fi

    say "chunk $n: turns $((done_turns + 1))-$((done_turns + this_chunk)) of $target"
    report="$(claude -p "$this_prompt" "${claude_args[@]}" 2>> "$dir/agent.log")" ||
      report="(this chunk's session ended without a report; see $dir/agent.log)"
    printf '\n--- chunk %s report ---\n%s\n' "$n" "$report" >> "$dir/agent.log"
    finish_log "chunk $n finished"
    n=$(( n + 1 ))
  done
}

say "run folder: $dir"
cd "$dir"

if (( chunk_turns != 0 )); then
  run_chunked
  say "chunked run finished; replay it with: $repo/scripts/agent-run.sh watch $dir"
  exit
fi

if [[ "$interactive" == 1 ]]; then
  say "watch it from another terminal with: $repo/scripts/agent-run.sh watch $dir"
  say "suggested first message: $prompt"
  claude "${claude_args[@]}" || true
  finish_log "agent session closed"
  exit
fi

(
  claude -p "$prompt" "${claude_args[@]}" || true
  finish_log "agent finished"
) > "$dir/agent.log" 2>&1 &
agent=$!
trap 'pkill -P "$agent" 2> /dev/null; kill "$agent" 2> /dev/null; finish_log "stopped by watcher"' EXIT
say "agent running; quit the watcher to stop it"
watch_run "$dir" "$pace"
if kill -0 "$agent" 2> /dev/null; then say "stopping the agent"; else say "agent finished; its summary is in $dir/agent.log"; fi
