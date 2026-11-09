//! End-to-end coverage of `cargo xtask agent-run` against a stub `claude` and a stub `cargo`
//! (#507): replaces `scripts/test-agent-run-otel.sh`. The stub `cargo` is a no-op (there's
//! nothing for it to build -- the stub `claude` never touches the real fn_agent/fn_tui
//! binaries), and the stub `claude` logs the `OTEL_RESOURCE_ATTRIBUTES` it was called with and
//! appends a `turn` and a `note` line to whatever log the run's own `mcp.json` points at, so
//! `agent-run`'s own turn-counting and SDK-gap collection both have something real to read.
//!
//! The real `claude` CLI is never invoked here, or by anything these tests run: every `Command`
//! below runs the compiled `xtask` binary directly (not `cargo xtask`, so no real `cargo` is
//! needed either), with a temp directory holding the stubs prepended to `$PATH`.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

fn xtask_bin() -> &'static str {
    env!("CARGO_BIN_EXE_xtask")
}

/// A stub `cargo`: always succeeds without building anything.
const STUB_CARGO: &str = "#!/bin/sh\nexit 0\n";

/// A stub `claude`: logs `$OTEL_RESOURCE_ATTRIBUTES` to `$STUB_LOG` (one line per call, if set),
/// appends one `turn` line and one `note` line (mentioning an "SDK gap", #484's convention) to
/// whatever log `agent-run`'s own `--mcp-config` points its `args` at (`--log` on a fresh run,
/// `--resume` on a resumed one), and, when called with `--output-format json`, prints a
/// `claude -p` result shape to stdout so `agent-run`'s own cost tracking and chunk results have
/// something to parse.
///
/// Knobs (#568), each a list of 1-based call numbers counted in the file `$STUB_COUNT`:
/// `STUB_IDLE_CALLS` write nothing to the log, `STUB_NOJSON_CALLS` print no result,
/// `STUB_NOCOST_CALLS` print one with no `total_cost_usd`, `STUB_EXIT_CALLS` exit 1 (after
/// printing); `STUB_COST` is the cost of a call in dollars (default 0.01).
const STUB_CLAUDE: &str = r#"#!/bin/sh
mcp_config=""
prev=""
output_format_json=0
session=""
# One block per call in $STUB_ARGS (#567): the working directory, then each argument on a line
# of its own, so a test can check what the player was started with and where.
if [ -n "$STUB_ARGS" ]; then
  { echo "PWD=$(pwd)"; for a in "$@"; do echo "ARG=$a"; done; echo "---"; } >> "$STUB_ARGS"
fi
for a in "$@"; do
  if [ "$prev" = "--mcp-config" ]; then
    mcp_config="$a"
  fi
  if [ "$prev" = "--session-id" ]; then
    session="$a"
  fi
  if [ "$prev" = "--output-format" ] && [ "$a" = "json" ]; then
    output_format_json=1
  fi
  prev="$a"
done

call=1
if [ -n "$STUB_COUNT" ]; then
  call=$(( $(cat "$STUB_COUNT" 2>/dev/null || echo 0) + 1 ))
  echo "$call" > "$STUB_COUNT"
fi
in_list() { case " $2 " in *" $1 "*) return 0 ;; esac; return 1; }

log=""
if [ -n "$mcp_config" ] && [ -f "$mcp_config" ]; then
  log=$(grep -A1 -e '"--log"' -e '"--resume"' "$mcp_config" 2>/dev/null \
    | grep -v -e '"--log"' -e '"--resume"' \
    | head -1 \
    | sed 's/^[ \t]*"//; s/",\{0,1\}[ \t]*$//')
fi

if [ -n "$log" ] && ! in_list "$call" "$STUB_IDLE_CALLS"; then
  turns=$(grep -c "\"type\":\"turn\"" "$log" 2>/dev/null)
  turns=${turns:-0}
  next=$((turns + 1))
  tick=$((next * 1000))
  printf '{"type":"turn","index":%d,"tick":%d}\n' "$next" "$tick" >> "$log"
  printf '{"type":"note","text":"SDK gap: stub note %d"}\n' "$next" >> "$log"
