//! End-to-end coverage of `cargo xtask agent-run --track ID --attempts N` and `--resume DIR`
//! (#530) against a stub `claude`, a stub `cargo` and a stub `fn_agent`: no model, no game.
//!
//! The stub `claude` plays an attempt by appending to the attempt's log what a session would
//! have: a `turn` and a `note` per call, a saved `governor` whose code carries the score the test
//! wants that attempt to earn (`STUB_SCORES`, one per attempt number), and a `plan` line. The
//! stub `fn_agent` answers `score` and `play` from that code: a clean run on seed S scores
//! `SCORE + S`, a played log `SCORE + 1`. So every number in a ledger is predictable, and the
//! held-out seed (9001) gives a value (`SCORE + 9001`) that must never show up in a prompt.
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
  turns=$(grep -c '"type":"turn"' "$log" 2>/dev/null)
  turns=${turns:-0}
  next=$((turns + 1))
  if [ "$next" = 1 ]; then
    # Later chunks rewrite mcp.json to `serve --resume`; keep the first chunk's.
    cp "$mcp_config" "$(dirname "$log")/first-mcp.json"
    # What `serve --carry-in` would have written before the first call.
    if grep -q -e '--carry-in' "$mcp_config"; then
      printf '{"type":"plan","plan":{"preloaded":true}}\n' >> "$log"
    fi
  fi
  printf '{"type":"turn","index":%d,"tick":%d}\n' "$next" "$((next * 1000))" >> "$log"
  printf '{"type":"note","text":"lab note from attempt %s, call %d"}\n' "$attempt" "$next" >> "$log"
  if [ -z "$STUB_NO_GOVERNOR" ]; then
    score=$(echo "$STUB_SCORES" | cut -d, -f"$attempt")
    printf '{"type":"script","name":"governor","version":%d,"code":"// SCORE=%s","turns":0,"wake_on":[]}\n' "$next" "$score" >> "$log"
    printf '{"type":"plan","plan":{"goal":"attempt %s","call":%d}}\n' "$attempt" "$next" >> "$log"
  fi
fi

if [ -n "$STUB_PROMPTS" ]; then
  mkdir -p "$STUB_PROMPTS"
  n=$(ls "$STUB_PROMPTS" | grep -c "^prompt-$attempt-")
  printf '%s\n' "$prompt" > "$STUB_PROMPTS/prompt-$attempt-$n"
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
case "$cmd" in
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
    printf '{"type":"score","score":%d.0,"progress":0.5,"care":0.9,"horizon":2,"attempt":null,"champion":null}\n' "$played" >> "$log"
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
  printf '{"scorecard":{"score":%d.0,"progress":0.5,"care":0.9},"stopped_by":null}\n' "$(( ${s:-0} + seed ))"
  ;;
*)
  echo "stub fn_agent: no $cmd" >&2
  exit 2
  ;;
esac
"#;

