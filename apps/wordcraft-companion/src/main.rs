//! `wordcraft-companion` (fork addition, see `docs/companion.md`): watches the WordCraft window you
//! write in and, when you pause, asks Claude (through your Claude Code login) for a few edits to the
//! paragraph you're on. They appear in the page in orange; click ✓ or ✗.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod claude;
mod prompt;
mod watch;

use std::collections::VecDeque;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use wordcraft_mcp::{Backend, Remote};

use crate::claude::Claude;
use crate::watch::{Context, Settings, Watch};

const USAGE: &str = "\
wordcraft-companion — Claude's inline suggestions in the WordCraft window you write in

Start WordCraft with a control port first: wordcraft --control 7981 (or set WORDCRAFT_CONTROL_PORT).

USAGE:
  wordcraft-companion [options]

OPTIONS:
  --connect HOST:PORT    the WordCraft window (default 127.0.0.1:7981)
  --brief FILE           what this piece is for: thesis, audience, tone (read again on every call)
  --style FILE           your style guide (read again on every call)
  --model NAME           Claude model for suggestions (default sonnet)
  --idle SECONDS         pause before asking (default 4)
  --max N                suggestions per paragraph at most (default 3)
  --reach N              look only this many paragraphs around the caret (default 2)
  --review-existing      also review text that was there when the companion started
  --max-per-hour N       Claude calls per hour at most (default 60)
  --claude PATH          the claude program (default: claude on the PATH)
  --claude-arg ARG       one more argument for claude -p (repeatable), e.g. --claude-arg=--effort=low
  --once                 review the caret's paragraph now, post, and exit
  --dry-run              print suggestions instead of posting them
  --verbose              print Claude's whole reply
";

struct Options {
    connect: String,
    brief: Option<String>,
    style: Option<String>,
    idle: Duration,
    max: usize,
    reach: Option<usize>,
    review_existing: bool,
    max_per_hour: usize,
    once: bool,
    dry_run: bool,
    verbose: bool,
    claude: Claude,
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut o = Options {
        connect: "127.0.0.1:7981".into(),
        brief: None,
        style: None,
        idle: Duration::from_secs(4),
        max: 3,
        reach: Some(2),
        review_existing: false,
        max_per_hour: 60,
        once: false,
        dry_run: false,
        verbose: false,
        claude: Claude { program: "claude".into(), model: "sonnet".into(), extra: Vec::new(), timeout: Duration::from_secs(180) },
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let (flag, inline) = match a.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f, Some(v.to_string())),
            _ => (a.as_str(), None),
        };
        let mut value = || inline.clone().or_else(|| it.next().cloned()).ok_or_else(|| format!("{flag} needs a value"));
        let number = |v: String| v.parse::<u64>().map_err(|_| format!("{flag}: not a number: {v}"));
        match flag {
            "--connect" => o.connect = value()?,
            "--brief" => o.brief = Some(value()?),
            "--style" => o.style = Some(value()?),
            "--model" => o.claude.model = value()?,
            "--idle" => o.idle = Duration::from_secs(number(value()?)?.max(1)),
            "--max" => o.max = usize::try_from(number(value()?)?.clamp(1, 10)).unwrap_or(3),
            "--reach" => o.reach = usize::try_from(number(value()?)?).ok(),
            "--review-existing" => {
                o.review_existing = true;
                o.reach = None;
            }
            "--max-per-hour" => o.max_per_hour = usize::try_from(number(value()?)?).unwrap_or(60),
            "--claude" => o.claude.program = value()?,
            "--claude-arg" => o.claude.extra.push(value()?),
            "--once" => o.once = true,
            "--dry-run" => o.dry_run = true,
            "--verbose" => o.verbose = true,
            "-h" | "--help" => return Err(String::new()),
            _ => return Err(format!("unknown option {a}")),
        }
    }
    Ok(o)
}

fn read_optional(path: &Option<String>) -> Option<String> {
    let p = path.as_ref()?;
    match std::fs::read_to_string(p) {
        Ok(t) => Some(t),
        Err(e) => {
            log(&format!("can't read {p}: {e}"));
            None
        }
    }
}

fn log(msg: &str) {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    println!("[{:02}:{:02}:{:02}] {msg}", (secs / 3600) % 24, (secs / 60) % 60, secs % 60);
}

fn context(app: &mut Remote) -> Result<Context, String> {
    app.call("engine.execute", json!({"command": "suggest.context", "params": {}})).map(|v| Context::from_json(&v))
}