fi

# Claude Code files a session's transcript under its config dir, in a folder named for the
# working directory.
if [ -n "$session" ] && [ -z "$STUB_NO_TRANSCRIPT" ] && [ -n "$CLAUDE_CONFIG_DIR" ]; then
  mkdir -p "$CLAUDE_CONFIG_DIR/projects/-stub-cwd"
  printf '{"type":"stub-transcript","session":"%s"}\n' "$session" \
    > "$CLAUDE_CONFIG_DIR/projects/-stub-cwd/$session.jsonl"
fi

if [ -n "$STUB_LOG" ]; then
  echo "$OTEL_RESOURCE_ATTRIBUTES" >> "$STUB_LOG"
fi

if [ "$output_format_json" = "1" ] && ! in_list "$call" "$STUB_NOJSON_CALLS"; then
  if in_list "$call" "$STUB_NOCOST_CALLS"; then
    printf '{"num_turns":1,"result":"stub report, no cost"}\n'
  else
    printf '{"num_turns":1,"usage":{"input_tokens":10,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens":5},"total_cost_usd":%s,"result":"stub report"}\n' "${STUB_COST:-0.01}"
  fi
fi

echo "stub claude: chunk done" >&2
in_list "$call" "$STUB_EXIT_CALLS" && exit 1
exit 0
"#;

struct Fixture {
    tmp: PathBuf,
    bin_dir: PathBuf,
    runs_dir: PathBuf,
}

fn write_executable(path: &Path, contents: &str) {
    fs::write(path, contents).unwrap();
    let mut perms = fs::metadata(path).unwrap().permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        perms.set_mode(0o755);
    }
    fs::set_permissions(path, perms).unwrap();
}

impl Fixture {
    fn new(name: &str) -> Self {
        let tmp = std::env::temp_dir().join(format!(
            "fn_agent_run_it_{name}_{}_{}",
            std::process::id(),
            unique_id()
        ));
        let _ = fs::remove_dir_all(&tmp);
        let bin_dir = tmp.join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        write_executable(&bin_dir.join("cargo"), STUB_CARGO);
        write_executable(&bin_dir.join("claude"), STUB_CLAUDE);
        let runs_dir = tmp.join("runs");
        Self { tmp, bin_dir, runs_dir }
    }

    /// An `xtask agent-run` invocation with the stub `cargo`/`claude` ahead of the real `$PATH`
    /// on it, a fresh `$FN_AGENT_RUNS`, and no caller `OTEL_RESOURCE_ATTRIBUTES` unless the
    /// test sets one.
    fn command(&self) -> Command {
        let mut cmd = Command::new(xtask_bin());
        let real_path = std::env::var("PATH").unwrap_or_default();
        cmd.env("PATH", format!("{}:{real_path}", self.bin_dir.display()));
        cmd.env("FN_AGENT_RUNS", &self.runs_dir);
        cmd.env_remove("OTEL_RESOURCE_ATTRIBUTES");
        // The player's empty working directory goes under $TMPDIR and its transcripts under
        // $CLAUDE_CONFIG_DIR (#567): keep both inside the fixture.
        cmd.env("TMPDIR", self.tmp.join("tmp"));
        cmd.env("CLAUDE_CONFIG_DIR", self.tmp.join("claude-config"));
        cmd.env_remove("STUB_NO_TRANSCRIPT");
        cmd.arg("agent-run");
        cmd
    }

    /// `command()` with a spend cap far above what the stub spends (#568: a model run needs one).
    fn capped(&self) -> Command {
        let mut cmd = self.command();
        cmd.args(["--max-usd", "100"]);
        cmd
    }

