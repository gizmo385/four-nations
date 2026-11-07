#!/usr/bin/env bash
# Starts an agent run (docs/design/agent-play.md): a Claude Code session plays one game through
# `fn_agent serve` while you watch it in the normal TUI with `fn_tui --watch`.
#
#   scripts/agent-run.sh [options]               # agent plays headless; watch it here
#   scripts/agent-run.sh --interactive [options] # chat with the agent here; watch elsewhere
#   scripts/agent-run.sh --chunk-turns N [options]  # play as successive short claude sessions
#   scripts/agent-run.sh --scripts [options]     # play by writing and running scripts (#484)
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
#   --prompt TEXT      replaces the default prompt (every chunk, under --chunk-turns or
#                      --scripts, verbatim)
#   --chunk-turns N    plays --turns as successive `claude -p` sessions of N turns each, every
#                      one a fresh process against the same log (`fn_agent serve --resume`,
#                      #341): the context any one of them reads stays about one chunk's worth
#                      instead of growing over the whole run, so looping playtests after a
#                      balance change stays cheap. Each chunk after the first is handed the
#                      previous chunk's own final report as a short, in-game-terms reminder that
#                      it's continuing the same settlement (#313: no mention of sessions, chunks
#                      or context -- only what a player would already know). Headless (no live
#                      watcher; replay the finished log afterwards with `... watch RUN_DIR`), and
#                      not combined with --interactive or --scripts.
#   --scripts          plays by writing and running scripts (docs/design/agent-scripting.md)
#                      instead of acting turn by turn: adds `fn_agent serve`'s own `--scripts`,
#                      and a prompt that teaches the trampoline loop (check state, save a short
#                      script, run it a few turns, widen the budget once it proves itself, note
#                      the outcome and any "SDK gap"). Always chunked by round trips rather than
#                      game turns (--chunk-calls, #341's rule in a new unit: a script can play
#                      many turns in one call, so counting turns doesn't bound a session's own
#                      cost the way it does for MCP-turn play). Headless, and not combined with
#                      --interactive or --chunk-turns.
#   --chunk-calls N    with --scripts, ends a session after about N tool calls (default 30) and
#                      resumes with `--resume`; the next session's prompt lists the library and
#                      notes --resume already restores, rather than a prose report (#341's rule,
#                      in-game terms only).
#   --carry DIR        with --scripts, saves the run's final script library and lab notes to
#                      DIR, and loads whatever's already there into the first session's own
#                      prompt (restored with save_script and notes) -- the "own notes and
#                      scripts, carried between runs" #476 settled on, for playing a fresh game
#                      (a new seed, say) with what the last one learned.
#
# Each run gets its own folder under $FN_AGENT_RUNS (default ~/fn-agent-runs), outside the repo,
# so the agent can't read docs/ or data/. The agent gets only fn_agent's tools: that's the "in-game
# knowledge only" rule (#313), and it's enforced by these claude flags, not by the game. In the
# default mode, quitting the watcher stops the agent too. The agent's own output goes to
# agent.log in the run folder. A --scripts run also gets cost.json (round trips and tokens
# summed from every chunk's own `claude -p --output-format json`, with a USD total) and
# sdk-gaps.md (the notes' own "SDK gap" lines, for the owner to file).
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
chunk_turns=0 prompt_given=0 scripts=0 chunk_calls=30 carry_dir=""
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
    --scripts) scripts=1; shift ;;
    --chunk-calls) chunk_calls="$2"; shift 2 ;;
    --carry) carry_dir="$2"; shift 2 ;;
    -h | --help) sed -n '2,/^set -euo/p' "$0" | sed '$d; s/^# \{0,1\}//'; exit 0 ;;
    *) say "unknown option $1 (see --help)"; exit 2 ;;
  esac
done
command -v claude > /dev/null || { say "claude (Claude Code) isn't on PATH"; exit 1; }
if [[ "$chunk_turns" != 0 && "$interactive" == 1 ]]; then
  say "--interactive and --chunk-turns can't be combined"
  exit 2
