//! End-to-end coverage of `cargo xtask agent-run --track ID --attempts N` and `--resume DIR`
//! (#530) against a stub `claude`, a stub `cargo` and a stub `fn_agent`: no model, no game.
//!
//! The stub `claude` plays an attempt by appending to the attempt's log what a session would
//! have: a `turn` and a `note` per call, a saved `governor` whose code carries the score the test
//! wants that attempt to earn (`STUB_SCORES`, one per attempt number), and a `plan` line. The
//! stub `fn_agent` answers `score` and `play` from that code: a clean run on seed S scores
//! `SCORE + S`, a played log `SCORE + 1`. So every number in a ledger is predictable, and the
//! owner-only seed (9001) gives a value (`SCORE + 9001`) that must never show up in a prompt.
//! The gate seeds are 101 to 104, so a governor with the same `SCORE` as the champion ties it on
//! every gate seed and one with a higher `SCORE` is ahead by the same amount on all of them.
//!
//! The real `claude` CLI is never invoked here; neither is the real game. The real
//! `fn_agent play`, `score` and `serve --carry-in` are covered in `agent_eval.rs` and in
//! `crates/agent/tests`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

fn xtask_bin() -> &'static str {
    env!("CARGO_BIN_EXE_xtask")
}

const STUB_CARGO: &str = "#!/bin/sh\nexit 0\n";

const STUB_CLAUDE: &str = r#"#!/bin/sh
mcp_config=""
prev=""
output_format_json=0
prompt=""
for a in "$@"; do
  [ "$prev" = "--mcp-config" ] && mcp_config="$a"
  [ "$prev" = "--output-format" ] && [ "$a" = "json" ] && output_format_json=1
  [ "$prev" = "-p" ] && prompt="$a"
  prev="$a"
done

log=""
if [ -n "$mcp_config" ] && [ -f "$mcp_config" ]; then
  log=$(grep -A1 -e '"--log"' -e '"--resume"' "$mcp_config" 2>/dev/null \
    | grep -v -e '"--log"' -e '"--resume"' \
    | head -1 \
    | sed 's/^[ \t]*"//; s/",\{0,1\}[ \t]*$//')
fi

attempt=0
if [ -n "$log" ]; then
  attempt=$(basename "$(dirname "$log")" | sed 's/attempt-//')
  # Which chunk of the attempt this is (0 for the first): the prompts written so far.
  chunk_n=0
  [ -n "$STUB_PROMPTS" ] && chunk_n=$(ls "$STUB_PROMPTS" 2>/dev/null | grep -c "^prompt-$attempt-")
  idle=""
  [ -n "$STUB_IDLE_FIRST" ] && [ "$chunk_n" = 0 ] && idle=1
  stall=""
  [ -n "$STUB_STALL_AT" ] && [ "$chunk_n" = "$STUB_STALL_AT" ] && stall=1
  turns=$(grep -c '"type":"turn"' "$log" 2>/dev/null)
  turns=${turns:-0}
  next=$((turns + 1))
  if [ ! -f "$(dirname "$log")/first-mcp.json" ]; then
    # Later chunks rewrite mcp.json to `serve --resume`; keep the first chunk's.
    cp "$mcp_config" "$(dirname "$log")/first-mcp.json"
    # What `serve --carry-in` would have written before the first call.
    if grep -q -e '--carry-in' "$mcp_config"; then
      printf '{"type":"plan","plan":{"preloaded":true}}\n' >> "$log"
    fi
  fi
  # STUB_STALL_AT=N: chunk N (from 0) changes nothing. STUB_IDLE_FIRST: chunk 0 plays no turn but
  # saves and notes, as a chunk spent editing and testing would.
  if [ -z "$stall" ]; then
    [ -z "$idle" ] && printf '{"type":"turn","index":%d,"tick":%d}\n' "$next" "$((next * 1000))" >> "$log"
    printf '{"type":"note","text":"lab note from attempt %s, call %d"}\n' "$attempt" "$next" >> "$log"
    if [ -z "$STUB_NO_GOVERNOR" ]; then
      score=$(echo "$STUB_SCORES" | cut -d, -f"$attempt")
      # STUB_VERSION_SCORES: the governor saved by chunk N (from 0) of every attempt scores the
      # Nth number, so an attempt can end on a worse version than one it saved before (#570).
      [ -n "$STUB_VERSION_SCORES" ] && score=$(echo "$STUB_VERSION_SCORES" | cut -d, -f"$((chunk_n + 1))")
      printf '{"type":"script","name":"governor","version":%d,"code":"// SCORE=%s%s%s%s","turns":0,"wake_on":[]}\n' "$next" "$score" "${STUB_STOP:+ STOP=$STUB_STOP}" "${STUB_STOP_OWNER:+ OWNERONLY}" "${STUB_INVERT:+ INVERT}" >> "$log"
      printf '{"type":"plan","plan":{"goal":"attempt %s","call":%d}}\n' "$attempt" "$next" >> "$log"
      # STUB_TEST_CHUNKS=0,1: those chunks end with a `test_governor` call.
      case ",$STUB_TEST_CHUNKS," in
        *",$chunk_n,"*) printf '{"type":"call","tool":"test_governor","summary":""}\n' >> "$log" ;;
      esac
    fi
  fi
fi

if [ -n "$STUB_PROMPTS" ]; then
  mkdir -p "$STUB_PROMPTS"
  n=$(ls "$STUB_PROMPTS" | grep -c "^prompt-$attempt-")
  printf '%s\n' "$prompt" > "$STUB_PROMPTS/prompt-$attempt-$n"
fi

# STUB_NOJSON_AT=N: the Nth call of the whole invocation (counted in $STUB_COUNT) exits 1 with
# no result, as a crashed `claude` would (#568).
call=1
if [ -n "$STUB_COUNT" ]; then
  call=$(( $(cat "$STUB_COUNT" 2>/dev/null || echo 0) + 1 ))
  echo "$call" > "$STUB_COUNT"
fi
if [ "$call" = "$STUB_NOJSON_AT" ]; then
  echo "stub claude: crashed" >&2
  exit 1
fi

if [ "$output_format_json" = "1" ]; then
  printf '{"num_turns":1,"usage":{"input_tokens":10,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens":5},"total_cost_usd":0.01,"result":"stub report"}\n'
fi
echo "stub claude: chunk done" >&2
exit 0
"#;

const STUB_FN_AGENT: &str = r#"#!/bin/sh
[ -n "$STUB_FN_AGENT_LOG" ] && echo "$*" >> "$STUB_FN_AGENT_LOG"
cmd="$1"
shift
# The build (#566): `STUB_BUILD` says which this "binary" is, and `STUB_SHIFT` is how far this
# build's sim moves every clean score, so a resume on another build re-scores the champion.
build="${STUB_BUILD:-stub-1}"
shift_by="${STUB_SHIFT:-0}"
case "$cmd" in
build-id)
  [ -n "$STUB_NO_BUILD_ID" ] && { echo "stub fn_agent: no build-id" >&2; exit 2; }
  echo "$build"
  ;;
score)
  log="$1"
  json=0
  append=0
  for a in "$@"; do
    [ "$a" = "--json" ] && json=1
    [ "$a" = "--append" ] && append=1
  done
  s=$(grep -o 'SCORE=[0-9]*' "$log" | tail -1 | cut -d= -f2)
  played=$(( ${s:-0} + 1 ))
  if [ "$append" = 1 ]; then
    printf '{"type":"score","score":%d.0,"progress":0.5,"care":0.9,"horizon":2,"attempt":null,"champion":null,"build":"%s"}\n' "$played" "$build" >> "$log"
  fi
  if [ "$json" = 1 ]; then
    printf '{"score": %d.0}\n' "$played"
  else
    echo "stub scorecard for $log (score $played)"
  fi
  ;;