    /// A scripted run of `turns` turns against the stub, `extra` options after them. The calls
    /// the stub gets are counted in `$STUB_COUNT` and its arguments recorded in `$STUB_ARGS`.
    fn scripted(&self, turns: &str, extra: &[&str]) -> Command {
        let mut cmd = self.command();
        cmd.args([
            "--scripts",
            "--turns",
            turns,
            "--seed",
            "4",
            "--nation",
            "air",
            "--max-usd",
            "100",
        ]);
        cmd.args(extra);
        cmd.env("STUB_COUNT", self.tmp.join("count"));
        cmd.env("STUB_ARGS", self.tmp.join("calls.txt"));
        cmd
    }

    /// How many times the stub `claude` was called by a `scripted` run.
    fn calls(&self) -> usize {
        fs::read_to_string(self.tmp.join("count")).map_or(0, |t| t.trim().parse().unwrap())
    }

    fn run_dir(&self) -> PathBuf {
        let mut dirs: Vec<PathBuf> = fs::read_dir(&self.runs_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .collect();
        dirs.sort();
        assert_eq!(dirs.len(), 1, "expected exactly one run dir, got {dirs:?}");
        dirs.remove(0)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.tmp);
    }
}

fn unique_id() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    NEXT.fetch_add(1, Ordering::SeqCst)
}

/// `--interactive` makes exactly one `claude` call, tagged with seed/nation/difficulty/mode
/// (#494), on top of whatever the caller's own `OTEL_RESOURCE_ATTRIBUTES` already had.
#[test]
fn interactive_run_makes_one_claude_call_tagged_with_otel_attributes() {
    let fx = Fixture::new("interactive");
    let stub_log = fx.tmp.join("stub.log");
    let status = fx
        .command()
        .args(["--interactive", "--seed", "11", "--nation", "fire", "--difficulty", "hard"])
        .env("STUB_LOG", &stub_log)
        .env("OTEL_RESOURCE_ATTRIBUTES", "caller.tag=keep-me")
        .status()
        .unwrap();
    assert!(status.success());

    let text = fs::read_to_string(&stub_log).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 1, "expected exactly 1 claude call, got {lines:?}");
    let line = lines[0];
    assert!(line.starts_with("caller.tag=keep-me,four_nations.run="), "{line}");
    assert!(
        line.contains(
            ",four_nations.seed=11,four_nations.nation=fire,four_nations.difficulty=hard,\
             four_nations.mode=interactive"
        ),
        "{line}"
    );
    assert!(!line.contains("four_nations.chunk="), "unchunked run shouldn't carry a chunk: {line}");

    let dir = fx.run_dir();
    let log = fs::read_to_string(dir.join("run.jsonl")).unwrap();
    assert!(
        log.lines().any(|l| l.contains("\"type\":\"end\"")),
        "finish_log should add an end line"
    );
}

/// `--chunk-turns 1 --turns 2` makes two `claude -p --output-format json` calls (chunk 1, then
/// chunk 2, since the stub's own fake agent only ever advances one turn per call), and sums
/// both calls' usage into `cost.json`.
#[test]
fn chunk_turns_run_makes_one_claude_call_per_chunk_and_sums_cost_json() {
    let fx = Fixture::new("chunked");
    let stub_log = fx.tmp.join("stub.log");
    let status = fx
        .capped()
        .args([
            "--chunk-turns",
            "1",
            "--turns",
            "2",
            "--seed",
            "3",
            "--nation",
            "water",
            "--difficulty",
            "normal",
        ])
        .env("STUB_LOG", &stub_log)
        .status()
        .unwrap();
    assert!(status.success());

    let text = fs::read_to_string(&stub_log).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2, "expected exactly 2 claude calls, got {lines:?}");
    assert!(
        lines[0].contains("four_nations.mode=mcp-chunked,four_nations.chunk=1"),
        "{}",
        lines[0]
    );
    assert!(
        lines[1].contains("four_nations.mode=mcp-chunked,four_nations.chunk=2"),
        "{}",
        lines[1]
    );

    let dir = fx.run_dir();
    let cost: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(dir.join("cost.json")).unwrap()).unwrap();
    assert_eq!(cost["round_trips"], 2);
    assert!((cost["usd"].as_f64().unwrap() - 0.02).abs() < 1e-9, "{cost}");

    let log = fs::read_to_string(dir.join("run.jsonl")).unwrap();
    assert!(log.lines().any(|l| l.contains("\"type\":\"end\"")));
    let agent_log = fs::read_to_string(dir.join("agent.log")).unwrap();
    assert!(agent_log.contains("chunk 1 report"));
    assert!(agent_log.contains("chunk 2 report"));
}