fi
if [[ "$scripts" == 1 && "$interactive" == 1 ]]; then
  say "--interactive and --scripts can't be combined"
  exit 2
fi
if [[ "$scripts" == 1 && "$chunk_turns" != 0 ]]; then
  say "--chunk-turns and --scripts can't be combined; --scripts chunks by round trips (--chunk-calls)"
  exit 2
fi
if [[ -n "$carry_dir" && "$scripts" != 1 ]]; then
  say "--carry only applies with --scripts"
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
[[ "$scripts" == 1 ]] && serve_args+=(--scripts)
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

# --scripts' own first prompt (docs/design/agent-scripting.md, "The trampoline"): the same
# in-game loop the design settled on -- check state, write one focused script, run it short to
# see what it does, then widen the budget once it's proven. `calls` is this session's own
# round-trip budget (--chunk-calls); the model is trusted to stop there the same way
# --chunk-turns trusts it to stop after its own turn count.
default_script_prompt() {
  local total="$1" calls="$2"
  printf '%s' "You're playing The Four Nations, a colony game, by writing and running scripts" \
    " through the four-nations tools, rather than acting turn by turn." \
    " Call briefing first, then observe." \
    " Save a short, focused script with save_script for one routine job, then run it with" \
    " run_script for just 3-5 turns to see what it does." \
    " Once a script proves itself, run it for a season, and once that goes well, for a year." \
    " Keep everyone alive and grow the settlement, toward about $total turns in all." \
    " After every run_script call, use notes to record what happened, and a line starting" \
    " \"SDK gap:\" for anything you wished the tools could do." \
    " This session is one chunk of that: after roughly $calls tool calls, stop and summarise" \
    " how it went so the next session can carry on."
}

