#!/usr/bin/env bash
# PreToolUse hook for Bash: rejects shell loops that poll for another process or a file
# (`until pgrep ...; do sleep`, `while pgrep ...`, `until [ -s file ]`). They match other agents'
# processes, or the loop's own shell, and have spun for up to four hours. Waiting belongs to
# `run_in_background` and its completion notice. See docs/process/work-tracking.md,
# "Waiting on background work".
# Newlines are folded to spaces so a loop written across lines still matches.
cmd="$(jq -r '.tool_input.command // empty' | tr '\n' ' ')"
loop='\b(until|while)\b.*\bdo\b'
if printf '%s' "$cmd" | grep -Eq "$loop" \
  && printf '%s' "$cmd" | grep -Eq '\bpgrep\b|\bsleep\b|\[ -s '; then
  echo "Blocked: polling loop. Run the command in the foreground, or with run_in_background and wait for its completion notice (docs/process/work-tracking.md, \"Waiting on background work\")." >&2
  exit 2
fi
exit 0