/// `--scripts` writes `sdk-gaps.md` from the run's own "SDK gap" notes, and `cost.json` the
/// same way `--chunk-turns` does.
#[test]
fn scripts_run_writes_sdk_gaps_and_cost_json() {
    let fx = Fixture::new("scripts");
    let status = fx
        .capped()
        .args([
            "--scripts",
            "--turns",
            "2",
            "--chunk-calls",
            "5",
            "--seed",
            "4",
            "--nation",
            "air",
            "--difficulty",
            "normal",
        ])
        .status()
        .unwrap();
    assert!(status.success());

    let dir = fx.run_dir();
    let gaps = fs::read_to_string(dir.join("sdk-gaps.md")).unwrap();
    assert!(gaps.contains("SDK gap: stub note"), "{gaps}");

    let cost: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(dir.join("cost.json")).unwrap()).unwrap();
    assert_eq!(cost["round_trips"], 2);

    // An ordinary scripted run has no horizon stop and no test tool (#569): those belong to a
    // campaign attempt.
    let mcp = fs::read_to_string(dir.join("mcp.json")).unwrap();
    assert!(!mcp.contains("--horizon") && !mcp.contains("--trial"), "{mcp}");
}

/// `--carry DIR` saves the run's script library and lab notes, readable back by a later run
/// (unit-tested directly in `agent_run::tests`); at this end-to-end level it's enough to check
/// the file lands where documented.
#[test]
fn scripts_run_with_carry_writes_a_handoff_file() {
    let fx = Fixture::new("carry");
    let carry_dir = fx.tmp.join("carry");
    let status = fx
        .capped()
        .args([
            "--scripts",
            "--turns",
            "1",
            "--chunk-calls",
            "5",
            "--carry",
            carry_dir.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(status.success());
    assert!(carry_dir.join("handoff.json").is_file());
}

/// An invalid flag combination is rejected before anything ever runs: no run folder, no
/// `claude` call.
#[test]
fn invalid_flag_combination_fails_before_touching_claude_or_cargo() {
    let fx = Fixture::new("invalid");
    let output = fx.command().args(["--interactive", "--chunk-turns", "3"]).output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("can't be combined"));
    assert!(!fx.runs_dir.exists(), "no run folder should have been created");
}

/// A quick sanity check that the stub `claude` itself behaves the way these tests assume
/// (parses `--mcp-config`, finds `--log`, appends exactly one `turn` line).
#[test]
fn stub_claude_appends_one_turn_line_per_call() {
    let fx = Fixture::new("stub-sanity");
    fs::create_dir_all(&fx.runs_dir).unwrap();
    let log = fx.runs_dir.join("run.jsonl");
    let mcp_config = fx.runs_dir.join("mcp.json");
    // The same shape (and the same one-array-element-per-line layout) `write_mcp_config`'s own
    // `serde_json::to_string_pretty` produces, since that's what the stub's own `grep -A1` is
    // written against.
    let mut f = fs::File::create(&mcp_config).unwrap();
    writeln!(
        f,
        "{{\n  \"mcpServers\": {{\n    \"four-nations\": {{\n      \"args\": [\n        \
         \"serve\",\n        \"--log\",\n        \"{}\"\n      ],\n      \"command\": \
         \"/bin/true\"\n    }}\n  }}\n}}",
        log.display()
    )
    .unwrap();
    let status = Command::new(fx.bin_dir.join("claude"))
        .arg("--mcp-config")
        .arg(&mcp_config)
        .status()
        .unwrap();
    assert!(status.success());
    let text = fs::read_to_string(&log).unwrap();
    assert_eq!(text.lines().filter(|l| l.contains("\"type\":\"turn\"")).count(), 1);
}

/// The calls the stub `claude` recorded in `$STUB_ARGS`: each one's working directory and
/// arguments.
fn recorded_calls(path: &Path) -> Vec<(PathBuf, Vec<String>)> {
    let text = fs::read_to_string(path).unwrap();
    text.split("---\n")
        .filter(|b| !b.trim().is_empty())
        .map(|block| {
            let mut cwd = PathBuf::new();
            let mut args = Vec::new();
            for line in block.lines() {
                if let Some(d) = line.strip_prefix("PWD=") {
                    cwd = PathBuf::from(d);
                } else if let Some(a) = line.strip_prefix("ARG=") {
                    args.push(a.to_string());
                }
            }
            (cwd, args)
        })
        .collect()
}

fn arg_after<'a>(args: &'a [String], flag: &str) -> &'a str {
    let i = args.iter().position(|a| a == flag).unwrap_or_else(|| panic!("no {flag} in {args:?}"));
    &args[i + 1]
}

