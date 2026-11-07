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
const STUB_CLAUDE: &str = r#"#!/bin/sh
mcp_config=""
prev=""
output_format_json=0
for a in "$@"; do
  if [ "$prev" = "--mcp-config" ]; then
    mcp_config="$a"
  fi
  if [ "$prev" = "--output-format" ] && [ "$a" = "json" ]; then
    output_format_json=1
  fi
  prev="$a"
done

log=""
if [ -n "$mcp_config" ] && [ -f "$mcp_config" ]; then
  log=$(grep -A1 -e '"--log"' -e '"--resume"' "$mcp_config" 2>/dev/null \
    | grep -v -e '"--log"' -e '"--resume"' \
    | head -1 \
    | sed 's/^[ \t]*"//; s/",\{0,1\}[ \t]*$//')
fi

if [ -n "$log" ]; then
  turns=$(grep -c "\"type\":\"turn\"" "$log" 2>/dev/null)
  turns=${turns:-0}
  next=$((turns + 1))
  tick=$((next * 1000))
  printf '{"type":"turn","index":%d,"tick":%d}\n' "$next" "$tick" >> "$log"
  printf '{"type":"note","text":"SDK gap: stub note %d"}\n' "$next" >> "$log"
fi

if [ -n "$STUB_LOG" ]; then
  echo "$OTEL_RESOURCE_ATTRIBUTES" >> "$STUB_LOG"
fi

if [ "$output_format_json" = "1" ]; then
  printf '{"num_turns":1,"usage":{"input_tokens":10,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens":5},"total_cost_usd":0.01,"result":"stub report"}\n'
fi

echo "stub claude: chunk done" >&2
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
        cmd.arg("agent-run");
        cmd
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
        .command()
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
        .command()
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
}

/// `--carry DIR` saves the run's script library and lab notes, readable back by a later run
/// (unit-tested directly in `agent_run::tests`); at this end-to-end level it's enough to check
/// the file lands where documented.
#[test]
fn scripts_run_with_carry_writes_a_handoff_file() {
    let fx = Fixture::new("carry");
    let carry_dir = fx.tmp.join("carry");
    let status = fx
        .command()
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