/// Train seeds 7 and 41 (a clean run scores `SCORE + 7` and `SCORE + 41`, mean `SCORE + 24`),
/// held out 9001.
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
train_seeds = [7, 41]
holdout_seeds = [9001]
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
            .env("FN_AGENT_BIN", self.bin_dir().join("fn_agent"))
            .env("STUB_SCORES", scores)
            .env("STUB_PROMPTS", &self.prompts)
            .env("STUB_FN_AGENT_LOG", &self.fn_agent_log)
            .env_remove("STUB_NO_GOVERNOR")
            .env_remove("OTEL_RESOURCE_ATTRIBUTES")
            .output()
            .unwrap()
    }

    fn campaign(&self, scores: &str, extra: &[&str]) -> Output {
        let mut args = vec!["--track", "tiny", "--model", "stub"];
        args.extend(extra);
        self.agent_run(scores, &args)
    }

    fn resume(&self, scores: &str, dir: &Path, extra: &[&str]) -> Output {
        let mut args = vec!["--resume", dir.to_str().unwrap()];
        args.extend(extra);
        self.agent_run(scores, &args)
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
        "attempt-1/eval/seed-7.jsonl",
        "attempt-1/eval/seed-41.jsonl",
        "attempt-1/eval/seed-9001.jsonl",
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
    assert_eq!(scores(r1, "clean"), [107.0, 141.0]);
    assert_eq!(r1["train_mean"], 124.0);
    assert_eq!(scores(r1, "held_out"), [9101.0]);
    assert_eq!(r1["kept"], true);
    assert!((r1["cost_usd"].as_f64().unwrap() - 0.02).abs() < 1e-9, "{r1}");
    let r2 = &rows[1];
    assert_eq!((r2["attempt"].as_u64(), r2["kept"].as_bool()), (Some(2), Some(true)));
    assert_eq!(r2["train_mean"], 224.0);
    // The preloaded plan isn't an edit: the champion's 2, plus this attempt's own 2.
    assert_eq!(r2["plan_revision"], 4);

    let meta = json(&dir.join("champion/meta.json"));
    assert_eq!((meta["attempt"].as_u64(), meta["train_mean"].as_f64()), (Some(2), Some(224.0)));
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

    // Every clean run carried the plan, on train seeds first and then the held-out seed.
    let calls = fs::read_to_string(&fx.fn_agent_log).unwrap();
    let plays: Vec<&str> = calls.lines().filter(|l| l.starts_with("play")).collect();
    assert_eq!(plays.len(), 6, "{calls}");
    assert!(plays.iter().all(|l| l.contains("--plan") && l.contains("--horizon 2")), "{calls}");
    let order: Vec<bool> = plays.iter().take(3).map(|l| l.contains("--seed 9001")).collect();
    assert_eq!(order, [false, false, true]);

    // The printed ledger is the owner's: held-out columns, the real numbers.
    let text = stdout(&out);
    assert!(
        text.contains("held-out 9001") && text.contains("9101") && text.contains("9201"),
        "{text}"
    );
    assert!(
        text.contains("clean 7/41") && text.contains("107 / 141") && text.contains("champ"),
        "{text}"
    );
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
    assert_eq!(rows(&dir)[1]["train_mean"], 124.0);

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

/// Held-out scores and seeds stay out of every prompt; the later attempts' first prompt carries
/// the train ledger and the champion's scorecard and asks for a better clean score.
#[test]
fn prompts_carry_the_train_ledger_and_never_the_held_out_scores() {
    let fx = Fixture::new("prompts");
    let out = fx.campaign("200,100", &["--attempts", "2"]);
    assert!(out.status.success(), "{}", stderr(&out));

    let first = fx.prompt("prompt-1-0");
    assert!(first.contains("You're playing The Four Nations"), "{first}");
    assert!(!first.contains("hill-climbing") && !first.contains("clean score"), "{first}");

    let second = fx.prompt("prompt-2-0");
    assert!(second.contains("attempt 2 of a hill-climbing campaign"), "{second}");
    assert!(
        second.contains("raise the clean score by improving the governor and the plan"),
        "{second}"
    );
    assert!(second.contains("207 / 241"), "the champion's train row is in it: {second}");
    assert!(second.contains("stub scorecard for"), "the champion's scorecard is in it: {second}");
    assert!(second.contains("You're playing The Four Nations"), "built on the script prompt");

    let all = fx.prompts();
    assert!(all.len() >= 4, "{all:?}");
    for (name, text) in &all {
        for hidden in ["9001", "9201", "9101", "held-out", "held out"] {
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
        .args(["agent-run", "--track", "tiny", "--attempts", "2", "--tracks"])
        .arg(fx.tracks())
        .env("PATH", format!("{}:{real_path}", fx.bin_dir().display()))
        .env("FN_AGENT_RUNS", &fx.runs_dir)
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
        assert!(r["governor_version"].is_null() && r["train_mean"].is_null(), "{r}");
        assert_eq!((r["clean"].as_array().unwrap().len(), r["kept"].as_bool()), (0, Some(false)));
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
        ("train_seeds = [7, 41]", "train_seeds = [7, 40]", "train_seeds"),
        ("holdout_seeds = [9001]", "holdout_seeds = [9002]", "holdout_seeds"),
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
        let out = fx.agent_run("1", args);
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