/// #567: the player runs from an empty directory outside the repo, with no built-in tools, no
/// user/project settings, no skills and no Claude Code prompt, and an absolute MCP config even
/// when `$FN_AGENT_RUNS` is relative. Checked on the arguments `claude` is really started with.
#[test]
fn every_chunk_runs_in_an_empty_sandbox_dir_with_the_clean_flags() {
    let fx = Fixture::new("sandbox");
    let calls_log = fx.tmp.join("calls.txt");
    // A relative runs dir, resolved against the cwd xtask is started from.
    let mut cmd = fx.capped();
    cmd.current_dir(&fx.tmp).env("FN_AGENT_RUNS", "rel-runs");
    let status = cmd
        .args(["--chunk-turns", "1", "--turns", "2", "--seed", "3", "--nation", "water"])
        .env("STUB_ARGS", &calls_log)
        .status()
        .unwrap();
    assert!(status.success());

    let calls = recorded_calls(&calls_log);
    assert_eq!(calls.len(), 2);
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().canonicalize().unwrap();
    let tmp = fx.tmp.join("tmp").canonicalize().unwrap();
    for (n, (cwd, args)) in calls.iter().enumerate() {
        let cwd = cwd.canonicalize().unwrap();
        assert!(cwd.starts_with(&tmp), "call {n} ran in {cwd:?}, not under {tmp:?}");
        assert!(!cwd.starts_with(&repo), "call {n} ran inside the repo: {cwd:?}");
        assert_eq!(fs::read_dir(&cwd).unwrap().count(), 0, "the sandbox dir must stay empty");

        assert_eq!(arg_after(args, "--tools"), "", "no built-in tools: {args:?}");
        assert_eq!(arg_after(args, "--setting-sources"), "local");
        assert!(args.contains(&"--disable-slash-commands".to_string()));
        assert!(args.contains(&"--strict-mcp-config".to_string()));
        assert_eq!(arg_after(args, "--allowedTools"), "mcp__four-nations__*");
        assert!(!arg_after(args, "--system-prompt").is_empty());
        let mcp = Path::new(arg_after(args, "--mcp-config"));
        assert!(mcp.is_absolute() && mcp.is_file(), "{mcp:?}");
        let config = fs::read_to_string(mcp).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&config).unwrap();
        let command = parsed["mcpServers"]["four-nations"]["command"].as_str().unwrap();
        assert!(Path::new(command).is_absolute(), "fn_agent path {command}");
        assert!(config.contains(fx.tmp.join("rel-runs").to_str().unwrap()), "{config}");
    }
    // One empty directory serves every chunk of the run.
    assert_eq!(calls[0].0, calls[1].0);
    let ids: Vec<&str> = calls.iter().map(|(_, a)| arg_after(a, "--session-id")).collect();
    assert_ne!(ids[0], ids[1]);
}

