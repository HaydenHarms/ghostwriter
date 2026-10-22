//! Builds `assets/thesaurus/oewn-2024.deflate`, the thesaurus behind `review.thesaurus` (fork
//! addition, see `crates/engine/src/cmd/thesaurus.rs`), from Open English WordNet 2024 in its
//! WNDB form (`english-wordnet-2024.zip` from https://en-word.net, unzipped):
//!
//! ```sh
//! cargo run -p wordcraft-engine --example thesaurus_data -- path/to/oewn2024 assets/thesaurus/oewn-2024.deflate
//! ```
//!
//! The output is deflated UTF-8 text in three sections (the format `thesaurus.rs` reads):
//!
//! ```text
//! WCTHES1
//! ==synsets
//! <pos n|v|a|r>\t<short gloss>\t<word|word|…>\t<antonym|…>      one meaning per line, numbered from 0
//! ==index
//! <lemma>\t<pos><id>,<id> <pos><id>…                             ids in base 36, most frequent first
//! ==exc
//! <pos>\t<inflected form>\t<lemma> <lemma>…                     irregular forms (went → go)
//! ```
//!
//! A meaning's words are its synset's words, then for adjectives the words of its "see also" and
//! similar synsets, and
//! for a synset with a single word its first hypernym's words. Meanings with nothing to offer are
//! left out.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::path::Path;

/// The most words kept for one meaning.
const MAX_WORDS: usize = 24;
/// The most similar-adjective words added to one meaning.
const MAX_SIMILAR: usize = 12;
/// The longest gloss kept, in characters.
const MAX_GLOSS: usize = 100;

struct Synset {
    pos: char,
    words: Vec<String>,
    /// (pointer symbol, target pos, target offset, source word, target word); words 1-based, 0 = all.
    ptrs: Vec<(String, char, u64, usize, usize)>,
    gloss: String,
}

fn pos_char(ss_type: &str) -> char {
    match ss_type {
        "n" => 'n',
        "v" => 'v',
        "a" | "s" => 'a',
        _ => 'r',
    }
}

/// `heavy_metal` → `heavy metal`; an adjective's position marker (`(a)`, `(p)`, `(ip)`) dropped.
fn word_text(w: &str) -> String {
    let w = w.split('(').next().unwrap_or(w);
    w.replace('_', " ")
}

/// The definition before the examples, cut to [`MAX_GLOSS`] characters.
fn short_gloss(g: &str) -> String {
    let def = g.split("; \"").next().unwrap_or(g);
    let def = def.split('"').next().unwrap_or(def).trim().trim_end_matches(';').trim();
    if def.chars().count() <= MAX_GLOSS {
        return def.to_string();
    }
    let cut: String = def.chars().take(MAX_GLOSS - 1).collect();
    format!("{}…", cut.trim_end())
}

fn parse_data(path: &Path, out: &mut HashMap<(char, u64), Synset>) -> Result<(), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    for line in text.lines() {
        if line.starts_with("  ") || line.trim().is_empty() {
            continue;
        }
        let (fields, gloss) = line.split_once(" | ").unwrap_or((line, ""));
        let f: Vec<&str> = fields.split_whitespace().collect();
        let field = |i: usize| f.get(i).copied().ok_or_else(|| format!("{}: short line {line:?}", path.display()));
        let offset: u64 = field(0)?.parse().map_err(|_| format!("bad offset in {line:?}"))?;
        let pos = pos_char(field(2)?);
        let w_cnt = usize::from_str_radix(field(3)?, 16).map_err(|_| format!("bad w_cnt in {line:?}"))?;
        let mut i = 4;
        let mut words = Vec::new();
        for _ in 0..w_cnt {
            words.push(word_text(field(i)?));
            i += 2;
        }
        let p_cnt: usize = field(i)?.parse().map_err(|_| format!("bad p_cnt in {line:?}"))?;
        i += 1;
        let mut ptrs = Vec::new();
        for _ in 0..p_cnt {
            let sym = field(i)?.to_string();
            let target: u64 = field(i + 1)?.parse().map_err(|_| format!("bad pointer in {line:?}"))?;
            let tpos = pos_char(field(i + 2)?);
            let st = field(i + 3)?;
            let src = usize::from_str_radix(st.get(..2).unwrap_or("00"), 16).unwrap_or(0);
            let dst = usize::from_str_radix(st.get(2..).unwrap_or("00"), 16).unwrap_or(0);
            ptrs.push((sym, tpos, target, src, dst));
            i += 4;
        }
        out.insert((pos, offset), Synset { pos, words, ptrs, gloss: short_gloss(gloss) });
    }
    Ok(())
}

fn push_unique(list: &mut Vec<String>, w: &str) {
    if list.len() < MAX_WORDS && !list.iter().any(|x| x.eq_ignore_ascii_case(w)) {
        list.push(w.to_string());
    }
}