play)
  script=""; seed=0; logf=""; prev=""
  for a in "$@"; do
    [ "$prev" = "--script" ] && script="$a"
    [ "$prev" = "--seed" ] && seed="$a"
    [ "$prev" = "--log" ] && logf="$a"
    prev="$a"
  done
  s=$(grep -o 'SCORE=[0-9]*' "$script" | tail -1 | cut -d= -f2)
  echo "SCORE=$s seed $seed" > "$logf"
  # `INVERT` in the governor: it does worse on the owner-only seed the better it does elsewhere.
  if grep -q INVERT "$script" && [ "$seed" = 9001 ]; then s=$(( 1000 - s )); fi
  # `STUB_GATE_BAD_ABOVE=N`: a governor with SCORE >= N falls apart on every gate seed (a mean far
  # below the champion's, though its screen scores are higher).
  if [ -n "$STUB_GATE_BAD_ABOVE" ] && [ "${s:-0}" -ge "$STUB_GATE_BAD_ABOVE" ] && [ "$seed" -gt 100 ] && [ "$seed" -lt 9000 ]; then s=$(( s - 2000 )); fi
  # `STUB_COLLAPSE_ABOVE=N`: a governor with SCORE >= N scores 120 on gate seed 104 and nothing
  # else changes: its mean is far ahead and it has one collapse the champion lacks.
  collapse=""
  if [ -n "$STUB_COLLAPSE_ABOVE" ] && [ "${s:-0}" -ge "$STUB_COLLAPSE_ABOVE" ] && [ "$seed" = 104 ]; then collapse=1; fi
  # `STOP=reason:turn` in the governor: the script is stopped on every seed, and the owner-only
  # seed reports a different reason, turn and why that must never reach a prompt.
  stop=$(grep -o 'STOP=[a-z]*:[0-9]*' "$script" | tail -1 | cut -d= -f2)
  by=null
  if [ -n "$stop" ]; then
    if [ "$seed" = 9001 ]; then
      by='{"reason":"error","why":"OWNER_WHY","turn":4242}'
    else
      by="{\"reason\":\"${stop%%:*}\",\"why\":\"a loop with no wait?\",\"turn\":${stop##*:}}"
    fi
  fi
  # `OWNERONLY` in the governor: the script is stopped on the owner-only seed and nowhere else.
  if grep -q OWNERONLY "$script" && [ "$seed" = 9001 ]; then
    by='{"reason":"error","why":"OWNER_WHY","turn":4242}'
  fi
  total=$(( ${s:-0} + seed + shift_by ))
  [ -n "$collapse" ] && total=120
  printf '{"scorecard":{"score":%d.0,"progress":0.5,"care":0.9,"care_detail":{"deaths":0}},"stopped_by":%s}\n' "$total" "$by"
  [ "$by" = null ] || exit 1
  ;;
par)
  # Par (#572): 500 plus the seed's last two digits, with Ready never met. `STUB_PAR_FAIL`: it
  # fails.
  seed=0; horizon=0; prev=""
  for a in "$@"; do
    [ "$prev" = "--seed" ] && seed="$a"
    [ "$prev" = "--horizon" ] && horizon="$a"
    prev="$a"
  done
  [ -n "$STUB_PAR_FAIL" ] && { echo "stub fn_agent: par failed" >&2; exit 3; }
  printf '{"build":"%s","horizon":%d,"score":%d.0,"progress":0.5,"care":1.0,"deaths":0,"cached":false,"milestones":[{"id":"population","name":"Population","kind":"sign","met_turn":100,"met_now":true,"credit":0.8},{"id":"ready","name":"Ready","kind":"ready","met_turn":null,"met_now":false,"credit":0.0}]}\n' "$build" "$horizon" "$((500 + seed % 100))"
  ;;
*)
  echo "stub fn_agent: no $cmd" >&2
  exit 2
  ;;
esac
"#;

/// Screen seeds 7 and 41 (a clean run scores `SCORE + 7` and `SCORE + 41`, mean `SCORE + 24`),
/// gate seeds 101 to 104 (mean `SCORE + 102.5`), owner-only seed 9001.
const TRACKS: &str = r#"
[[track]]
id = "tiny"
name = "Tiny"
nation = "water"
difficulty = "normal"
map_size = "small"
site = "default"
horizon_turns = 2
play_seed = 7
screen_seeds = [7, 41]
gate_seeds = [101, 102, 103, 104]
owner_seeds = [9001]
collapse_below = 150
"#;

fn write_executable(path: &Path, contents: &str) {
    fs::write(path, contents).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).unwrap();
    }
}

struct Fixture {
    tmp: PathBuf,
    runs_dir: PathBuf,
    prompts: PathBuf,
    fn_agent_log: PathBuf,
    /// Extra environment for the stubs (`STUB_STOP`).
    extra_env: Vec<(String, String)>,
    /// Whether a campaign measures par and the do-nothing control (#572). Off by default: the
    /// tests of everything else don't want those calls in the stub's log.
    references: bool,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let tmp = std::env::temp_dir()
            .join(format!("fn_agent_campaign_it_{name}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let bin_dir = tmp.join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        write_executable(&bin_dir.join("cargo"), STUB_CARGO);
        write_executable(&bin_dir.join("claude"), STUB_CLAUDE);
        write_executable(&bin_dir.join("fn_agent"), STUB_FN_AGENT);
        fs::write(tmp.join("tracks.toml"), TRACKS).unwrap();
        Self {
            runs_dir: tmp.join("runs"),
            prompts: tmp.join("prompts"),
            fn_agent_log: tmp.join("fn_agent.log"),
            extra_env: Vec::new(),
            references: false,
            tmp,
        }
    }

    fn bin_dir(&self) -> PathBuf {
        self.tmp.join("bin")
    }

    fn tracks(&self) -> PathBuf {
        self.tmp.join("tracks.toml")
    }

    /// `xtask agent-run ARGS`, with `STUB_SCORES` as the attempts' scores.
    fn agent_run(&self, scores: &str, args: &[&str]) -> Output {
        let real_path = std::env::var("PATH").unwrap_or_default();
        Command::new(xtask_bin())
            .arg("agent-run")
            .args(args)
            .arg("--tracks")
            .arg(self.tracks())
            .env("PATH", format!("{}:{real_path}", self.bin_dir().display()))
            .env("FN_AGENT_RUNS", &self.runs_dir)
            .env("FN_AGENT_FINISH_LOG_SETTLE_MS", "0")
            // The player's sandbox dir and transcripts stay inside the fixture (#567).
            .env("TMPDIR", self.tmp.join("tmp"))
            .env("CLAUDE_CONFIG_DIR", self.tmp.join("claude-config"))
            .env("FN_AGENT_BIN", self.bin_dir().join("fn_agent"))
            // Par and the controls are kept per build (#572): in the fixture, not the owner's.
            .env("FN_AGENT_CACHE", self.tmp.join("cache"))
            .env_remove("STUB_PAR_FAIL")
            .env("STUB_SCORES", scores)
            .env("STUB_PROMPTS", &self.prompts)
            .env("STUB_FN_AGENT_LOG", &self.fn_agent_log)
            .env_remove("STUB_NO_GOVERNOR")
            .env_remove("STUB_VERSION_SCORES")
            .env_remove("STUB_TEST_CHUNKS")
            .env_remove("STUB_INVERT")
            .env_remove("STUB_STOP")
            .env_remove("STUB_STOP_OWNER")
            .env_remove("STUB_GATE_BAD_ABOVE")
            .env_remove("STUB_COLLAPSE_ABOVE")
            .env_remove("STUB_BUILD")
            .env_remove("STUB_SHIFT")
            .env_remove("STUB_NO_BUILD_ID")
            .envs(self.extra_env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .env_remove("OTEL_RESOURCE_ATTRIBUTES")
            .output()
            .unwrap()
    }

    fn campaign(&self, scores: &str, extra: &[&str]) -> Output {
        let mut args = vec!["--track", "tiny", "--model", "stub"];
        args.extend(extra);
        if !self.references {
            args.extend(["--no-par", "--no-controls"]);
        }
        self.agent_run(scores, &with_cap(args))
    }

    fn resume(&self, scores: &str, dir: &Path, extra: &[&str]) -> Output {
        let mut args = vec!["--resume", dir.to_str().unwrap()];
        args.extend(extra);
        if !self.references {
            args.extend(["--no-par", "--no-controls"]);
        }
        self.agent_run(scores, &with_cap(args))
    }

    fn campaign_dir(&self) -> PathBuf {
        let dirs: Vec<PathBuf> = fs::read_dir(&self.runs_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .collect();
        assert_eq!(dirs.len(), 1, "expected exactly one campaign folder, got {dirs:?}");
        assert!(dirs[0].to_string_lossy().contains("-campaign-tiny"), "{dirs:?}");
        dirs[0].clone()
    }

    /// Every prompt `claude` was given, by file name.
    fn prompts(&self) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = fs::read_dir(&self.prompts)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| {
                (
                    e.file_name().to_string_lossy().into_owned(),
                    fs::read_to_string(e.path()).unwrap(),
                )
            })
            .collect();
        out.sort();
        out
    }

    fn prompt(&self, name: &str) -> String {
        self.prompts()
            .into_iter()
            .find(|(n, _)| n == name)
            .unwrap_or_else(|| panic!("no prompt {name} in {:?}", self.prompts()))
            .1
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.tmp);
    }
}

/// A campaign needs a spend cap (#568): `args` with a generous one, unless it has its own.
fn with_cap(mut args: Vec<&str>) -> Vec<&str> {
    if !args.contains(&"--max-usd") {
        args.extend(["--max-usd", "100"]);
    }
    args
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn json(path: &Path) -> Value {
    serde_json::from_str(
        &fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display())),
    )
    .unwrap()
}

fn rows(dir: &Path) -> Vec<Value> {
    json(&dir.join("ledger.json"))["rows"].as_array().unwrap().clone()
}

fn scores(row: &Value, key: &str) -> Vec<f64> {
    row[key].as_array().unwrap().iter().map(|s| s["score"].as_f64().unwrap()).collect()
}

