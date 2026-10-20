//! Asking Claude through Claude Code's print mode (`claude -p`): it runs on the writer's own
//! Claude Code login (subscription), no API key.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct Claude {
    /// The `claude` program.
    pub program: String,
    pub model: String,
    /// More arguments for every call.
    pub extra: Vec<String>,
    pub timeout: Duration,
}

impl Claude {
    fn args(&self) -> Vec<String> {
        let mut a: Vec<String> = ["-p", "--output-format", "json", "--tools", "", "--no-session-persistence", "--model"].map(String::from).to_vec();
        a.push(self.model.clone());
        a.extend(self.extra.iter().cloned());
        a
    }

    /// A folder of its own, so the call picks up no project's CLAUDE.md or settings.
    fn workdir() -> PathBuf {
        let d = std::env::temp_dir().join("wordcraft-companion");
        let _ = std::fs::create_dir_all(&d);
        d
    }

    fn command(&self, through_shell: bool) -> Command {
        let mut c = if through_shell {
            // Windows: an npm install of Claude Code is `claude.cmd`, which only the shell runs.
            let mut c = Command::new("cmd");
            c.arg("/C").arg(&self.program);
            c
        } else {
            Command::new(&self.program)
        };
        c.args(self.args()).current_dir(Self::workdir()).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // CREATE_NO_WINDOW: started from the Claude panel the companion has no console, and
            // Windows would open a console window for every call otherwise.
            c.creation_flags(0x0800_0000);
        }
        c
    }

    /// Send `prompt`; the reply is `claude`'s standard output.
    pub fn ask(&self, prompt: &str) -> Result<String, String> {
        let mut child = match self.command(false).spawn() {
            Ok(c) => c,
            Err(e) if cfg!(windows) && e.kind() == std::io::ErrorKind::NotFound => {
                self.command(true).spawn().map_err(|e| format!("can't start {}: {e}", self.program))?
            }
            Err(e) => return Err(format!("can't start {}: {e} (is Claude Code installed? --claude sets its path)", self.program)),
        };
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(prompt.as_bytes()).map_err(|e| format!("writing the prompt: {e}"))?;
        }
        let mut out = child.stdout.take().ok_or("no output")?;
        let mut err = child.stderr.take().ok_or("no error output")?;
        let reader = std::thread::spawn(move || {
            let mut s = String::new();
            let _ = out.read_to_string(&mut s);
            s
        });
        let err_reader = std::thread::spawn(move || {
            let mut s = String::new();
            let _ = err.read_to_string(&mut s);
            s
        });
        let start = Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(st)) => break st,
                Ok(None) if start.elapsed() > self.timeout => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("claude took longer than {} s", self.timeout.as_secs()));
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(100)),
                Err(e) => return Err(format!("waiting for claude: {e}")),
            }
        };
        let stdout = reader.join().unwrap_or_default();
        let stderr = err_reader.join().unwrap_or_default();
        if !status.success() && stdout.trim().is_empty() {
            return Err(format!("claude failed ({status}): {}", stderr.trim().chars().take(400).collect::<String>()));
        }
        Ok(stdout)
    }
}
