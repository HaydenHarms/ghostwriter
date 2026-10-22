//! The Thesaurus pane (fork addition, see `docs/companion.md`): Review › Thesaurus or Shift+F7
//! looks up the selection or the word at the caret (`review.thesaurus`, which
//! `crates/engine/src/cmd/thesaurus.rs` answers from Open English WordNet) and shows its meanings
//! on the right. Clicking a synonym puts it in place of the word, in the word's form and case;
//! right-clicking one looks it up instead. Opened from [`after_run`], drawn by [`show`].

use std::cell::RefCell;

use egui::{RichText, Sense, Stroke, Ui};
use serde_json::{Value, json};

use crate::WordApp;
use crate::theme::{Tokens, regular, semibold};

/// The most words kept in the Back history.
const MAX_HISTORY: usize = 32;

#[derive(Default)]
struct State {
    open: bool,
    /// What the search box holds.
    query: String,
    /// The last lookup's result (`review.thesaurus`).
    result: Value,
    /// The word in the text a synonym replaces, and where it is (`{"anchor", "focus"}`).
    target: Option<(String, Value)>,
    /// Words looked up before this one, for Back.
    history: Vec<String>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// Whether the pane is open.
pub fn is_open() -> bool {
    STATE.with(|s| s.borrow().open)
}

/// After a command the user ran: `review.thesaurus` (the ribbon, Shift+F7, command search) opens
/// the pane on what it looked up. Scripts and agents (the control channel) don't come through here.
pub fn after_run(id: &str, r: &Result<Value, String>) {
    let (true, Ok(v)) = (id == "review.thesaurus", r) else { return };
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.open = true;
        s.query = v.get("word").and_then(Value::as_str).unwrap_or("").to_string();
        s.target = v.get("range").filter(|_| !s.query.is_empty()).map(|r| (s.query.clone(), r.clone()));
        s.result = v.clone();
        s.history.clear();
    });
}

/// Look `word` up in the pane, remembering the current word for Back.
fn look_up(app: &mut WordApp, word: &str, remember: bool) {
    let word = word.trim().to_string();
    let r = app.session.run("review.thesaurus", &json!({"word": word})).unwrap_or(Value::Null);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let current = s.result.get("word").and_then(Value::as_str).unwrap_or("").to_string();
        if remember && !current.is_empty() && current != word {
            s.history.push(current);
            if s.history.len() > MAX_HISTORY {
                s.history.remove(0);
            }
        }
        s.query = word;
        s.result = r;
    });
}

/// Put `syn` in place of the word the pane was opened on (or at the caret, without one).
fn replace(app: &mut WordApp, syn: &str) {
    let target = STATE.with(|s| s.borrow().target.clone());
    if let Some((word, range)) = target {
        let selected = app.session.run("select.range", &range).is_ok() && app.session.selected_text() == word;
        if !selected {
            STATE.with(|s| s.borrow_mut().target = None);
            app.status(tl!("The word has changed in the text. Look it up again."));
            return;
        }
    }
    let at = app.session.sel.ordered().0;
    if app.run("text.insert", json!({"text": syn, "raw": true})).is_err() {
        return;
    }
    // The new word is the one a second click replaces.
    let mut end = at.clone();
    end.off = at.off + syn.len();
    let range = json!({"anchor": wordcraft_engine::cmd::pos_json(&at), "focus": wordcraft_engine::cmd::pos_json(&end)});
    STATE.with(|s| s.borrow_mut().target = Some((syn.to_string(), range)));
    app.canvas.want_focus = true;
}