/// Ask about paragraph `target` and post what comes back. Returns how many were posted.
fn review(app: &mut Remote, o: &Options, watch: &mut Watch, ctx: &Context, target: usize) -> Result<usize, String> {
    let (brief, style) = (read_optional(&o.brief), read_optional(&o.style));
    let material = prompt::Material { brief: brief.as_deref(), style: style.as_deref(), max: o.max };
    let Some(p) = prompt::build(ctx, target, &material) else { return Ok(0) };
    let preview: String = ctx.block(target).map(|b| b.text.chars().take(60).collect()).unwrap_or_default();
    log(&format!("asking {} about ¶{target}: “{preview}…”", o.claude.model));
    let started = Instant::now();
    let reply = o.claude.ask(&p)?;
    if o.verbose {
        log(&format!("reply: {}", reply.trim()));
    }
    let proposals = prompt::parse(&reply)?;
    log(&format!("{} suggestion(s) in {:.1} s", proposals.len(), started.elapsed().as_secs_f32()));
    let mut posted = 0;
    for pr in proposals.iter().take(o.max) {
        if o.dry_run {
            log(&format!("  {} — {}", pr.params, pr.why));
            continue;
        }
        match app.call("engine.execute", json!({"command": "suggest.add", "params": pr.params})) {
            Ok(_) => {
                posted += 1;
                log(&format!("  posted {} — {}", short(&pr.params), pr.why));
            }
            // The writer changed that text meanwhile, or the caret is on it: skip it.
            Err(e) => log(&format!("  skipped {}: {e}", short(&pr.params))),
        }
    }
    // The paragraph as it now reads (suggestions in) counts as looked at, so accepting them
    // doesn't trigger another round.
    if let Ok(now) = context(app)
        && let Some(b) = now.block(target)
    {
        watch.mark(&b.text);
    }
    Ok(posted)
}

fn short(v: &Value) -> String {
    let s = v.to_string();
    if s.chars().count() > 90 { format!("{}…", s.chars().take(90).collect::<String>()) } else { s }
}

fn run(o: Options) -> Result<(), String> {
    let mut app = Remote::connect(&o.connect).map_err(|e| format!("can't reach WordCraft at {}: {e} (start it with --control)", o.connect))?;
    if let Some(w) = app.key_warning() {
        return Err(w);
    }
    let first = context(&mut app)?;
    let mut watch = Watch::new(Settings { idle: o.idle, min_words: 8, reach: o.reach });
    if o.once {
        let target = first.caret.ok_or("the caret isn't in the body")?;
        review(&mut app, &o, &mut watch, &first, target)?;
        return Ok(());
    }
    if !o.review_existing {
        watch.mark_all(&first);
    }
    log(&format!("watching {} — write; suggestions appear after a {} s pause. Ctrl+C stops.", o.connect, o.idle.as_secs()));
    let mut calls: VecDeque<Instant> = VecDeque::new();
    let mut warned_budget = false;
    loop {
        std::thread::sleep(Duration::from_millis(800));
        let ctx = match context(&mut app) {
            Ok(c) => c,
            Err(e) => {
                log(&format!("WordCraft not answering ({e}); retrying"));
                std::thread::sleep(Duration::from_secs(3));
                continue;
            }
        };
        let now = Instant::now();
        while calls.front().is_some_and(|t| now.duration_since(*t) > Duration::from_secs(3600)) {
            calls.pop_front();
        }
        if calls.len() >= o.max_per_hour {
            if !warned_budget {
                log(&format!("reached {} calls this hour (--max-per-hour); pausing", o.max_per_hour));
                warned_budget = true;
            }
            continue;
        }
        warned_budget = false;
        let Some(target) = watch.observe(&ctx, now) else { continue };
        calls.push_back(now);
        if let Err(e) = review(&mut app, &o, &mut watch, &ctx, target) {
            log(&format!("no suggestions this time: {e}"));
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let o = match parse_args(&args) {
        Ok(o) => o,
        Err(e) => {
            if !e.is_empty() {
                eprintln!("{e}\n");
            }
            eprint!("{USAGE}");
            return if e.is_empty() { ExitCode::SUCCESS } else { ExitCode::from(2) };
        }
    };
    match run(o) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("wordcraft-companion: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn options() {
        let o = parse_args(&args(&["--model=opus", "--idle", "6", "--claude-arg=--effort=low", "--review-existing", "--brief", "b.md"])).unwrap();
        assert_eq!(o.claude.model, "opus");
        assert_eq!(o.idle, Duration::from_secs(6));
        assert_eq!(o.claude.extra, vec!["--effort=low"]);
        assert_eq!(o.reach, None);
        assert_eq!(o.brief.as_deref(), Some("b.md"));
        assert!(parse_args(&args(&["--idle", "x"])).is_err());
        assert!(parse_args(&args(&["--nope"])).is_err());
        assert!(parse_args(&args(&["--model"])).is_err());
    }
}