# A resumed --scripts chunk's own prompt (#341's rule in script-mode terms, #484's acceptance
# criteria: the library and notes rather than a prose report). `--resume` already restores the
# real script library and memory into this session (`fn_agent::tools::rebuild_session`); this
# text is only the same orientation a `scripts {}`/`notes {}` call would give, so the model
# doesn't have to ask for it before carrying on.
scripts_handoff_prompt() {
  local total="$1" done_turns="$2" calls="$3" listing="$4"
  printf '%s\n\n%s\n\n%s' \
    "You're continuing the same settlement from where you left off (turn $done_turns of about $total so far), not starting a new one. Here is your own script library and lab notes:" \
    "$listing" \
    "Carry on the same way: run or improve a script with run_script and save_script, record outcomes and any SDK gap in notes, and let time pass. After roughly $calls more tool calls, stop and summarise how it went."
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

# Replays `log`'s own `script`/`turn`/`wake`/`note` lines, the same ones
# `fn_agent::tools::rebuild_session` reads on `--resume` (docs/design/agent-scripting.md,
# "Memory across sessions"), into one of two shapes:
#   listing  a short, human-readable summary (names, versions, last outcome; no code) for a
#            resumed --scripts chunk's own prompt -- `--resume` already restores the real
#            library and memory, so this is only orientation text, not a restore payload
#   carry    the full library (every saved script's own latest code) plus every note, as JSON
#            -- what `--carry DIR` writes between separate runs that share no log
library_notes() {
  local log="$1" mode="$2"
  python3 - "$log" "$mode" << 'PY'
import json, sys

log, mode = sys.argv[1:]
scripts, notes = {}, []
pending, turns_run = None, 0
try:
    lines = [json.loads(l) for l in open(log) if l.strip()]
except FileNotFoundError:
    lines = []
for obj in lines:
    kind = obj.get("type")
    if kind == "script":
        name, version, code, turns = obj["name"], obj["version"], obj["code"], obj["turns"]
        if version >= 1:
            scripts.setdefault(name, {})["version"] = version
            scripts[name]["code"] = code
        pending = name if (version >= 1 and turns >= 1) else None
        turns_run = 0
    elif kind == "turn":
        turns_run += 1
    elif kind == "wake":
        if pending and pending in scripts:
            scripts[pending]["last_outcome"] = obj.get("reason")
            scripts[pending]["last_turns_run"] = turns_run
        pending, turns_run = None, 0
    elif kind == "note":
        notes.append(obj["text"])

if mode == "carry":
    print(json.dumps({"scripts": scripts, "notes": notes}))
    sys.exit(0)

out = []
if scripts:
    out.append("Scripts:")
    for name, s in scripts.items():
        outcome = s.get("last_outcome")
        if outcome:
            out.append(f"- {name} v{s['version']} (last: {outcome}, {s.get('last_turns_run', 0)} turns)")
        else:
            out.append(f"- {name} v{s['version']} (not run yet)")
else:
    out.append("Scripts: none saved yet.")
out.append("")
if notes:
    out.append("Notes:")
    out.extend(f"- {n}" for n in notes)
else:
    out.append("Notes: none yet.")
print("\n".join(out))
PY
}

# --carry DIR's own save (#484, #476 "option A"): writes this run's final script library and
# lab notes to DIR/handoff.json, read back by `carry_block` on a later run. A no-op with no
# --carry, or with nothing to carry yet.
carry_save() {
  local log="$1" dir="$2"
  [[ -n "$dir" ]] || return 0
  mkdir -p "$dir"
  library_notes "$log" carry > "$dir/handoff.json"
  say "carried scripts and notes to $dir/handoff.json"
}

# --carry DIR's own load: the text a first --scripts prompt is given ahead of
# `default_script_prompt`, asking the model to restore DIR/handoff.json's own scripts (each
# with its exact name and code, via save_script) and notes before doing anything else. Empty
# with no --carry, or with nothing saved there yet (a first run), so it adds nothing to the
# prompt.
carry_block() {
  local dir="$1"
  [[ -n "$dir" && -s "$dir/handoff.json" ]] || return 0
  python3 - "$dir/handoff.json" << 'PY'
import json, sys

data = json.load(open(sys.argv[1]))
scripts, notes = data.get("scripts", {}), data.get("notes", [])
if not scripts and not notes:
    sys.exit(0)

out = ["You have saved work from a previous settlement: your own script library and lab notes,"
       " carried over. Restore them before anything else."]
for name, s in scripts.items():
    out.append(f"\nScript `{name}` (version {s['version']}): call save_script with this exact"
                f" name and code to restore it.\n```\n{s['code']}\n```")
if notes:
    out.append("\nYour lab notes from before (call notes to add them back):\n"
                + "\n".join(f"- {n}" for n in notes))
print("\n".join(out))
PY
}

# Adds one chunk's own `claude -p --output-format json` result (read from `json_path`, a file
# rather than a string so a script's own code in it never has to survive shell quoting) into
# `dir/cost.json`'s running total: round trips (`num_turns`), input/cache/output tokens, and
# USD. Creates the file with zeros first if this is the run's first chunk. Updated after every
# chunk, not just at the end, so an interrupted run still reports what it spent so far.
update_cost() {
  local dir="$1" json_path="$2"
  python3 - "$dir/cost.json" "$json_path" << 'PY'
import json, os, sys

cost_path, json_path = sys.argv[1:]
try:
    data = json.load(open(json_path))
except (FileNotFoundError, json.JSONDecodeError):
    data = {}
usage = data.get("usage", {})
add = {
    "round_trips": data.get("num_turns", 0),
    "input_tokens": usage.get("input_tokens", 0),
    "cache_creation_input_tokens": usage.get("cache_creation_input_tokens", 0),
    "cache_read_input_tokens": usage.get("cache_read_input_tokens", 0),
    "output_tokens": usage.get("output_tokens", 0),
    "usd": data.get("total_cost_usd", 0.0),
}
if os.path.exists(cost_path):
    totals = json.load(open(cost_path))
else:
    totals = dict.fromkeys(add, 0)
for k, v in add.items():
    totals[k] = totals.get(k, 0) + v
with open(cost_path, "w") as f:
    json.dump(totals, f, indent=2)
    f.write("\n")
PY
}

# `dir/sdk-gaps.md` (#484): every lab note whose text mentions an "SDK gap" (the convention
# `default_script_prompt` asks for), pulled from the finished run's own log, for the owner to
# turn into issues.
write_sdk_gaps() {
  local dir="$1"
  python3 - "$dir/run.jsonl" "$dir/sdk-gaps.md" << 'PY'
import json, sys

log_path, out_path = sys.argv[1:]
gaps = []
try:
    for line in open(log_path):
        line = line.strip()
        if not line:
            continue
        obj = json.loads(line)
        if obj.get("type") == "note" and "sdk gap" in obj["text"].lower():
            gaps.append(obj["text"])
except FileNotFoundError:
    pass
with open(out_path, "w") as f:
    f.write("# SDK gaps\n\n")
    if gaps:
        for g in gaps:
            f.write(f"- {g}\n")
    else:
        f.write("None noted this run.\n")
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

# `--scripts`: plays `$turns` as successive `claude -p` sessions chunked by round trips rather
# than game turns (`--chunk-calls`, #484, docs/design/agent-scripting.md): a script can play
# many turns in one tool call, so a session's own cost doesn't bound the way it does under
# `--chunk-turns`. Every chunk but the first resumes `run.jsonl` (`fn_agent serve --scripts
# --resume`), which restores the real script library and memory on its own; the prompt only
# needs to list them, not carry a prose report forward. Ends the same way `run_chunked` does --
# once `turns_done` reaches `$turns`, or after a generous backstop of chunks gives up.
run_scripts_chunked() {
  local target="$turns" calls="$chunk_calls" n=1 done_turns listing chunk_json result
  # Turns per chunk vary a lot with how mature the library is (one call might run a whole
  # season, another might barely finish one turn), so this backstop can't be sized from
  # `calls` the way `run_chunked`'s is from `size`: one extra chunk per turn, plus headroom.
  local max_chunks=$(( target + 10 ))
  say "scripted run: $target turns total, about $calls tool calls per chunk, log at $dir/run.jsonl"
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

    if (( n == 1 )); then
      write_mcp_config "${serve_args[@]}"
    else
      write_mcp_config serve --resume "$dir/run.jsonl" --scripts
    fi

    local this_prompt
    if (( prompt_given )); then
      this_prompt="$prompt"
    elif (( n == 1 )); then
      this_prompt="$(default_script_prompt "$target" "$calls")"
      local carried
      carried="$(carry_block "$carry_dir")"
      [[ -n "$carried" ]] && this_prompt="$carried"$'\n\n'"$this_prompt"
    else
      listing="$(library_notes "$dir/run.jsonl" listing)"
      this_prompt="$(scripts_handoff_prompt "$target" "$done_turns" "$calls" "$listing")"
    fi

    say "chunk $n: about $calls tool calls, past turn $done_turns of $target so far"
    chunk_json="$dir/chunk-$n.json"
    claude -p "$this_prompt" "${claude_args[@]}" --output-format json > "$chunk_json" \
      2>> "$dir/agent.log" || true
    result="$(python3 -c '
import json, sys
try:
    print(json.load(open(sys.argv[1])).get("result", ""))
except (FileNotFoundError, json.JSONDecodeError):
    pass
' "$chunk_json")"
    [[ -n "$result" ]] || result="(this chunk's session ended without a report; see $dir/agent.log)"
    printf '\n--- chunk %s report ---\n%s\n' "$n" "$result" >> "$dir/agent.log"
    update_cost "$dir" "$chunk_json"
    finish_log "chunk $n finished"
    n=$(( n + 1 ))
  done
  write_sdk_gaps "$dir"
  carry_save "$dir/run.jsonl" "$carry_dir"
}

say "run folder: $dir"
cd "$dir"

if (( scripts == 1 )); then
  run_scripts_chunked
  say "scripted run finished; replay it with: $repo/scripts/agent-run.sh watch $dir"
  say "cost: $dir/cost.json; SDK gaps: $dir/sdk-gaps.md"
  exit
fi

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