fn strings(v: &Value, key: &str) -> Vec<String> {
    v.get(key).and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

/// The pane, on the right, while open.
pub fn show(app: &mut WordApp, ui: &mut Ui) {
    if !is_open() {
        return;
    }
    let t = Tokens::get(ui.ctx());
    egui::Panel::right("thesaurus_pane")
        .default_size(260.0)
        .resizable(true)
        .frame(egui::Frame::NONE.fill(t.panel).inner_margin(10).stroke(Stroke::new(1.0, t.border)))
        .show(ui, |ui| {
            if crate::panes::header(ui, "Thesaurus") {
                STATE.with(|s| s.borrow_mut().open = false);
                return;
            }
            pane(app, ui, &t);
        });
}

fn pane(app: &mut WordApp, ui: &mut Ui, t: &Tokens) {
    let (mut query, result, back) = STATE.with(|s| {
        let s = s.borrow();
        (s.query.clone(), s.result.clone(), s.history.last().cloned())
    });
    let mut look: Option<(String, bool)> = None;
    ui.horizontal(|ui| {
        if let Some(prev) = &back
            && crate::widgets::icon_button(ui, "previous", tl!("Back")).clicked()
        {
            STATE.with(|s| {
                s.borrow_mut().history.pop();
            });
            look = Some((prev.clone(), false));
        }
        let r = egui::TextEdit::singleline(&mut query).hint_text(tl!("Search the thesaurus")).desired_width(f32::INFINITY).show(ui).response;
        if r.changed() {
            STATE.with(|s| s.borrow_mut().query = query.clone());
        }
        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !query.trim().is_empty() {
            look = Some((query.clone(), true));
        }
    });
    ui.add_space(6.0);
    let word = result.get("word").and_then(Value::as_str).unwrap_or("").to_string();
    let senses = result.get("senses").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut put: Option<String> = None;
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        if word.is_empty() {
            ui.label(RichText::new(tl!("Select a word, or type one above.")).color(t.text_dim));
        } else if senses.is_empty() {
            ui.label(RichText::new(crate::i18n::fmt(tl!("No synonyms for “{word}”."), &[("word", &word)])).color(t.text_dim));
        }
        let tip = tl!("Click to use this word; right-click to look it up.");
        let mut chips = |ui: &mut Ui, words: &[String]| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                for w in words {
                    let r =
                        ui.add(egui::Button::new(RichText::new(w).font(regular(12.5)).color(t.accent_text)).sense(Sense::click())).on_hover_text(tip);
                    if r.clicked() {
                        put = Some(w.clone());
                    } else if r.secondary_clicked() {
                        look = Some((w.clone(), true));
                    }
                }
            });
        };
        for sense in &senses {
            let lemma = sense.get("lemma").and_then(Value::as_str).unwrap_or("");
            let pos = sense.get("pos").and_then(Value::as_str).unwrap_or("");
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(lemma).font(semibold(13.0)).color(t.text));
                ui.label(RichText::new(tl!(pos)).font(regular(11.5)).italics().color(t.text_dim));
            });
            if let Some(g) = sense.get("gloss").and_then(Value::as_str).filter(|g| !g.is_empty()) {
                ui.label(RichText::new(g).font(regular(11.5)).color(t.text_dim));
            }
            chips(ui, &strings(sense, "synonyms"));
            let ants = strings(sense, "antonyms");
            if !ants.is_empty() {
                ui.label(RichText::new(tl!("Antonyms")).font(regular(11.5)).color(t.text_dim));
                chips(ui, &ants);
            }
            ui.add_space(2.0);
            ui.separator();
        }
        ui.add_space(4.0);
        ui.label(RichText::new(tl!("Synonyms from Open English WordNet 2024 (CC BY 4.0).")).font(regular(10.5)).color(t.text_dim));
    });
    if let Some(w) = put {
        replace(app, &w);
    }
    if let Some((w, remember)) = look {
        look_up(app, &w, remember);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> WordApp {
        WordApp::new(wordcraft_engine::Session::new(wordcraft_doc::Document::new()), crate::Services::default())
    }

    fn text(a: &WordApp) -> String {
        a.session.doc.plain_text(wordcraft_doc::StoryRef::Body).trim_end().to_string()
    }

    #[test]
    fn the_ribbon_command_opens_the_pane_and_a_click_replaces_the_word() {
        let mut a = app();
        a.run("text.insert", json!({"text": "Prices are improving fast"})).unwrap();
        a.run("select.text", json!({"text": "improving"})).unwrap();
        STATE.with(|s| *s.borrow_mut() = State::default());
        a.run("review.thesaurus", json!({})).unwrap();
        assert!(is_open());
        let syns = STATE.with(|s| strings(&s.borrow().result, "synonyms"));
        assert!(syns.iter().any(|w| w == "bettering"), "{syns:?}");
        replace(&mut a, "bettering");
        assert_eq!(text(&a), "Prices are bettering fast");
        // A second pick replaces the word just put in.
        replace(&mut a, "enhancing");
        assert_eq!(text(&a), "Prices are enhancing fast");
        // One undo step each.
        a.run("edit.undo", json!({})).unwrap();
        assert_eq!(text(&a), "Prices are bettering fast");
    }

    #[test]
    fn a_changed_word_is_not_replaced() {
        let mut a = app();
        a.run("text.insert", json!({"text": "a happy day"})).unwrap();
        a.run("select.text", json!({"text": "happy"})).unwrap();
        a.run("review.thesaurus", json!({})).unwrap();
        a.run("select.text", json!({"text": "happy"})).unwrap();
        a.run("text.insert", json!({"text": "sunny"})).unwrap();
        replace(&mut a, "glad");
        assert_eq!(text(&a), "a sunny day");
    }

    #[test]
    fn looking_up_keeps_a_history_for_back() {
        let mut a = app();
        STATE.with(|s| *s.borrow_mut() = State::default());
        look_up(&mut a, "happy", true);
        look_up(&mut a, "glad", true);
        assert_eq!(STATE.with(|s| s.borrow().history.clone()), vec!["happy".to_string()]);
    }
}