/// #567: each session's transcript lands in the run folder under `transcripts/`, named for its
/// chunk.
#[test]
fn each_chunks_transcript_is_copied_into_the_run_folder() {
    let fx = Fixture::new("transcripts");
    let calls_log = fx.tmp.join("calls.txt");
    let status = fx
        .capped()
        .args(["--chunk-turns", "1", "--turns", "2", "--seed", "3", "--nation", "water"])
        .env("STUB_ARGS", &calls_log)
        .status()
        .unwrap();
    assert!(status.success());

    let dir = fx.run_dir();
    for (n, (_, args)) in recorded_calls(&calls_log).iter().enumerate() {
        let id = arg_after(args, "--session-id");
        let copied = fs::read_to_string(dir.join(format!("transcripts/chunk-{}.jsonl", n + 1)))
            .unwrap_or_else(|e| panic!("chunk {} transcript: {e}", n + 1));
        assert!(copied.contains(id), "chunk {} copied the wrong session: {copied}", n + 1);
    }
}

/// #567: a transcript that can't be found warns and the run carries on to the end.
#[test]
fn a_missing_transcript_warns_and_does_not_fail_the_run() {
    let fx = Fixture::new("no-transcript");
    let output = fx
        .capped()
        .args(["--chunk-turns", "1", "--turns", "2", "--seed", "3", "--nation", "water"])
        .env("STUB_NO_TRANSCRIPT", "1")
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("warning: no transcript found for chunk-1"), "{stderr}");
    assert!(stderr.contains("warning: no transcript found for chunk-2"), "{stderr}");
    let dir = fx.run_dir();
    assert!(!dir.join("transcripts/chunk-1.jsonl").exists());
    assert!(dir.join("cost.json").is_file(), "the run still finished and wrote its cost");
}

/// #567: an interactive session gets the same sandbox, and its transcript is kept too.
#[test]
fn interactive_sessions_are_sandboxed_and_keep_their_transcript() {
    let fx = Fixture::new("interactive-sandbox");
    let calls_log = fx.tmp.join("calls.txt");
    let status = fx
        .command()
        .args(["--interactive", "--seed", "11", "--nation", "fire"])
        .env("STUB_ARGS", &calls_log)
        .status()
        .unwrap();
    assert!(status.success());
    let calls = recorded_calls(&calls_log);
    assert_eq!(calls.len(), 1);
    let (cwd, args) = &calls[0];
    assert!(cwd.starts_with(fx.tmp.join("tmp").canonicalize().unwrap()), "{cwd:?}");
    assert_eq!(arg_after(args, "--tools"), "");
    assert!(fx.run_dir().join("transcripts/interactive.jsonl").is_file());
}

// -------------------------------------------------------------------------------------------
// #568: a mandatory spend cap, a no-progress stop, and chunks that fail loudly
// -------------------------------------------------------------------------------------------

fn stop_json(dir: &Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(dir.join("stop.json")).unwrap()).unwrap()
}

fn cost_json(dir: &Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(dir.join("cost.json")).unwrap()).unwrap()
}

/// The `--max-budget-usd` each recorded `claude` call was given.
fn budgets(calls_log: &Path) -> Vec<f64> {
    recorded_calls(calls_log)
        .iter()
        .map(|(_, args)| arg_after(args, "--max-budget-usd").parse().unwrap())
        .collect()
}