fn base36(mut n: usize) -> String {
    const D: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut s = Vec::new();
    loop {
        s.push(D.get(n % 36).copied().unwrap_or(b'0'));
        n /= 36;
        if n == 0 {
            break;
        }
    }
    s.reverse();
    String::from_utf8(s).unwrap_or_default()
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let (Some(dir), Some(out_path)) = (args.get(1), args.get(2)) else {
        return Err("usage: thesaurus_data <oewn2024 dir> <out.deflate>".into());
    };
    let dir = Path::new(dir);
    let files = [("noun", 'n'), ("verb", 'v'), ("adj", 'a'), ("adv", 'r')];
    let mut synsets = HashMap::new();
    for (f, _) in files {
        parse_data(&dir.join(format!("data.{f}")), &mut synsets)?;
    }

    // Each synset's thesaurus line, and its new id when it has something to offer.
    let mut keys: Vec<(char, u64)> = synsets.keys().copied().collect();
    keys.sort();
    let mut ids: HashMap<(char, u64), usize> = HashMap::new();
    let mut lines = Vec::new();
    for key in &keys {
        let Some(s) = synsets.get(key) else { continue };
        let mut words: Vec<String> = Vec::new();
        for w in &s.words {
            push_unique(&mut words, w);
        }
        if s.pos == 'a' {
            // "See also" (cheerful, glad for happy) before the similar adjectives (blissful, halcyon).
            for want in ["^", "&"] {
                for (sym, tpos, off, _, _) in &s.ptrs {
                    if sym == want
                        && let Some(t) = synsets.get(&(*tpos, *off))
                    {
                        for w in t.words.iter().take(MAX_SIMILAR) {
                            push_unique(&mut words, w);
                        }
                    }
                }
            }
        }
        if words.len() < 2
            && let Some((_, tpos, off, _, _)) = s.ptrs.iter().find(|p| p.0 == "@" || p.0 == "@i")
            && let Some(t) = synsets.get(&(*tpos, *off))
        {
            for w in &t.words {
                push_unique(&mut words, w);
            }
        }
        let mut ants: Vec<String> = Vec::new();
        for (sym, tpos, off, _, dst) in &s.ptrs {
            if sym == "!"
                && let Some(t) = synsets.get(&(*tpos, *off))
            {
                let picked = if *dst == 0 { t.words.first() } else { t.words.get(dst - 1) };
                if let Some(w) = picked
                    && !ants.contains(w)
                    && ants.len() < 6
                {
                    ants.push(w.clone());
                }
            }
        }
        if words.len() < 2 && ants.is_empty() {
            continue;
        }
        ids.insert(*key, lines.len());
        let gloss = s.gloss.replace(['\t', '\n'], " ");
        lines.push(format!("{}\t{}\t{}\t{}", s.pos, gloss, words.join("|"), ants.join("|")));
    }

    // The index: lemma → its meanings per part of speech, in WordNet's (frequency) order.
    let mut index: BTreeMap<String, Vec<(char, Vec<usize>)>> = BTreeMap::new();
    for (f, pos) in files {
        let path = dir.join(format!("index.{f}"));
        let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        for line in text.lines() {
            if line.starts_with("  ") || line.trim().is_empty() {
                continue;
            }
            let f: Vec<&str> = line.split_whitespace().collect();
            let (Some(lemma), Some(cnt)) = (f.first(), f.get(2).and_then(|c| c.parse::<usize>().ok())) else { continue };
            let found: Vec<usize> =
                f.iter().rev().take(cnt).rev().filter_map(|o| o.parse::<u64>().ok()).filter_map(|o| ids.get(&(pos, o)).copied()).collect();
            if !found.is_empty() {
                index.entry(lemma.replace('_', " ")).or_default().push((pos, found));
            }
        }
    }

    let mut text = String::from("WCTHES1\n==synsets\n");
    for l in &lines {
        text.push_str(l);
        text.push('\n');
    }
    text.push_str("==index\n");
    for (lemma, per_pos) in &index {
        let parts: Vec<String> =
            per_pos.iter().map(|(p, list)| format!("{p}{}", list.iter().map(|i| base36(*i)).collect::<Vec<_>>().join(","))).collect();
        let _ = writeln!(text, "{lemma}\t{}", parts.join(" "));
    }
    text.push_str("==exc\n");
    for (f, pos) in [("noun", 'n'), ("verb", 'v'), ("adj", 'a')] {
        let path = dir.join(format!("{f}.exc"));
        let exc = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        for line in exc.lines() {
            let mut it = line.split_whitespace();
            let Some(form) = it.next() else { continue };
            let lemmas: Vec<String> = it.map(|l| l.replace('_', " ")).collect();
            if !lemmas.is_empty() {
                let _ = writeln!(text, "{pos}\t{}\t{}", form.replace('_', " "), lemmas.join(" "));
            }
        }
    }
    let packed = miniz_oxide::deflate::compress_to_vec(text.as_bytes(), 10);
    std::fs::write(out_path, &packed).map_err(|e| format!("{out_path}: {e}"))?;
    println!("{} meanings, {} lemmas, {} bytes of text, {} deflated", lines.len(), index.len(), text.len(), packed.len());
    Ok(())
}