/// The folder layout, the ledger's rows and the champion after two attempts that each beat the
/// last.
#[test]
fn a_campaign_lays_out_attempts_ledger_and_champion() {
    let fx = Fixture::new("layout");
    let out = fx.campaign("100,200", &["--attempts", "2"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let dir = fx.campaign_dir();

    for sub in [
        "ledger.json",
        "champion/handoff.json",
        "champion/meta.json",
        "champion/scorecard.txt",
        "attempt-1/run.jsonl",
        "attempt-1/cost.json",
        "attempt-1/handoff.json",
        // The attempt saved v1 and v2 with the same code but a plan edit between: two pairs, and
        // the later (v2) won the tie.
        "attempt-1/eval/versions.json",
        "attempt-1/eval/v2-2/seed-7.jsonl",
        "attempt-1/eval/v2-2/seed-41.jsonl",
        "attempt-1/eval/v2-2/seed-101.jsonl",
        "attempt-1/eval/v2-2/seed-104.jsonl",
        "attempt-1/eval/v2-2/seed-9001.jsonl",
        "champion/gate.json",
        "attempt-1/chosen-handoff.json",
        "attempt-2/run.jsonl",
    ] {
        assert!(dir.join(sub).is_file(), "missing {sub}");
    }
    assert!(!dir.join("attempt-3").exists());

    let ledger = json(&dir.join("ledger.json"));
    assert_eq!(ledger["track"]["id"], "tiny");
    assert_eq!(ledger["track"]["horizon_turns"], 2);
    let rows = rows(&dir);
    assert_eq!(rows.len(), 2);
    let r1 = &rows[0];
    assert_eq!(r1["attempt"], 1);
    assert_eq!(r1["governor_version"], 2, "the last call saved governor v2");
    assert_eq!(r1["plan_revision"], 2);
    assert_eq!(r1["played"], 101.0);
    assert_eq!(scores(r1, "screen"), [107.0, 141.0]);
    assert_eq!(r1["screen_mean"], 124.0);
    assert_eq!(scores(r1, "gate"), [201.0, 202.0, 203.0, 204.0]);
    assert_eq!(r1["gate_mean"], 202.5);
    assert!(r1["verdict"].is_null(), "the first governor has nothing to be compared with");
    assert_eq!(scores(r1, "owner"), [9101.0]);
    assert_eq!(r1["kept"], true);
    assert!((r1["cost_usd"].as_f64().unwrap() - 0.02).abs() < 1e-9, "{r1}");
    let r2 = &rows[1];
    assert_eq!((r2["attempt"].as_u64(), r2["kept"].as_bool()), (Some(2), Some(true)));
    assert_eq!(r2["screen_mean"], 224.0);
    assert_eq!(r2["gate_mean"], 302.5);
    // Ahead by 100 on every gate seed, so the interval is [100, 100]; the champion is attempt 1.
    let v = &r2["verdict"];
    assert_eq!((v["champion_attempt"].as_u64(), v["promoted"].as_bool()), (Some(1), Some(true)));
    assert_eq!(
        (v["diff"].as_f64(), v["lo"].as_f64(), v["hi"].as_f64()),
        (Some(100.0), Some(100.0), Some(100.0))
    );
    assert_eq!(
        (v["wins"].as_u64(), v["losses"].as_u64(), v["ties"].as_u64()),
        (Some(4), Some(0), Some(0))
    );
    assert_eq!(scores(r2, "owner"), [9201.0], "a promoted candidate runs the owner-only tier");
    // The preloaded plan isn't an edit: the champion's 2, plus this attempt's own 2.
    assert_eq!(r2["plan_revision"], 4);

    let meta = json(&dir.join("champion/meta.json"));
    assert_eq!((meta["attempt"].as_u64(), meta["screen_mean"].as_f64()), (Some(2), Some(224.0)));
    assert_eq!(meta["gate_mean"], 302.5);
    let gate = json(&dir.join("champion/gate.json"));
    assert_eq!(gate["build"], "stub-1");
    assert_eq!(scores(&gate, "scores"), [301.0, 302.0, 303.0, 304.0]);
    let handoff = json(&dir.join("champion/handoff.json"));
    assert_eq!(handoff["scripts"]["governor"]["code"], "// SCORE=200");
    assert_eq!(handoff["plan"]["goal"], "attempt 2");
    let card = fs::read_to_string(dir.join("champion/scorecard.txt")).unwrap();
    assert!(
        card.contains("seed-7.jsonl"),
        "the champion's scorecard is its clean run on play_seed: {card}"
    );

    // The first attempt starts empty; the second preloads the champion.
    let mcp1 = fs::read_to_string(dir.join("attempt-1/first-mcp.json")).unwrap();
    let mcp2 = fs::read_to_string(dir.join("attempt-2/first-mcp.json")).unwrap();
    assert!(!mcp1.contains("--carry-in"), "{mcp1}");
    assert!(mcp2.contains("--carry-in") && mcp2.contains("champion/handoff.json"), "{mcp2}");
    for mcp in [&mcp1, &mcp2] {
        assert!(mcp.contains("\"--scripts\"") && mcp.contains("\"--map-size\""), "{mcp}");
        assert!(mcp.contains("\"7\""), "play_seed 7: {mcp}");
    }

    // The played log ends with its score line.
    let run1 = fs::read_to_string(dir.join("attempt-1/run.jsonl")).unwrap();
    assert!(run1.lines().last().unwrap().contains("\"type\":\"score\""), "{run1}");

    // Every clean run carried the plan. Per attempt: both pairs on the screen seeds first (four
    // runs, in a parallel queue so in any order), then the chosen pair on the gate seeds, then
    // the owner-only seed for it alone. The champion's own gate scores are not run again.
    let calls = fs::read_to_string(&fx.fn_agent_log).unwrap();
    let plays: Vec<&str> = calls.lines().filter(|l| l.starts_with("play")).collect();
    assert_eq!(plays.len(), 18, "{calls}");
    assert!(plays.iter().all(|l| l.contains("--plan") && l.contains("--horizon 2")), "{calls}");
    let seed_of = |l: &str| -> u64 {
        l.split("--seed ").nth(1).unwrap().split_whitespace().next().unwrap().parse().unwrap()
    };
    for attempt in plays.chunks(9) {
        let seeds: Vec<u64> = attempt.iter().map(|l| seed_of(l)).collect();
        assert!(seeds[..4].iter().all(|s| [7, 41].contains(s)), "{seeds:?}");
        assert!(seeds[4..8].iter().all(|s| (101..=104).contains(s)), "{seeds:?}");
        assert_eq!(seeds[8], 9001, "{seeds:?}");
    }

    // The printed ledger is the owner's: the owner-only tier's summary, the real numbers.
    let text = stdout(&out);
    assert!(
        text.contains("owner-only (1 seeds)") && text.contains("9101") && text.contains("9201"),
        "{text}"
    );
    assert!(
        text.contains("screen 7/41") && text.contains("107 / 141") && text.contains("champ"),
        "{text}"
    );
    assert!(text.contains("+100 [+100, +100]"), "the gain and its interval: {text}");
}

/// A worse attempt leaves the champion alone, keeps its notes, and drops its code and plan.
#[test]
fn a_worse_attempt_is_not_kept_but_its_notes_are() {
    let fx = Fixture::new("notkept");
    let out = fx.campaign("200,100,300", &["--attempts", "3"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let dir = fx.campaign_dir();
    let kept: Vec<bool> = rows(&dir).iter().map(|r| r["kept"].as_bool().unwrap()).collect();
    assert_eq!(kept, [true, false, true]);
    assert_eq!(rows(&dir)[1]["screen_mean"], 124.0);
    let v = &rows(&dir)[1]["verdict"];
    assert_eq!(v["promoted"], false, "{v}");
    assert_eq!(v["diff"], -100.0);
    assert!(rows(&dir)[1]["owner"].as_array().unwrap().is_empty(), "no owner-only run for a loser");

    // After attempt 2 alone the champion would still be attempt 1's; run it again to look.
    let fx2 = Fixture::new("notkept2");
    let out = fx2.campaign("200,100", &["--attempts", "2"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let dir2 = fx2.campaign_dir();
    let meta = json(&dir2.join("champion/meta.json"));
    assert_eq!(meta["attempt"], 1);
    let handoff = json(&dir2.join("champion/handoff.json"));
    assert_eq!(handoff["scripts"]["governor"]["code"], "// SCORE=200", "the code is attempt 1's");
    assert_eq!(handoff["plan"]["goal"], "attempt 1", "the plan is attempt 1's");
    let notes: Vec<&str> =
        handoff["notes"].as_array().unwrap().iter().filter_map(|n| n.as_str()).collect();
    assert!(notes.contains(&"lab note from attempt 1, call 1"), "{notes:?}");
    assert!(
        notes.contains(&"lab note from attempt 2, call 2"),
        "attempt 2's notes merged: {notes:?}"
    );
    assert_eq!(rows(&dir2)[1]["plan_revision"], 4, "revisions count even when dropped");

    // A third attempt, after a worse one, still starts from attempt 1's strategy.
    let mcp3 = fs::read_to_string(dir.join("attempt-3/first-mcp.json")).unwrap();
    assert!(mcp3.contains("champion/handoff.json"), "{mcp3}");
    assert_eq!(json(&dir.join("champion/meta.json"))["attempt"], 3);
}

/// Owner-only scores and seeds stay out of every prompt; the later attempts' first prompt carries
/// the screen ledger and the champion's scorecard and asks for a better clean score.
#[test]
fn prompts_carry_the_screen_ledger_and_never_the_owner_tier() {
    let fx = Fixture::new("prompts");
    let out = fx.campaign("200,100", &["--attempts", "2"]);
    assert!(out.status.success(), "{}", stderr(&out));

    let first = fx.prompt("prompt-1-0");
    assert!(first.contains("You're playing The Four Nations"), "{first}");
    // It isn't told it is in a campaign, but it is told what decides the attempt (#569).
    assert!(!first.contains("hill-climbing"), "{first}");
    assert!(first.contains("the clean score") && first.contains("test_governor"), "{first}");

    let second = fx.prompt("prompt-2-0");
    assert!(second.contains("attempt 2 of a hill-climbing campaign"), "{second}");
    assert!(
        second.contains("raise the clean score by improving the governor and the plan"),
        "{second}"
    );
    assert!(
        second.contains("scripts {\"name\": \"governor\"}"),
        "it says how to read the governor: {second}"
    );
    assert!(
        second.contains("Revise the governor you read") && second.contains("small diff"),
        "it asks for a revision, not a rewrite: {second}"
    );
    assert!(second.contains("207 / 241"), "the champion's screen row is in it: {second}");
    assert!(second.contains("vs champ") && second.contains("collapses"), "{second}");
    assert!(second.contains("stub scorecard for"), "the champion's scorecard is in it: {second}");
    assert!(second.contains("You're playing The Four Nations"), "built on the script prompt");

    let all = fx.prompts();
    assert!(all.len() >= 4, "{all:?}");
    for (name, text) in &all {
        for hidden in ["9001", "9201", "9101", "owner", "held-out", "held out"] {
            assert!(!text.contains(hidden), "{hidden:?} leaked into {name}:\n{text}");
        }
    }
    // ... but the owner's table has them.
    assert!(stdout(&out).contains("9201"));
}

/// `--max-usd` stops the loop once it is spent, and prints the ledger it has.
#[test]
fn the_budget_stops_the_loop() {
    let fx = Fixture::new("budget");
    // Each stub call costs $0.01 and an attempt is two calls. Attempt 1 spends $0.02 of the
    // $0.025; attempt 2 gets $0.005, spends one call and the cap is gone.
    let out = fx.campaign("100,200,300,400", &["--attempts", "4", "--max-usd", "0.025"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let dir = fx.campaign_dir();
    assert_eq!(rows(&dir).len(), 2, "{}", stderr(&out));
    assert!(!dir.join("attempt-3").exists());
    assert!(stderr(&out).contains("stopping"), "{}", stderr(&out));
    assert!(stdout(&out).contains("ledger"), "{}", stdout(&out));
}

/// An attempt that never saves a governor has nothing to evaluate: it's recorded, not kept, and
/// the next attempt starts as the first did.
#[test]
fn an_attempt_without_a_governor_is_recorded_and_not_kept() {
    let fx = Fixture::new("nogov");
    let real_path = std::env::var("PATH").unwrap_or_default();
    let out = Command::new(xtask_bin())
        .args(["agent-run", "--track", "tiny", "--attempts", "2", "--max-usd", "100", "--tracks"])
        .arg(fx.tracks())
        .env("PATH", format!("{}:{real_path}", fx.bin_dir().display()))
        .env("FN_AGENT_RUNS", &fx.runs_dir)
        .env("FN_AGENT_FINISH_LOG_SETTLE_MS", "0")
        .env("FN_AGENT_BIN", fx.bin_dir().join("fn_agent"))
        .env("STUB_NO_GOVERNOR", "1")
        .env("STUB_PROMPTS", &fx.prompts)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let dir = fx.campaign_dir();
    let rows = rows(&dir);
    assert_eq!(rows.len(), 2);
    for r in &rows {
        assert!(r["governor_version"].is_null() && r["screen_mean"].is_null(), "{r}");
        assert_eq!((r["screen"].as_array().unwrap().len(), r["kept"].as_bool()), (0, Some(false)));
    }
    assert!(!dir.join("champion").exists());
    let mcp2 = fs::read_to_string(dir.join("attempt-2/first-mcp.json")).unwrap();
    assert!(!mcp2.contains("--carry-in"), "{mcp2}");
    assert!(!fx.prompt("prompt-2-0").contains("hill-climbing"));
}

/// `--resume` numbers new attempts after the last (and after an unfinished attempt's folder, which
/// it leaves alone), keeps appending to the ledger and starts from the current champion.
#[test]
fn resume_continues_numbering_ledger_and_champion() {
    let fx = Fixture::new("resume");
    let out = fx.campaign("100,200", &["--attempts", "2"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let dir = fx.campaign_dir();

    // A crash left attempt 3 half done: no ledger row, so it isn't counted, and it isn't reused.
    fs::create_dir_all(dir.join("attempt-3")).unwrap();
    fs::write(dir.join("attempt-3/marker.txt"), "half done").unwrap();

    let out = fx.resume("100,200,0,150,300", &dir, &["--attempts", "2", "--model", "stub"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let rows = rows(&dir);
    let numbers: Vec<u64> = rows.iter().map(|r| r["attempt"].as_u64().unwrap()).collect();
    assert_eq!(numbers, [1, 2, 4, 5]);
    let kept: Vec<bool> = rows.iter().map(|r| r["kept"].as_bool().unwrap()).collect();
    assert_eq!(kept, [true, true, false, true], "attempt 4 (174) loses to the champion (224)");
    assert_eq!(fs::read_to_string(dir.join("attempt-3/marker.txt")).unwrap(), "half done");
    assert!(!dir.join("attempt-3/run.jsonl").exists());
    assert_eq!(json(&dir.join("champion/meta.json"))["attempt"], 5);

    // It started from the champion, and its prompt was the later-attempt one.
    let mcp4 = fs::read_to_string(dir.join("attempt-4/first-mcp.json")).unwrap();
    assert!(mcp4.contains("champion/handoff.json"), "{mcp4}");
    let p4 = fx.prompt("prompt-4-0");
    assert!(p4.contains("attempt 4 of a hill-climbing campaign"), "{p4}");
    assert!(p4.contains("207 / 241") && p4.contains("224") && !p4.contains("9201"), "{p4}");
    // The ledger it printed has every attempt, old and new.
    let text = stdout(&out);
    for n in ["107 / 141", "207 / 241", "157 / 191", "307 / 341"] {
        assert!(text.contains(n), "{n} in\n{text}");
    }
}

/// `--max-usd` on a resume caps that invocation's spend, not the campaign's.
#[test]
fn resume_budget_caps_only_this_invocation() {
    let fx = Fixture::new("resumebudget");
    let out = fx.campaign("100,200", &["--attempts", "2"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let dir = fx.campaign_dir();
    // The campaign has already spent $0.04, over a $0.015 cap; this invocation hasn't spent any.
    let out = fx.resume("100,200,300,400", &dir, &["--attempts", "3", "--max-usd", "0.015"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let numbers: Vec<u64> = rows(&dir).iter().map(|r| r["attempt"].as_u64().unwrap()).collect();
    assert_eq!(numbers, [1, 2, 3], "one more attempt, then the cap");
}

/// A resume refuses a folder with no ledger, and a track whose horizon or seeds changed, saying
/// why; it plays nothing.
#[test]
fn resume_refuses_what_it_cannot_compare() {
    let fx = Fixture::new("refuse");
    let empty = fx.tmp.join("not-a-campaign");
    fs::create_dir_all(&empty).unwrap();
    let out = fx.resume("1", &empty, &[]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("ledger.json"), "{}", stderr(&out));
    assert!(!empty.join("attempt-1").exists());

    let out = fx.campaign("100", &["--attempts", "1"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let dir = fx.campaign_dir();
    let before = rows(&dir);

    let tracks = fs::read_to_string(fx.tracks()).unwrap();
    let cases = [
        ("horizon_turns = 2", "horizon_turns = 3", "horizon_turns"),
        ("screen_seeds = [7, 41]", "screen_seeds = [7, 40]", "screen_seeds"),
        ("gate_seeds = [101, 102, 103, 104]", "gate_seeds = [101, 102, 103, 105]", "gate_seeds"),
        ("owner_seeds = [9001]", "owner_seeds = [9002]", "owner_seeds"),
        ("collapse_below = 150", "collapse_below = 151", "collapse_below"),
        ("play_seed = 7", "play_seed = 8", "play_seed"),
    ];
    for (from, to, field) in cases {
        fs::write(fx.tracks(), tracks.replace(from, to)).unwrap();
        let out = fx.resume("100", &dir, &["--attempts", "1"]);
        assert!(!out.status.success(), "{field}: {}", stdout(&out));
        let err = stderr(&out);
        assert!(err.contains(field) && err.contains("comparable"), "{field}: {err}");
        assert!(!dir.join("attempt-2").exists(), "{field}: nothing was played");
        assert_eq!(rows(&dir), before);
    }
    // A renamed track is the same track.
    fs::write(fx.tracks(), tracks.replace("name = \"Tiny\"", "name = \"Other\"")).unwrap();
    let out = fx.resume("100,200", &dir, &["--attempts", "1"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(rows(&dir).len(), 2);

    // A track that's gone says so.
    fs::write(fx.tracks(), tracks.replace("id = \"tiny\"", "id = \"other\"")).unwrap();
    let out = fx.resume("100", &dir, &["--attempts", "1"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("tiny"), "{}", stderr(&out));
}

/// Flag combinations that make no sense are refused up front.
#[test]
fn campaign_flags_are_checked() {
    let fx = Fixture::new("flags");
    let refused = |args: &[&str], needle: &str| {
        let out = fx.agent_run("1", &with_cap(args.to_vec()));
        assert!(!out.status.success(), "{args:?} should fail");
        let err = stderr(&out);
        assert!(err.contains(needle), "{args:?}: wanted {needle:?} in {err}");
    };
    refused(&["--track", "tiny"], "--attempts");
    refused(&["--track", "tiny", "--attempts", "0"], "at least 1");
    refused(&["--track", "tiny", "--attempts", "1", "--seed", "3"], "--seed");
    refused(&["--track", "tiny", "--attempts", "1", "--nation", "fire"], "--nation");
    refused(&["--track", "tiny", "--attempts", "1", "--prompt", "x"], "--prompt");
    refused(&["--track", "tiny", "--attempts", "1", "--carry", "d"], "--carry");
    refused(&["--track", "tiny", "--attempts", "1", "--resume", "d"], "pick one");
    refused(&["--track", "nope", "--attempts", "1"], "unknown track");
    refused(&["--attempts", "2"], "--track or --resume");
    assert!(!fx.runs_dir.exists() || fs::read_dir(&fx.runs_dir).unwrap().next().is_none());
}

/// A clean run whose governor was stopped (#561): the ledger keeps the reason, turn and why, the
/// next prompt says so for the screen seeds (and that a stop means coasting), the owner's table
/// marks it, and a gate or owner-only run's stop shows up nowhere the model reads. Every chunk of
/// an attempt, resumed ones too, offers `test_governor` on the screen seeds only.
#[test]
fn stopped_clean_runs_are_reported_to_the_model_and_marked_for_the_owner() {
    let mut fx = Fixture::new("stopped");
    fx.extra_env.push(("STUB_STOP".to_owned(), "runaway:1".to_owned()));
    let out = fx.campaign("100,200", &["--attempts", "2"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let dir = fx.campaign_dir();

    let rows = rows(&dir);
    let first = &rows[0]["screen"][0];
    assert_eq!(first["stopped"], "runaway");
    assert_eq!(first["stopped_turn"], 1);
    assert_eq!(first["stopped_why"], "a loop with no wait?");
    assert_eq!(rows[0]["owner"][0]["stopped_turn"], 4242, "the owner's ledger keeps it");
    assert_eq!(rows[0]["gate"][0]["stopped"], "runaway", "the gate's keep the stop too");

    let second = fx.prompt("prompt-2-0");
    assert!(
        second.contains(
            "runaway stopped 2 of 2 clean runs at turn 1; first (seed 7): \"a loop with no wait?\""
        ),
        "{second}"
    );
    assert!(second.contains("107* / 141*"), "{second}");
    assert!(second.contains("coasts to the end with no orders"), "{second}");
    // The first attempt's prompt has no ledger but does say what a stop costs.
    let first_prompt = fx.prompt("prompt-1-0");
    assert!(first_prompt.contains("test_governor") && first_prompt.contains("coasts"));

    for (name, text) in &fx.prompts() {
        for hidden in ["OWNER_WHY", "4242", "9001", "9101", "9201"] {
            assert!(!text.contains(hidden), "{hidden:?} leaked into {name}:\n{text}");
        }
    }
    let table = stdout(&out);
    assert!(table.contains("107* / 141*") && table.contains("stopped 1"), "{table}");

    // The tool is offered on every chunk, on the screen seeds only.
    let first_mcp = fs::read_to_string(dir.join("attempt-1/first-mcp.json")).unwrap();
    let last_mcp = fs::read_to_string(dir.join("attempt-1/mcp.json")).unwrap();
    assert!(last_mcp.contains("--resume"), "the last chunk is a resumed one: {last_mcp}");
    for mcp in [&first_mcp, &last_mcp] {
        assert!(mcp.contains("\"--trial-seeds\"") && mcp.contains("\"7,41\""), "{mcp}");
        assert!(mcp.contains("\"--trial-turns\""), "{mcp}");
        // Whole numbers among the arguments, not substrings of the text: the config holds temp
        // paths whose process ids and timestamps can contain these digits (#588, #593).
        let numbers: Vec<&str> = mcp
            .split('"')
            .skip(1)
            .step_by(2)
            .filter(|arg| !arg.is_empty() && arg.chars().all(|c| c.is_ascii_digit() || c == ','))
            .flat_map(|arg| arg.split(','))
            .collect();
        for hidden in ["9001", "101", "102", "103", "104"] {
            assert!(!numbers.contains(&hidden), "{hidden} in {mcp}");
        }
    }
}

/// A stop on an owner-only seed alone is the owner's: the table counts it, but the prompt's ledger
/// has no `*` and no footnote, nor anything else of that tier.
#[test]
fn an_owner_only_stop_reaches_the_owner_and_not_the_prompt() {
    let mut fx = Fixture::new("ownerstop");
    fx.extra_env.push(("STUB_STOP_OWNER".to_owned(), "1".to_owned()));
    let out = fx.campaign("100,200", &["--attempts", "2"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let rows = rows(&fx.campaign_dir());
    assert_eq!(rows[0]["screen"][0]["stopped"], Value::Null, "the screen runs ran clean");
    assert_eq!(rows[0]["gate"][0]["stopped"], Value::Null, "the gate runs ran clean");
    assert_eq!(rows[0]["owner"][0]["stopped"], "error");

    let table = stdout(&out);
    assert!(table.contains("9101 · collapse 0/1 · deaths 0 · stopped 1"), "{table}");
    assert!(!table.contains("OWNER_WHY"), "{table}");

    let second = fx.prompt("prompt-2-0");
    assert!(second.contains("The ledger so far"), "{second}");
    assert!(!second.contains("the governor stopped"), "{second}");
    for (name, text) in &fx.prompts() {
        assert!(
            !text.contains("the governor stopped")
                && !text.contains("107*")
                && !text.contains("OWNER_WHY"),
            "a stop in {name}:\n{text}"
        );
    }
}

/// A ledger written before #561 (`stopped` is the reason alone) resumes, and its stops read
/// without a turn or a why.
#[test]
fn an_old_ledger_with_only_stop_reasons_still_resumes() {
    let fx = Fixture::new("oldledger");
    let out = fx.campaign("100", &["--attempts", "1"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let dir = fx.campaign_dir();
    let mut ledger = json(&dir.join("ledger.json"));
    for seed in ledger["rows"][0]["screen"].as_array_mut().unwrap() {
        seed["stopped"] = Value::String("runaway".to_owned());
    }
    fs::write(dir.join("ledger.json"), serde_json::to_string_pretty(&ledger).unwrap()).unwrap();

    let out = fx.resume("100,200", &dir, &["--attempts", "1", "--model", "stub"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let p = fx.prompt("prompt-2-0");
    assert!(p.contains("attempt 1: runaway stopped 2 of 2 clean runs\n"), "{p}");
    assert_eq!(rows(&dir).len(), 2);
}

/// The live game stops at the track's horizon on every chunk's serve line, the first and the
/// resumed ones (#569), and the chunk loop ends the attempt there.
#[test]
fn every_chunk_stops_live_play_at_the_horizon() {
    let fx = Fixture::new("horizon");
    let out = fx.campaign("100", &["--attempts", "1"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let dir = fx.campaign_dir();
    let first_mcp = fs::read_to_string(dir.join("attempt-1/first-mcp.json")).unwrap();
    let last_mcp = fs::read_to_string(dir.join("attempt-1/mcp.json")).unwrap();
    assert!(last_mcp.contains("--resume"), "{last_mcp}");
    for mcp in [&first_mcp, &last_mcp] {
        let flat: String = mcp.split_whitespace().collect();
        assert!(flat.contains("\"--horizon\",\"2\""), "{mcp}");
    }
    assert!(stderr(&out).contains("reached turn 2 of 2"), "{}", stderr(&out));
}

/// Every chunk of every attempt, attempt 1's first prompt and the handoffs included, says what
/// decides the attempt and how to test it, and none of them has owner-only data in it (#569).
#[test]
fn every_chunk_prompt_says_what_decides_the_attempt() {
    let fx = Fixture::new("decides");
    let out = fx.campaign("100,200", &["--attempts", "2"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let all = fx.prompts();
    let names: Vec<&str> = all.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["prompt-1-0", "prompt-1-1", "prompt-2-0", "prompt-2-1"], "{names:?}");
    for (name, text) in &all {
        for phrase in ["the clean score", "from turn 0", "test_governor", "several other maps"] {
            assert!(text.contains(phrase), "{phrase:?} missing from {name}:\n{text}");
        }
        for hidden in ["9001", "9101", "9201", "owner", "held-out", "held out"] {
            assert!(!text.contains(hidden), "{hidden:?} leaked into {name}:\n{text}");
        }
    }
    // A handoff says where the live game is, and is the continuing prompt, not the first.
    let handoff = fx.prompt("prompt-1-1");
    assert!(handoff.contains("The live game is at turn 1 of 2"), "{handoff}");
    assert!(handoff.contains("not starting a new one"), "{handoff}");
}

/// A chunk that played no turn (it spent itself editing and testing) is the same attempt carrying
/// on: the next chunk's handoff says so and says what has been done, never "turn 0 so far", and
/// the attempt isn't restarted or cut short (#569).
#[test]
fn a_chunk_with_no_turns_played_continues_the_same_attempt() {
    let mut fx = Fixture::new("idle");
    fx.extra_env.push(("STUB_IDLE_FIRST".to_owned(), "1".to_owned()));
    let out = fx.campaign("100", &["--attempts", "1"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let dir = fx.campaign_dir();
    // The idle chunk, then one turn each: three chunks, one attempt, one ledger row.
    let names: Vec<String> = fx.prompts().into_iter().map(|(n, _)| n).collect();
    assert_eq!(names, ["prompt-1-0", "prompt-1-1", "prompt-1-2"]);
    assert_eq!(rows(&dir).len(), 1);

    let after_idle = fx.prompt("prompt-1-1");
    assert!(after_idle.contains("No turn has been played in the live game yet"), "{after_idle}");
    assert!(after_idle.contains("not a fresh start"), "{after_idle}");
    assert!(after_idle.contains("not starting a new one"), "{after_idle}");
    assert!(!after_idle.contains("turn 0 of") && !after_idle.contains("so far)"), "{after_idle}");
    assert!(after_idle.contains("lab note from attempt 1"), "what was done is in it: {after_idle}");
    let next = fx.prompt("prompt-1-2");
    assert!(next.contains("The live game is at turn 1 of 2"), "{next}");
}

/// A chunk that changed nothing at all ends a campaign attempt; a chunk that only edited does not
/// (the test above). Without it an attempt with a stuck session would run chunk after chunk
/// until the dollar cap (#569).
#[test]
fn a_chunk_that_changes_nothing_ends_the_attempt() {
    let mut fx = Fixture::new("stall");
    fx.extra_env.push(("STUB_STALL_AT".to_owned(), "1".to_owned()));
    let out = fx.campaign("100", &["--attempts", "1"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let names: Vec<String> = fx.prompts().into_iter().map(|(n, _)| n).collect();
    assert_eq!(names, ["prompt-1-0", "prompt-1-1"], "no third chunk");
    assert!(stderr(&out).contains("changed nothing"), "{}", stderr(&out));
    // The attempt is still evaluated and recorded.
    assert_eq!(rows(&fx.campaign_dir()).len(), 1);
}

/// #566: every row and the champion record the build their scores were made on.
#[test]
fn rows_and_champion_record_the_build() {
    let fx = Fixture::new("buildrec");
    let out = fx.campaign("100", &["--attempts", "1"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let dir = fx.campaign_dir();
    assert_eq!(rows(&dir)[0]["build"], "stub-1");
    assert_eq!(json(&dir.join("champion/meta.json"))["build"], "stub-1");
    // The score line `fn_agent score --append` left on the played log says who computed it.
    let run1 = fs::read_to_string(dir.join("attempt-1/run.jsonl")).unwrap();
    assert!(run1.lines().last().unwrap().contains("\"build\":\"stub-1\""), "{run1}");
    assert!(json(&dir.join("ledger.json")).get("reevals").is_none(), "nothing to re-evaluate");
}

/// A resume on another build evaluates the champion again before anything else, and the attempts
/// that follow are compared with that, not with the champion's old mean; both numbers are in the
/// ledger, the prompt and the printed table. A resume on the same build does none of this.
#[test]
fn a_resume_on_another_build_re_evaluates_the_champion_first() {
    let mut fx = Fixture::new("reeval");
    // Attempt 1: SCORE 300 on screen seeds 7 and 41, mean 324, gate mean 402.5, on build stub-1.
    let out = fx.campaign("300", &["--attempts", "1"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let dir = fx.campaign_dir();

    // The same build again: the champion's mean stands, so nothing is run for it.
    let out = fx.resume("300,0", &dir, &["--attempts", "1", "--model", "stub"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(!dir.join("reeval-1").exists());
    assert!(json(&dir.join("ledger.json")).get("reevals").is_none());
    assert!(!stderr(&out).contains("evaluating it again"), "{}", stderr(&out));

    // Another build, whose sim scores every clean run 100 lower. The champion is now worth 224 on
    // the screen and 302.5 on the gate. Attempt 3 earns 274 and 352.5 there: less than the
    // champion's old 324 and 402.5, more than its new 224 and 302.5, by 50 on every gate seed.
    fx.extra_env.push(("STUB_BUILD".to_owned(), "stub-2".to_owned()));
    fx.extra_env.push(("STUB_SHIFT".to_owned(), "-100".to_owned()));
    let out = fx.resume("300,0,350", &dir, &["--attempts", "1", "--model", "stub"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(
        stderr(&out).contains("screen mean 324 -> 224, gate mean 402 -> 302 on build stub-2")
            || stderr(&out)
                .contains("screen mean 324 -> 224, gate mean 403 -> 302 on build stub-2"),
        "{}",
        stderr(&out)
    );
    for seed in [7, 41, 101, 102, 103, 104] {
        assert!(dir.join(format!("reeval-1/seed-{seed}.jsonl")).is_file(), "reeval-1/seed-{seed}");
    }
    assert!(!dir.join("reeval-1/seed-9001.jsonl").exists(), "the owner-only tier is not re-run");
    let ledger = json(&dir.join("ledger.json"));
    let reevals = ledger["reevals"].as_array().unwrap();
    assert_eq!(reevals.len(), 1, "{ledger}");
    let re = &reevals[0];
    assert_eq!((re["attempt"].as_u64(), re["after_rows"].as_u64()), (Some(1), Some(2)));
    assert_eq!((re["was_build"].as_str(), re["build"].as_str()), (Some("stub-1"), Some("stub-2")));
    assert_eq!(
        (re["was_screen_mean"].as_f64(), re["screen_mean"].as_f64()),
        (Some(324.0), Some(224.0))
    );
    assert_eq!(
        (re["was_gate_mean"].as_f64(), re["gate_mean"].as_f64()),
        (Some(402.5), Some(302.5))
    );
    let rows = rows(&dir);
    assert_eq!(rows[2]["build"], "stub-2");
    assert_eq!(rows[2]["screen_mean"], 274.0);
    assert_eq!(rows[2]["gate_mean"], 352.5);
    assert_eq!(rows[2]["verdict"]["diff"], 50.0, "judged against the champion on this build");
    assert_eq!(
        rows[2]["kept"], true,
        "352.5 beats the re-evaluated 302.5, though not the old 402.5"
    );
    assert_eq!(rows[1]["kept"], false);
    let meta = json(&dir.join("champion/meta.json"));
    assert_eq!((meta["attempt"].as_u64(), meta["screen_mean"].as_f64()), (Some(3), Some(274.0)));
    assert_eq!(meta["build"], "stub-2");
    assert_eq!(json(&dir.join("champion/gate.json"))["build"], "stub-2");

    // The prompt and the owner's table both show the old and the new value; the prompt shows no
    // owner-only score.
    let p = fx.prompt("prompt-3-0");
    let line = "screen mean 324 -> 224";
    assert!(p.contains(line) && p.contains("stub-1") && p.contains("stub-2"), "{p}");
    assert!(!p.contains("9101") && !p.contains("9201"), "{p}");
    assert!(stdout(&out).contains(line), "{}", stdout(&out));

    // And once the champion is on this build, the next resume on it leaves things alone.
    let out = fx.resume("0,0,0,10", &dir, &["--attempts", "1", "--model", "stub"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(json(&dir.join("ledger.json"))["reevals"].as_array().unwrap().len(), 1);
    assert!(!dir.join("reeval-2").exists());
}

/// A campaign from before builds were recorded has none on its rows or its champion. A resume
/// takes that for another build and re-evaluates once, saying the old build wasn't recorded.
#[test]
fn a_campaign_with_no_recorded_build_is_re_evaluated_on_resume() {
    let fx = Fixture::new("reevalold");
    let out = fx.campaign("100", &["--attempts", "1"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let dir = fx.campaign_dir();
    let mut ledger = json(&dir.join("ledger.json"));
    ledger["rows"][0].as_object_mut().unwrap().remove("build");
    fs::write(dir.join("ledger.json"), serde_json::to_string_pretty(&ledger).unwrap()).unwrap();
    let mut meta = json(&dir.join("champion/meta.json"));
    meta.as_object_mut().unwrap().remove("build");
    fs::write(dir.join("champion/meta.json"), serde_json::to_string_pretty(&meta).unwrap())
        .unwrap();

    let out = fx.resume("100,50", &dir, &["--attempts", "1", "--model", "stub"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let ledger = json(&dir.join("ledger.json"));
    assert!(ledger["rows"][0].get("build").is_none(), "an old row stays as it was");
    let reevals = ledger["reevals"].as_array().unwrap();
    assert_eq!(reevals.len(), 1);
    assert!(reevals[0]["was_build"].is_null());
    assert_eq!(reevals[0]["build"], "stub-1");
    assert!(stdout(&out).contains("an unrecorded build"), "{}", stdout(&out));
    assert_eq!(json(&dir.join("champion/meta.json"))["build"], "stub-1");
}

/// An `fn_agent` that can't say what build it is can't have a champion compared fairly: the
/// resume stops before it plays anything, and says why.
#[test]
fn a_resume_stops_when_fn_agent_cannot_name_its_build() {
    let mut fx = Fixture::new("nobuild");
    let out = fx.campaign("100", &["--attempts", "1"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let dir = fx.campaign_dir();
    fx.extra_env.push(("STUB_NO_BUILD_ID".to_owned(), "1".to_owned()));
    let out = fx.resume("100,200", &dir, &["--attempts", "1", "--model", "stub"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("build-id"), "{}", stderr(&out));
    assert!(!dir.join("attempt-2").exists(), "nothing was played");
}

/// #568: a campaign calls the model, so `--track` and `--resume` both refuse to start without
/// `--max-usd`, before a folder is made or `claude` is run; a resumed campaign's old spend is no
/// excuse (the cap is of this invocation).
#[test]
fn a_campaign_needs_a_spend_cap_and_so_does_a_resume() {
    let fx = Fixture::new("nocap");
    let out = fx.agent_run("1", &["--track", "tiny", "--attempts", "1"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("needs a spend cap: add --max-usd N"), "{}", stderr(&out));
    assert!(!fx.runs_dir.exists(), "no campaign folder");

    let out = fx.campaign("100", &["--attempts", "1"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let dir = fx.campaign_dir();
    let out = fx.agent_run("1", &["--resume", dir.to_str().unwrap()]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("needs a spend cap: add --max-usd N"), "{}", stderr(&out));
    assert_eq!(rows(&dir).len(), 1, "nothing was played");
}

/// #568: an attempt that ends because `claude` crashed (no result, so no cost) is not charged as
/// $0 and not retried: the campaign stops with an error, the attempt's folder says why, and the
/// attempt gets no ledger row.
#[test]
fn a_crashed_chunk_stops_the_campaign_with_no_row_for_the_attempt() {
    let mut fx = Fixture::new("crash");
    fx.extra_env.push(("STUB_COUNT".to_owned(), fx.tmp.join("count").display().to_string()));
    // The first attempt is two chunks; the third call is attempt 2's first.
    fx.extra_env.push(("STUB_NOJSON_AT".to_owned(), "3".to_owned()));
    let out = fx.campaign("100,200,300", &["--attempts", "3"]);
    assert!(!out.status.success(), "{}", stderr(&out));
    let err = stderr(&out);
    assert!(err.contains("unknown, not $0"), "{err}");
    assert!(err.contains("exit status: 1"), "{err}");
    assert!(err.contains("attempt 2"), "{err}");
    let dir = fx.campaign_dir();
    assert_eq!(rows(&dir).len(), 1, "attempt 1 only");
    let stop = json(&dir.join("attempt-2/stop.json"));
    assert_eq!(stop["reason"], "unknown_cost");
    assert_eq!(json(&dir.join("attempt-2/cost.json"))["unknown_chunks"], serde_json::json!([1]));
    assert!(!dir.join("attempt-3").exists(), "no retry, no next attempt");
    assert_eq!(fs::read_to_string(fx.tmp.join("count")).unwrap().trim(), "3");
}

/// #570: an attempt that saved a better version and then a worse one hands on the better one,
/// chosen on the screen mean alone, and runs the gate and owner-only seeds for that one only.
#[test]
fn an_attempt_hands_on_its_best_version_not_its_last() {
    let mut fx = Fixture::new("bestver");
    // v1 scores 300 and is tested; v2, saved after it and not tested, scores 100. The owner-only
    // seed (9001) reverses the order (INVERT: 1000 - SCORE), so a selection that peeked at it
    // would pick v2.
    fx.extra_env.push(("STUB_VERSION_SCORES".to_owned(), "300,100".to_owned()));
    fx.extra_env.push(("STUB_TEST_CHUNKS".to_owned(), "0".to_owned()));
    fx.extra_env.push(("STUB_INVERT".to_owned(), "1".to_owned()));
    let out = fx.campaign("1", &["--attempts", "1"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let dir = fx.campaign_dir();

    let rows = rows(&dir);
    let r = &rows[0];
    assert_eq!(r["governor_version"], 1, "the better one, not the last saved: {r}");
    assert_eq!((r["saved"].as_u64(), r["tried"].as_u64()), (Some(2), Some(2)), "{r}");
    assert_eq!(scores(r, "screen"), [307.0, 341.0]);
    assert_eq!(r["screen_mean"], 324.0);
    // Owner-only: v1's own, 1000 - 300 + 9001.
    assert_eq!(scores(r, "owner"), [9701.0]);
    // v1's plan is the one it ended its turn with: the first chunk's, one edit.
    assert_eq!(r["plan_revision"], 1);

    let meta = json(&dir.join("champion/meta.json"));
    assert_eq!(meta["governor_version"], 1);
    assert_eq!(meta["screen_mean"], 324.0);
    let handoff = json(&dir.join("champion/handoff.json"));
    assert!(handoff["scripts"]["governor"]["code"].as_str().unwrap().contains("SCORE=300"));
    assert_eq!(handoff["scripts"]["governor"]["version"], 1);
    assert_eq!(handoff["plan"]["call"], 1, "v1's plan, not the last plan");
    // The attempt's own record is untouched: its final strategy is v2 with the last plan.
    let last = json(&dir.join("attempt-1/handoff.json"));
    assert!(last["scripts"]["governor"]["code"].as_str().unwrap().contains("SCORE=100"));
    assert_eq!(last["plan"]["call"], 2);

    // What was tried, with screen scores and the time each stage took, and no owner-only number.
    let tried = json(&dir.join("attempt-1/eval/versions.json"));
    assert_eq!((tried["saved"].as_u64(), tried["tried"].as_u64()), (Some(2), Some(2)));
    assert_eq!(tried["chosen"], 1);
    assert!(tried["seconds"].is_u64() && tried["build"] == "stub-1", "{tried}");
    assert!(tried["threads"].as_u64().unwrap() >= 1 && tried["promoted"] == true, "{tried}");
    for stage in ["screen_seconds", "gate_seconds", "owner_seconds"] {
        assert!(tried[stage].is_u64(), "{stage}: {tried}");
    }
    let pairs = tried["pairs"].as_array().unwrap();
    let means: Vec<f64> = pairs.iter().map(|p| p["screen_mean"].as_f64().unwrap()).collect();
    assert_eq!(means, [324.0, 124.0]);
    assert_eq!(pairs[0]["tested"], true);
    assert_eq!(pairs[1]["tested"], false);
    assert!(!tried.to_string().contains("9701") && !tried.to_string().contains("9101"));

    // Every pair ran on the screen seeds (four runs); the four gate seeds and the owner-only seed
    // ran once, for the chosen governor.
    let calls = fs::read_to_string(&fx.fn_agent_log).unwrap();
    let plays: Vec<&str> = calls.lines().filter(|l| l.starts_with("play")).collect();
    let owner: Vec<&&str> = plays.iter().filter(|l| l.contains("--seed 9001")).collect();
    let gate =
        plays.iter().filter(|l| (101..=104).any(|s| l.contains(&format!("--seed {s} ")))).count();
    assert_eq!((plays.len(), gate, owner.len()), (9, 4, 1), "{calls}");
    assert!(plays.last().unwrap().contains("--seed 9001"), "owner-only last: {calls}");

    // The ledger the owner sees names the version and how many were tried.
    let text = stdout(&out);
    assert!(text.contains("tried") && text.contains("2/2") && text.contains("v1"), "{text}");
}

/// A version that only ties the champion's mean does not replace it: the champion test still
/// wants a strict gain.
#[test]
fn a_version_that_ties_the_champion_does_not_replace_it() {
    let fx = Fixture::new("tie");
    let out = fx.campaign("100,100", &["--attempts", "2"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let rows = rows(&fx.campaign_dir());
    assert_eq!((rows[0]["kept"].as_bool(), rows[1]["kept"].as_bool()), (Some(true), Some(false)));
    let v = &rows[1]["verdict"];
    assert_eq!((v["diff"].as_f64(), v["ties"].as_u64()), (Some(0.0), Some(4)), "{v}");
}

/// #571: the gate decides, not the screen. A candidate far ahead on the screen seeds that falls
/// apart on the gate seeds is not promoted, and the owner-only tier is not run for it.
#[test]
fn the_gate_decides_not_the_screen() {
    let mut fx = Fixture::new("gatedecides");
    fx.extra_env.push(("STUB_GATE_BAD_ABOVE".to_owned(), "500".to_owned()));
    let out = fx.campaign("100,900", &["--attempts", "2"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let dir = fx.campaign_dir();
    let rows = rows(&dir);
    assert_eq!(rows[1]["screen_mean"], 924.0, "far ahead on the screen");
    assert_eq!(rows[1]["kept"], false);
    assert_eq!(rows[1]["verdict"]["promoted"], false);
    assert!(rows[1]["verdict"]["hi"].as_f64().unwrap() < 0.0, "{}", rows[1]["verdict"]);
    assert!(rows[1]["owner"].as_array().unwrap().is_empty());
    assert_eq!(json(&dir.join("champion/meta.json"))["attempt"], 1);
    let calls = fs::read_to_string(&fx.fn_agent_log).unwrap();
    assert_eq!(
        calls.matches("--seed 9001").count(),
        1,
        "the owner-only tier ran for attempt 1 only"
    );
    // The prompt of the next attempt would have shown the loss (the gate mean and the gain
    // against the champion), and the table printed to the owner has it.
    let text = stdout(&out);
    assert!(text.contains("-1200 [-1200, -1200]"), "{text}");
}

/// #571: a clear gain that comes with a collapse the champion does not have is not promoted.
#[test]
fn a_new_collapse_blocks_promotion() {
    let mut fx = Fixture::new("collapse");
    fx.extra_env.push(("STUB_COLLAPSE_ABOVE".to_owned(), "500".to_owned()));
    let out = fx.campaign("100,900", &["--attempts", "2"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let rows = rows(&fx.campaign_dir());
    let v = &rows[1]["verdict"];
    assert!(v["lo"].as_f64().unwrap() > 0.0, "the interval is wholly above zero: {v}");
    assert_eq!(
        (v["candidate_collapses"].as_u64(), v["champion_collapses"].as_u64()),
        (Some(1), Some(0))
    );
    assert_eq!(v["promoted"], false);
    assert!(v["reason"].as_str().unwrap().contains("collapses on 1"), "{v}");
    assert_eq!(rows[1]["kept"], false);
    // The failure count is on the table, for the model as well as the owner.
    assert!(fx.prompt("prompt-2-0").contains("0/4"), "{}", fx.prompt("prompt-2-0"));
    assert!(stdout(&out).contains("1/4"), "{}", stdout(&out));
}

/// #571: a campaign from before the seed tiers is refused on resume with a message that says why,
/// and nothing is played.
#[test]
fn resume_refuses_a_ledger_from_before_the_tiers() {
    let fx = Fixture::new("pretier");
    let dir = fx.tmp.join("old-campaign");
    fs::create_dir_all(dir.join("champion")).unwrap();
    let old = serde_json::json!({
        "track": {
            "id": "tiny", "name": "Tiny", "nation": "water", "difficulty": "normal",
            "map_size": "small", "site": "default", "prelude": null, "horizon_turns": 2,
            "play_seed": 7, "train_seeds": [7, 41], "holdout_seeds": [9001]
        },
        "rows": []
    });
    fs::write(dir.join("ledger.json"), old.to_string()).unwrap();
    let out = fx.resume("100", &dir, &["--attempts", "1"]);
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(err.contains("before the seed tiers") && err.contains("--track ID"), "{err}");
    assert!(!dir.join("attempt-1").exists(), "nothing was played");
}

// ----- par and the controls (#572) -----

/// Par and the do-nothing control are measured once the attempts are done and shown in the
/// owner's table and the ledger, in the owner's words, and nothing of them reaches a prompt or the
/// scorecard a prompt carries.
#[test]
fn par_and_the_controls_are_for_the_owners_table_and_the_ledger_not_the_prompt() {
    let mut fx = Fixture::new("references");
    fx.references = true;
    let out = fx.campaign("100,200", &["--attempts", "2"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let table = stdout(&out);
    // Par is 500 + the seed's last two digits (the stub; 9001 gives 501): screen 7 and 41, the four gate seeds,
    // and the owner-only seed 9001. Do-nothing scores the seed itself.
    for want in [
        "references (build stub-1)",
        "par         screen 507/541 \u{b7} gate 501/502/503/504 \u{b7} owner 501",
        "do-nothing  screen 7/41 \u{b7} gate 101/102/103/104 \u{b7} owner 9001",
        "par is what a stock scripted opening reaches on this start, reading the world directly",
        "not a target a fair player should hit, and not a ceiling",
        "by turn 2, par had not met on any of 7 seed(s): Ready",
        "not a proof",
    ] {
        assert!(table.contains(want), "missing {want:?} in\n{table}");
    }
    let dir = fx.campaign_dir();
    let ledger = json(&dir.join("ledger.json"));
    assert_eq!(ledger["references"]["par"].as_array().unwrap().len(), 7, "{ledger}");
    assert_eq!(ledger["references"]["controls"][0]["label"], "do-nothing");
    assert_eq!(ledger["references"]["controls"][0]["seeds"].as_array().unwrap().len(), 7);
    assert_eq!(ledger["references"]["build"], "stub-1");
    // The rows are as they were: a reader of the rows never meets the new key.
    assert!(rows(&dir)[0].get("references").is_none());

    // Not in any prompt, and the scorecards the prompts carry are asked for with no par.
    for (name, text) in fx.prompts() {
        for hidden in
            ["references (build", "do-nothing", "stock scripted", "reading the world", "507/541"]
        {
            assert!(!text.contains(hidden), "{hidden:?} leaked into {name}:\n{text}");
        }
    }
    let calls = fs::read_to_string(&fx.fn_agent_log).unwrap();
    let scores: Vec<&str> = calls.lines().filter(|l| l.starts_with("score ")).collect();
    assert!(!scores.is_empty());
    for l in &scores {
        assert!(l.contains("--no-par"), "a score call without --no-par: {l}");
    }
    let pars: Vec<&str> = calls.lines().filter(|l| l.starts_with("par ")).collect();
    assert_eq!(pars.len(), 7, "one par per seed of the three tiers: {pars:?}");
    assert!(pars.iter().all(|l| l.contains("--horizon 2") && l.contains("--json")), "{pars:?}");
    // ... and not during an attempt: the first par comes after the last clean evaluation.
    let first_par = calls.lines().position(|l| l.starts_with("par ")).unwrap();
    let all: Vec<&str> = calls.lines().collect();
    let last_play =
        all.iter().rposition(|l| l.starts_with("play ") && !l.contains("do-nothing")).unwrap();
    assert!(first_par > last_play, "par ran during an attempt:\n{calls}");
}

/// A control is played once per build and tier: a resume on the same build reads it back.
#[test]
fn the_controls_are_kept_per_build_and_a_new_build_plays_them_again() {
    let mut fx = Fixture::new("controls-cache");
    fx.references = true;
    let out = fx.campaign("100", &["--attempts", "1"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let count = |fx: &Fixture| {
        fs::read_to_string(&fx.fn_agent_log)
            .unwrap()
            .lines()
            .filter(|l| l.starts_with("play ") && l.contains("do-nothing"))
            .count()
    };
    assert_eq!(count(&fx), 7, "seven seeds, played once");
    let dir = fx.campaign_dir();
    let out = fx.resume("100", &dir, &["--attempts", "1"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(count(&fx), 7, "the same build: read back from the cache");
    assert!(stdout(&out).contains("do-nothing  screen 7/41"), "{}", stdout(&out));

    fx.extra_env.push(("STUB_BUILD".into(), "stub-2".into()));
    let out = fx.resume("100", &dir, &["--attempts", "1"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(count(&fx), 14, "another build plays them again");
    assert!(stdout(&out).contains("references (build stub-2)"), "{}", stdout(&out));
}

/// `--control GOV@PLAN` adds a fixed strategy to the table; the switches leave par and the
/// do-nothing control out; a par that can't be made is said so and stops nothing.
#[test]
fn a_fixed_control_the_switches_and_a_par_that_fails() {
    let mut fx = Fixture::new("controls-flags");
    fx.references = true;
    fs::write(fx.tmp.join("busy.rhai"), "// SCORE=300\nwait(3);").unwrap();
    fs::write(fx.tmp.join("busy.json"), r#"{"targets": {"wood": 10}}"#).unwrap();
    let spec =
        format!("{}@{}", fx.tmp.join("busy.rhai").display(), fx.tmp.join("busy.json").display());
    let out = fx.campaign("100", &["--attempts", "1", "--control", &spec, "--no-par"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let table = stdout(&out);
    assert!(table.contains("do-nothing  screen 7/41"), "{table}");
    assert!(table.contains("busy        screen 307/341"), "{table}");
    assert!(!table.contains("par  "), "--no-par: {table}");

    // A par that fails is noted in the table; the campaign and the controls still finish.
    fx.extra_env.push(("STUB_PAR_FAIL".into(), "1".into()));
    let dir = fx.campaign_dir();
    let out = fx.resume("100", &dir, &["--attempts", "1"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let table = stdout(&out);
    assert!(table.contains("no par: ") && table.contains("par failed"), "{table}");
    assert!(table.contains("do-nothing  screen 7/41"), "{table}");

    // The switches belong to a campaign.
    let out = fx.agent_run("1", &["--no-par", "--seed", "3", "--nation", "fire", "--max-usd", "1"]);
    assert!(
        !out.status.success() && stderr(&out).contains("--track or --resume"),
        "{}",
        stderr(&out)
    );
}