/// A run that calls the model is refused without `--max-usd`, saying how to give one, before a
/// run folder is made or `claude` is started. `--interactive` is the exemption, and refuses a cap.
#[test]
fn a_model_run_without_a_cap_is_refused_before_anything_runs() {
    let fx = Fixture::new("nocap");
    let stub_log = fx.tmp.join("stub.log");
    for args in [&["--seed", "3", "--nation", "fire"][..], &["--scripts"], &["--chunk-turns", "3"]]
    {
        let out = fx.command().args(args).env("STUB_LOG", &stub_log).output().unwrap();
        assert!(!out.status.success(), "{args:?} should be refused");
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.contains("needs a spend cap: add --max-usd N"), "{args:?}: {err}");
        assert!(err.contains("e.g. --max-usd 5"), "{err}");
    }
    let out = fx.command().args(["--scripts", "--max-usd", "0"]).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("more than 0"));
    let out = fx.command().args(["--interactive", "--max-usd", "5"]).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("--interactive"));
    assert!(!fx.runs_dir.exists(), "no run folder should have been created");
    assert!(!stub_log.exists(), "claude must not have been started");
}

/// Every chunk of a scripted run, and of a `--chunk-turns` run, is started with
/// `--max-budget-usd` of what is left of the cap, so `claude` itself stops a chunk that would
/// overspend, not just the loop between chunks.
#[test]
fn every_chunk_gets_the_remaining_budget_as_claudes_own_cap() {
    let fx = Fixture::new("remaining");
    let status = fx
        .scripted("3", &["--chunk-calls", "5", "--max-usd", "1"])
        .env("STUB_COST", "0.25")
        .status()
        .unwrap();
    assert!(status.success());
    assert_eq!(budgets(&fx.tmp.join("calls.txt")), [1.0, 0.75, 0.5]);

    let fx = Fixture::new("remaining-turns");
    let calls_log = fx.tmp.join("calls.txt");
    let status = fx
        .command()
        .args(["--chunk-turns", "1", "--turns", "3", "--max-usd", "2"])
        .env("STUB_COST", "0.5")
        .env("STUB_ARGS", &calls_log)
        .status()
        .unwrap();
    assert!(status.success());
    assert_eq!(budgets(&calls_log), [2.0, 1.5, 1.0]);
}

/// A chunk that ignores its budget (the stub does; `claude` does not) is the worst case for the
/// loop: it is charged what it cost, and the run stops before another chunk starts.
#[test]
fn a_spent_cap_stops_the_run_before_the_next_chunk() {
    let fx = Fixture::new("capstop");
    let out = fx.scripted("10", &["--max-usd", "1"]).env("STUB_COST", "0.6").output().unwrap();
    assert!(out.status.success());
    assert_eq!(fx.calls(), 2, "0.6, then 0.6 into the 0.4 left: over the cap, so no third");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("hit the $1 cap"), "{err}");
    let dir = fx.run_dir();
    assert_eq!(stop_json(&dir)["reason"], "cap");
    assert!((cost_json(&dir)["usd"].as_f64().unwrap() - 1.2).abs() < 1e-9);
}

/// Two chunks in a row that change nothing end an ordinary run, saying why; the reason is in the
/// run folder.
#[test]
fn two_idle_chunks_in_a_row_end_a_scripted_run() {
    let fx = Fixture::new("idle2");
    let out = fx.scripted("10", &[]).env("STUB_IDLE_CALLS", "2 3").output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(fx.calls(), 3, "one turn, then two idle chunks");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("2 chunk(s) in a row changed nothing"), "{err}");
    let stop = stop_json(&fx.run_dir());
    assert_eq!(stop["reason"], "no_progress");
    assert_eq!(stop["chunk"], 3);
    assert_eq!(stop["turns"], 1);
    assert!(stop["detail"].as_str().unwrap().contains("changed nothing"));
}

/// One idle chunk is forgiven when the next one does something, and a finished run says so.
#[test]
fn one_idle_chunk_does_not_end_a_scripted_run() {
    let fx = Fixture::new("idle1");
    let out = fx.scripted("3", &[]).env("STUB_IDLE_CALLS", "2").output().unwrap();
    assert!(out.status.success());
    assert_eq!(fx.calls(), 4, "turn 1, idle, turn 2, turn 3");
    assert_eq!(stop_json(&fx.run_dir())["reason"], "done");
}

