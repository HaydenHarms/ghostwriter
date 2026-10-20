//! The Claude panel (fork addition, see `docs/companion.md`): Review › Claude, or "Claude" in the
//! command search (`suggest.panel`, which keeps whether it's open). It shows whether the companion is running (and starts it),
//! and sets what it does: suggestions on or off, how many and how soon, the model, learning the
//! writer's style (also per document), and the style profile itself.
//!
//! The panel and the companion share two files in the profile folder
//! (`wordcraft_mcp::suggest::settings`): the panel writes `settings.json`, which the
//! companion reads about every second, and shows the companion's `status.json`.

use egui::{Stroke, Ui};
use wordcraft_engine::cmd::suggest::{panel_open, set_panel};

use crate::WordApp;
use crate::theme::Tokens;
use crate::widgets::{big_toggle, group};

/// The Review tab's Claude group.
pub fn ribbon_group(app: &mut WordApp, ui: &mut Ui) {
    group(ui, "Claude", None, app, |ui, app| {
        big_toggle(ui, app, "ghostwriter", "Claude", "suggest.panel", panel_open());
    });
}

/// Ghostwriter's mascot, a little hooded reaper ghost with a scythe, as 16 × 16 pixel art (an
/// original drawing). `O` outline, `B` body, `L` highlight, `D` the shadow under the hood, `E` eyes,
/// `S` the scythe's handle, `W` its blade, `H` the hand.
const MASCOT: [&str; 16] = [
    "........WWWWWWS.",
    "...OOOOW...WWWS.",
    "..OBBBBBO.....S.",
    ".OBBLLBBBO....S.",
    ".OBLDDDDBO....S.",
    "OBBDDDDDDBO...S.",
    "OBDDEDDEDDBO..S.",
    "OBDDEDDEDDBO..S.",
    "OBBDDDDDDBBO.HS.",
    "OBBBDDDDBBBOHHS.",
    "OBBBBBBBBBBO..S.",
    "OBBBBBBBBBBO..S.",
    "OBBBBBBBBBBO..S.",
    "OBBOBBBOBBBO..S.",
    "OBO.OBO.OBO...S.",
    ".O...O...O......",
];

/// Paint the `ghostwriter` icon ([`MASCOT`]) into `r`; false for any other icon.
pub fn icon(p: &egui::Painter, r: egui::Rect, name: &str) -> bool {
    if name != "ghostwriter" {
        return false;
    }
    let k = (r.width().min(r.height()) / 16.0).floor().max(1.0);
    let origin = r.center() - egui::vec2(8.0 * k, 8.0 * k);
    for (y, row) in MASCOT.iter().enumerate() {
        for (x, ch) in row.chars().enumerate() {
            let rgb = match ch {
                'O' => (0x6B, 0x2E, 0x0A),
                'B' | 'H' => (0xE0, 0x7B, 0x1F),
                'L' => (0xF8, 0xB0, 0x5C),
                'D' => (0x2A, 0x12, 0x06),
                'E' => (0xFF, 0xE0, 0x6A),
                'S' => (0x8B, 0x5A, 0x2B),
                'W' => (0xA8, 0xB0, 0xBC),
                _ => continue,
            };
            let at = origin + egui::vec2(x as f32 * k, y as f32 * k);
            p.rect_filled(egui::Rect::from_min_size(at, egui::vec2(k, k)), 0.0, egui::Color32::from_rgb(rgb.0, rgb.1, rgb.2));
        }
    }
    true
}

/// The panel, on the right, while open.
pub fn show(app: &mut WordApp, ui: &mut Ui) {
    if !panel_open() {
        return;
    }
    let t = Tokens::get(ui.ctx());
    egui::Panel::right("claude_pane")
        .default_size(290.0)
        .resizable(true)
        .frame(egui::Frame::NONE.fill(t.panel).inner_margin(10).stroke(Stroke::new(1.0, t.border)))
        .show(ui, |ui| {
            if crate::panes::header(ui, "Claude") {
                set_panel(false);
                return;
            }
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| pane(app, ui));
        });
}