/// The same rule on a `--chunk-turns` run.
#[test]
fn two_idle_chunks_in_a_row_end_a_chunk_turns_run() {
    let fx = Fixture::new("idle2-turns");
    let out = fx
        .command()
        .args(["--chunk-turns", "1", "--turns", "10", "--max-usd", "5"])
        .env("STUB_COUNT", fx.tmp.join("count"))
        .env("STUB_IDLE_CALLS", "2 3")
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(fx.calls(), 3);
    assert_eq!(stop_json(&fx.run_dir())["reason"], "no_progress");
}

/// A chunk whose cost can't be read is unknown, not $0: whether `claude` printed nothing, exited
/// nonzero, or printed a result with no cost, the run stops (and fails) at that chunk instead of
/// retrying, and the folder says which chunk's spend is missing.
#[test]
fn a_chunk_with_an_unreadable_cost_stops_the_run() {
    for (name, env, needle) in [
        ("exit0", ("STUB_NOJSON_CALLS", "2"), "claude exited 0 but left no result"),
        ("nocost", ("STUB_NOCOST_CALLS", "2"), "claude exited 0 but left no result"),
        ("exit1", ("STUB_EXIT_CALLS", "2"), "exit status: 1 and left no result with a cost"),
    ] {
        let fx = Fixture::new(&format!("unknown-{name}"));
        let mut cmd = fx.scripted("10", &[]);
        cmd.env(env.0, env.1);
        if name == "exit1" {
            cmd.env("STUB_NOJSON_CALLS", "2");
        }
        let out = cmd.output().unwrap();
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(!out.status.success(), "{name}: {err}");
        assert!(err.contains(needle), "{name}: {err}");
        assert!(err.contains("unknown, not $0"), "{name}: {err}");
        assert_eq!(fx.calls(), 2, "{name}: stopped, no retry");
        let dir = fx.run_dir();
        let cost = cost_json(&dir);
        assert_eq!(cost["unknown_chunks"], serde_json::json!([2]), "{name}");
        assert!((cost["usd"].as_f64().unwrap() - 0.01).abs() < 1e-9, "{name}: {cost}");
        let stop = stop_json(&dir);
        assert_eq!(stop["reason"], "unknown_cost", "{name}");
        assert_eq!(stop["chunk"], 2);
        if name == "exit1" {
            let agent_log = fs::read_to_string(dir.join("agent.log")).unwrap();
            assert!(agent_log.contains("chunk 2 failed: claude ended with exit status: 1"));
        }
    }
}

/// Same for `--chunk-turns`.
#[test]
fn a_chunk_turns_chunk_with_an_unreadable_cost_stops_the_run() {
    let fx = Fixture::new("unknown-turns");
    let out = fx
        .command()
        .args(["--chunk-turns", "1", "--turns", "5", "--max-usd", "5"])
        .env("STUB_COUNT", fx.tmp.join("count"))
        .env("STUB_NOJSON_CALLS", "1")
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(fx.calls(), 1);
    assert_eq!(stop_json(&fx.run_dir())["reason"], "unknown_cost");
}

/// A nonzero exit that still printed a result with its cost is reported and charged, and the run
/// goes on (the cap and the no-progress rule bound what repeats).
#[test]
fn a_nonzero_exit_with_a_known_cost_is_reported_and_charged() {
    let fx = Fixture::new("exit-known");
    let out = fx.scripted("2", &[]).env("STUB_EXIT_CALLS", "1").output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("chunk 1: claude ended with exit status: 1"), "{err}");
    assert_eq!(fx.calls(), 2);
    let dir = fx.run_dir();
    assert!((cost_json(&dir)["usd"].as_f64().unwrap() - 0.02).abs() < 1e-9);
    assert_eq!(stop_json(&dir)["reason"], "done");
    let agent_log = fs::read_to_string(dir.join("agent.log")).unwrap();
    assert!(agent_log.contains("chunk 1 failed: claude ended with exit status: 1"), "{agent_log}");
}