#[cfg(target_arch = "wasm32")]
fn pane(_: &mut WordApp, ui: &mut Ui) {
    ui.label(tl!("Claude works in the desktop app."));
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::cell::{Cell, RefCell};
    use std::path::{Path, PathBuf};

    use egui::{RichText, Ui};
    use serde_json::{Map, Value, json};
    use wordcraft_mcp::suggest::profile::Profile;
    use wordcraft_mcp::suggest::settings;

    use crate::WordApp;
    use crate::theme::{Tokens, regular, semibold};

    /// Defaults the companion uses when nothing is set (keep in step with its options).
    const DEFAULT_MAX: u64 = 3;
    const DEFAULT_IDLE: u64 = 4;
    const DEFAULT_MODEL: &str = "sonnet";
    /// How long "Click again to forget" waits for the second click.
    const ARM_FOR: std::time::Duration = std::time::Duration::from_secs(4);

    /// What the panel shows, from the shared files.
    struct Cache {
        at: std::time::Instant,
        settings: Map<String, Value>,
        /// The companion's report and its age in seconds.
        status: Option<(Map<String, Value>, u64)>,
        learned: String,
    }

    thread_local! {
        /// "Forget my style" clicked once: a second click within a few seconds forgets.
        static ARMED: Cell<Option<std::time::Instant>> = const { Cell::new(None) };
        /// The shared files, read at most about once a second.
        static CACHE: RefCell<Option<Cache>> = const { RefCell::new(None) };
    }

    fn dir() -> Option<PathBuf> {
        Profile::default_dir()
    }

    /// The shared files, re-read when older than a second (or `fresh`).
    fn cached<R>(fresh: bool, f: impl FnOnce(&Cache) -> R) -> Option<R> {
        let dir = dir()?;
        CACHE.with(|c| {
            let mut c = c.borrow_mut();
            if fresh || c.as_ref().is_none_or(|c| c.at.elapsed().as_millis() >= 1000) {
                *c = Some(Cache {
                    at: std::time::Instant::now(),
                    settings: settings::read(&dir),
                    status: settings::status(&dir),
                    learned: Profile::new(dir.clone()).style().1,
                });
            }
            c.as_ref().map(f)
        })
    }

    fn set(key: &str, value: Value) {
        if let Some(d) = dir() {
            if let Err(e) = settings::set(&d, key, value) {
                log::warn!("Claude panel: {e}");
            }
            cached(true, |_| ());
        }
    }

    /// The companion program next to this one, if it's there.
    fn companion() -> Option<PathBuf> {
        let here = std::env::current_exe().ok()?;
        let dir = here.parent()?;
        let ext = std::env::consts::EXE_SUFFIX;
        ["ghostwriter-companion", "wordcraft-companion"].iter().map(|n| dir.join(format!("{n}{ext}"))).find(|p| p.is_file())
    }

    /// Start the companion for this window, stopping when the window closes. Its output goes to
    /// `companion.log` in the profile folder.
    fn start(exe: &Path) -> Result<(), String> {
        let d = dir().ok_or("no settings folder")?;
        std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
        let out = std::fs::File::create(d.join("companion.log")).map_err(|e| e.to_string())?;
        let err = out.try_clone().map_err(|e| e.to_string())?;
        let port = std::env::var("WORDCRAFT_CONTROL_PORT").ok().filter(|p| p.parse::<u16>().is_ok()).unwrap_or_else(|| "7981".into());
        let mut c = std::process::Command::new(exe);
        c.args(["--exit-with-app", "--connect", &format!("127.0.0.1:{port}")]).stdin(std::process::Stdio::null()).stdout(out).stderr(err);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // CREATE_NO_WINDOW: no console window beside the app.
            c.creation_flags(0x0800_0000);
        }
        c.spawn().map(drop).map_err(|e| e.to_string())
    }

    /// Open the style profile in a text editor.
    fn open_profile() {
        let Some(d) = dir() else { return };
        let path = Profile::new(d).style_path();
        if !path.exists() {
            return;
        }
        let r = if cfg!(windows) {
            std::process::Command::new("notepad.exe").arg(&path).spawn()
        } else if cfg!(target_os = "macos") {
            std::process::Command::new("open").arg("-t").arg(&path).spawn()
        } else {
            std::process::Command::new("xdg-open").arg(&path).spawn()
        };
        if let Err(e) = r {
            log::warn!("Claude panel: can't open {}: {e}", path.display());
        }
    }

    fn forget() {
        let Some(d) = dir() else { return };
        if let Err(e) = Profile::new(d).forget() {
            log::warn!("Claude panel: {e}");
        }
        let n = cached(false, |c| c.settings.get("forget").and_then(Value::as_u64).unwrap_or(0)).unwrap_or(0);
        set("forget", json!(n.saturating_add(1)));
    }

    fn section(ui: &mut Ui, title: &str) {
        ui.add_space(10.0);
        ui.label(RichText::new(tl!(title)).font(semibold(13.0)));
        ui.add_space(2.0);
    }

    pub(super) fn pane(app: &mut WordApp, ui: &mut Ui) {
        let t = Tokens::get(ui.ctx());
        ui.ctx().request_repaint_after(std::time::Duration::from_secs(1));
        let Some((s, status, learned)) = cached(false, |c| (c.settings.clone(), c.status.clone(), c.learned.clone())) else {
            ui.label(tl!("Claude isn't running"));
            return;
        };
        let flag = |k: &str| s.get(k).and_then(Value::as_bool).unwrap_or(true);
        let num = |k: &str, d: u64| s.get(k).and_then(Value::as_u64).unwrap_or(d);

        // Status.
        let running = status.as_ref().filter(|(st, age)| *age < settings::STALE && st.get("stopped").is_none());
        let (dot, line) = match running {
            Some((st, _)) => match st.get("busy").and_then(Value::as_str) {
                Some("suggesting") => (t.orange, tl!("Claude is suggesting…")),
                Some("learning") => (t.orange, tl!("Claude is learning your style…")),
                _ if !flag("suggest") && !flag("learn") => (t.text_disabled, tl!("Claude is paused")),
                _ => (t.green, tl!("Claude is reading along")),
            },
            None if status.as_ref().is_some_and(|(st, _)| st.get("stopped").is_some()) => (t.red, tl!("Claude stopped")),
            None => (t.text_disabled, tl!("Claude isn't running")),
        };
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
            ui.painter().circle_filled(r.center(), 4.5, dot);
            ui.label(RichText::new(line).font(semibold(13.0)));
        });
        match running {
            Some((st, _)) => {
                let calls = st.get("callsThisHour").and_then(Value::as_u64).unwrap_or(0);
                let max = st.get("maxPerHour").and_then(Value::as_u64).unwrap_or(0);
                ui.label(RichText::new(format!("{calls} / {max} · {}", tl!("calls to Claude this hour"))).font(regular(12.0)).color(t.text_dim));
            }
            None => {
                if let Some(msg) = status.as_ref().and_then(|(st, _)| st.get("message")).and_then(Value::as_str).filter(|m| !m.is_empty()) {
                    ui.label(RichText::new(msg).font(regular(12.0)).color(t.text_dim));
                }
                match companion() {
                    Some(exe) => {
                        if ui.add(crate::widgets::primary_button(ui, tl!("Start Claude"))).clicked() {
                            if let Err(e) = start(&exe) {
                                log::warn!("Claude panel: can't start {}: {e}", exe.display());
                            }
                            cached(true, |_| ());
                        }
                    }
                    None => {
                        ui.label(
                            RichText::new(tl!("Run ghostwriter-companion in a terminal to start Claude.")).font(regular(12.0)).color(t.text_dim),
                        );
                    }
                }
            }
        }

        // Suggestions.
        section(ui, "Suggestions");
        let mut on = flag("suggest");
        if ui.checkbox(&mut on, tl!("Suggest edits as I write")).changed() {
            set("suggest", json!(on));
        }
        ui.add_enabled_ui(on, |ui| {
            ui.label(tl!("Suggestions per paragraph"));
            let mut max = num("max", DEFAULT_MAX).clamp(1, 5);
            if ui.add(egui::Slider::new(&mut max, 1..=5)).changed() {
                set("max", json!(max));
            }
            ui.label(tl!("Pause before suggesting (seconds)"));
            let mut idle = num("idle", DEFAULT_IDLE).clamp(2, 20);
            if ui.add(egui::Slider::new(&mut idle, 2..=20)).changed() {
                set("idle", json!(idle));
            }
        });
        ui.label(tl!("Model"));
        let model = s.get("model").and_then(Value::as_str).unwrap_or(DEFAULT_MODEL).to_string();
        let name = |id: &str| settings::MODELS.iter().find(|(m, _)| *m == id).map_or_else(|| id.to_string(), |(_, n)| tl!(n).to_string());
        egui::ComboBox::from_id_salt("claude_model").selected_text(name(&model)).show_ui(ui, |ui| {
            for (id, label) in settings::MODELS {
                if ui.selectable_label(model == id, tl!(label)).clicked() {
                    set("model", json!(id));
                }
            }
        });

        // Your style.
        section(ui, "Your style");
        let mut learn = flag("learn");
        if ui.checkbox(&mut learn, tl!("Learn my style")).changed() {
            set("learn", json!(learn));
        }
        let path = app.session.path.as_ref().map(|p| p.display().to_string());
        ui.add_enabled_ui(learn && path.is_some(), |ui| {
            let skipped = path.as_ref().is_some_and(|p| settings::skipped(&s).contains(p));
            let mut this = !skipped;
            if ui.checkbox(&mut this, tl!("Learn from this document")).changed()
                && let (Some(p), Some(d)) = (&path, dir())
            {
                if let Err(e) = settings::skip_learning(&d, p, !this) {
                    log::warn!("Claude panel: {e}");
                }
                cached(true, |_| ());
            }
        });
        if path.is_none() {
            ui.label(RichText::new(tl!("Save the document to choose this.")).font(regular(12.0)).color(t.text_dim));
        }
        ui.add_space(6.0);
        ui.label(RichText::new(tl!("What Claude has learned")).font(semibold(12.0)));
        egui::Frame::NONE.fill(t.canvas).inner_margin(8).corner_radius(4).show(ui, |ui| {
            ui.set_width(ui.available_width());
            if learned.trim().is_empty() {
                ui.label(
                    RichText::new(tl!("Nothing yet: Claude builds your profile once you've written a page or so."))
                        .font(regular(12.0))
                        .color(t.text_dim),
                );
            } else {
                egui::ScrollArea::vertical().id_salt("claude_learned").max_height(220.0).show(ui, |ui| {
                    ui.label(RichText::new(learned.trim()).font(regular(12.0)));
                });
            }
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui.add_enabled(!learned.trim().is_empty(), egui::Button::new(tl!("Open profile"))).clicked() {
                open_profile();
            }
            // One button: the first click asks, a second within a few seconds forgets.
            let armed = ARMED.with(Cell::get).filter(|at| at.elapsed() < ARM_FOR);
            let label = if armed.is_some() { tl!("Click again to forget") } else { tl!("Forget my style") };
            let text = if armed.is_some() { RichText::new(label).color(t.red) } else { RichText::new(label) };
            if ui.button(text).on_hover_text(tl!("Forget everything Claude has learned about your writing")).clicked() {
                if armed.is_some() {
                    forget();
                    ARMED.with(|a| a.set(None));
                } else {
                    ARMED.with(|a| a.set(Some(std::time::Instant::now())));
                }
            }
        });
    }
}

#[cfg(not(target_arch = "wasm32"))]
use native::pane;

#[cfg(test)]
mod tests {
    use serde_json::json;
    use wordcraft_engine::Session;

    use wordcraft_engine::cmd::suggest::panel_open;

    use crate::{Services, WordApp};

    #[test]
    fn the_mascot_is_sixteen_square() {
        assert!(super::MASCOT.iter().all(|row| row.chars().count() == 16));
    }

    #[test]
    fn the_claude_command_opens_and_closes_the_panel() {
        let mut a = WordApp::new(Session::new(wordcraft_engine::sample::sample_document()), Services::default());
        assert!(!panel_open());
        assert_eq!(a.run("suggest.panel", json!({})).unwrap(), json!({"open": true}));
        assert!(panel_open());
        a.run("suggest.panel", json!({})).unwrap();
        assert!(!panel_open());
        a.run("suggest.panel", json!({"value": true})).unwrap();
        a.run("suggest.panel", json!({"value": true})).unwrap();
        assert!(panel_open(), "value sets rather than toggles");
        a.run("suggest.panel", json!({"value": false})).unwrap();
    }
}
