//! The thesaurus (fork addition, see `docs/companion.md`): `review.thesaurus` looks a word up in
//! Open English WordNet 2024 (`assets/thesaurus/oewn-2024.deflate`, built by
//! `crates/engine/examples/thesaurus_data.rs`) instead of upstream's short built-in list.
//!
//! A word is found in any form (`improving` → *improve*, `mice` → *mouse*, `better` → *good*)
//! and its synonyms come back in the same form and capitalisation (`Improving` → `Bettering`), so
//! picking one can replace the word as it stands. Meanings are grouped by part of speech, most
//! frequent first, each with a short gloss and its antonyms.

use std::sync::OnceLock;

use serde_json::{Value, json};

use super::pos_json;
use crate::{CmdResult, CommandSpec, Session, p};

static DATA: &[u8] = include_bytes!("../../../../assets/thesaurus/oewn-2024.deflate");

/// The longest word or phrase looked up, in characters.
const MAX_WORD: usize = 64;
/// The most meanings returned for one lookup, and per lemma.
const MAX_SENSES: usize = 16;
const MAX_SENSES_PER_LEMMA: usize = 8;
/// How many synonyms the flat `synonyms` list (the context menu's) holds.
const MAX_FLAT: usize = 8;
/// The most inflated text accepted from the data (it is about 12 MB).
const MAX_TEXT: usize = 64 << 20;

/// Swap upstream's `review.thesaurus` for this one, in place (its id, label, ribbon place and
/// Shift+F7 stay as they are).
pub fn replace(specs: &mut [CommandSpec]) {
    if let Some(spec) = specs.iter_mut().find(|s| s.id == "review.thesaurus") {
        spec.run = run;
        spec.params = r#"{"word"?: string (default: the selection, or the word at the caret)} → {"word", "synonyms": [the first few], "senses": [{"lemma", "pos": "noun|verb|adjective|adverb", "gloss", "synonyms", "antonyms"}], "range"?: {"anchor", "focus"} (the word in the text, when not given)}"#;
    }
}

/// How the word looked up is inflected, so its synonyms can be put in the same form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Form {
    Base,
    /// A noun's plural.
    Plural,
    /// A verb's third person singular (`improves`).
    Third,
    /// A verb's past (`improved`).
    Past,
    /// A verb's present participle (`improving`).
    Ing,
    /// An adjective's comparative (`bigger`).
    Comparative,
    /// An adjective's superlative (`biggest`).
    Superlative,
}

struct Data {
    text: String,
    /// Each meaning's line in `text`.
    synsets: Vec<(usize, usize)>,
    /// Each index line in `text`, sorted by lemma.
    index: Vec<(usize, usize)>,
    /// Irregular forms: (pos, form, lemmas).
    exc: Vec<(char, String, Vec<String>)>,
}

fn data() -> Option<&'static Data> {
    static D: OnceLock<Option<Data>> = OnceLock::new();
    D.get_or_init(|| match parse(DATA) {
        Ok(d) => Some(d),
        Err(e) => {
            log::warn!("thesaurus: {e}");
            None
        }
    })
    .as_ref()
}

fn parse(bytes: &[u8]) -> Result<Data, String> {
    let raw = miniz_oxide::inflate::decompress_to_vec_with_limit(bytes, MAX_TEXT).map_err(|e| format!("inflate failed: {e:?}"))?;
    let text = String::from_utf8(raw).map_err(|_| "not UTF-8".to_string())?;
    let mut d = Data { synsets: Vec::new(), index: Vec::new(), exc: Vec::new(), text: String::new() };
    let mut section = "";
    let mut at = 0;
    let mut first = true;
    for line in text.split_inclusive('\n') {
        let start = at;
        at += line.len();
        let line = line.trim_end_matches(['\n', '\r']);
        if first {
            if line != "WCTHES1" {
                return Err("bad header".into());
            }
            first = false;
            continue;
        }
        if let Some(s) = line.strip_prefix("==") {
            section = match s {
                "synsets" => "synsets",
                "index" => "index",
                "exc" => "exc",
                _ => return Err(format!("unknown section {s:?}")),
            };
            continue;
        }
        let end = start + line.len();
        match section {
            "synsets" => d.synsets.push((start, end)),
            "index" => d.index.push((start, end)),
            "exc" => {
                let mut f = line.split('\t');
                if let (Some(pos), Some(form), Some(lemmas)) = (f.next().and_then(|p| p.chars().next()), f.next(), f.next()) {
                    d.exc.push((pos, form.to_string(), lemmas.split(' ').map(str::to_string).collect()));
                }
            }
            _ => return Err("data before the first section".into()),
        }
    }
    d.text = text;
    // The index must be sorted for the binary search; the generator writes it so.
    let key = |r: &(usize, usize)| d.text.get(r.0..r.1).and_then(|l| l.split('\t').next()).unwrap_or("");
    if !d.index.windows(2).all(|w| w.first().map(key) <= w.get(1).map(key)) {
        return Err("index not sorted".into());
    }
    Ok(d)
}

impl Data {
    fn line(&self, r: (usize, usize)) -> &str {
        self.text.get(r.0..r.1).unwrap_or("")
    }

    /// The meaning ids for `lemma` as part of speech `pos`, most frequent first.
    fn lookup(&self, lemma: &str, pos: char) -> Vec<usize> {
        let found = self.index.binary_search_by(|r| self.line(*r).split('\t').next().unwrap_or("").cmp(lemma));
        let Ok(i) = found else { return Vec::new() };
        let Some(line) = self.index.get(i).map(|r| self.line(*r)) else { return Vec::new() };
        let Some(lists) = line.split('\t').nth(1) else { return Vec::new() };
        lists
            .split(' ')
            .filter(|l| l.starts_with(pos))
            .flat_map(|l| l.get(pos.len_utf8()..).unwrap_or("").split(',').filter_map(|id| usize::from_str_radix(id, 36).ok()))
            .filter(|id| *id < self.synsets.len())
            .collect()
    }

    fn has(&self, lemma: &str, pos: char) -> bool {
        !self.lookup(lemma, pos).is_empty()
    }

    /// The lemmas `form` is an irregular form of, as part of speech `pos`.
    fn exc_lemmas(&self, form: &str, pos: char) -> Vec<&str> {
        self.exc.iter().filter(|(p, f, _)| *p == pos && f == form).flat_map(|(_, _, l)| l.iter().map(String::as_str)).collect()
    }

    /// The irregular forms of `lemma` as part of speech `pos`.
    fn exc_forms(&self, lemma: &str, pos: char) -> Vec<&str> {
        self.exc.iter().filter(|(p, _, l)| *p == pos && l.iter().any(|x| x == lemma)).map(|(_, f, _)| f.as_str()).collect()
    }
}

/// WordNet's detachment rules: (suffix, replacement, form).
const NOUN_RULES: &[(&str, &str, Form)] = &[
    ("ses", "s", Form::Plural),
    ("xes", "x", Form::Plural),
    ("zes", "z", Form::Plural),
    ("ches", "ch", Form::Plural),
    ("shes", "sh", Form::Plural),
    ("men", "man", Form::Plural),
    ("ies", "y", Form::Plural),
    ("s", "", Form::Plural),
];
const VERB_RULES: &[(&str, &str, Form)] = &[
    ("ies", "y", Form::Third),
    ("es", "e", Form::Third),
    ("es", "", Form::Third),
    ("s", "", Form::Third),
    ("ied", "y", Form::Past),
    ("ed", "e", Form::Past),
    ("ed", "", Form::Past),
    ("ing", "e", Form::Ing),
    ("ing", "", Form::Ing),
];
const ADJ_RULES: &[(&str, &str, Form)] = &[
    ("ier", "y", Form::Comparative),
    ("er", "e", Form::Comparative),
    ("er", "", Form::Comparative),
    ("iest", "y", Form::Superlative),
    ("est", "e", Form::Superlative),
    ("est", "", Form::Superlative),
];

const POS: [char; 4] = ['n', 'v', 'a', 'r'];

fn pos_name(pos: char) -> &'static str {
    match pos {
        'n' => "noun",
        'v' => "verb",
        'a' => "adjective",
        _ => "adverb",
    }
}

/// What `word` (lower case) can be: (lemma, part of speech, form), plain forms first.
fn candidates(d: &Data, word: &str) -> Vec<(String, char, Form)> {
    let mut out: Vec<(String, char, Form)> = Vec::new();
    let mut add = |lemma: String, pos: char, form: Form| {
        if !out.iter().any(|(l, p, _)| *l == lemma && *p == pos) {
            out.push((lemma, pos, form));
        }
    };
    for pos in POS {
        if d.has(word, pos) {
            add(word.to_string(), pos, Form::Base);
        }
    }
    for pos in ['n', 'v', 'a'] {
        for lemma in d.exc_lemmas(word, pos) {
            if d.has(lemma, pos) {
                let form = match pos {
                    'n' => Form::Plural,
                    'v' if word.ends_with("ing") => Form::Ing,
                    'v' => Form::Past,
                    _ if word.ends_with("est") => Form::Superlative,
                    _ => Form::Comparative,
                };
                add(lemma.to_string(), pos, form);
            }
        }
        let rules = match pos {
            'n' => NOUN_RULES,
            'v' => VERB_RULES,
            _ => ADJ_RULES,
        };
        for (suffix, with, form) in rules {
            let Some(stem) = word.strip_suffix(suffix) else { continue };
            if stem.is_empty() {
                continue;
            }
            let lemma = format!("{stem}{with}");
            if d.has(&lemma, pos) {
                add(lemma, pos, *form);
            } else if with.is_empty()
                && matches!(form, Form::Past | Form::Ing | Form::Comparative | Form::Superlative)
                && let Some(undoubled) = undouble(stem)
                && d.has(undoubled, pos)
            {
                // stopped → stop, bigger → big.
                add(undoubled.to_string(), pos, *form);
            }
        }
    }
    out
}

/// `stopp` → `stop`: a stem ending in a doubled consonant, undoubled.
fn undouble(stem: &str) -> Option<&str> {
    let mut rev = stem.chars().rev();
    let (a, b) = (rev.next()?, rev.next()?);
    (a == b && is_consonant(a)).then(|| stem.get(..stem.len() - a.len_utf8())).flatten()
}

fn is_vowel(c: char) -> bool {
    matches!(c, 'a' | 'e' | 'i' | 'o' | 'u')
}

fn is_consonant(c: char) -> bool {
    c.is_ascii_alphabetic() && !is_vowel(c)
}

/// One syllable ending consonant–vowel–consonant (`stop`, `big`, not `fix` or `open`): doubles
/// its last letter before `-ed`, `-ing`, `-er`, `-est`.
fn doubles(w: &str) -> bool {
    let c: Vec<char> = w.chars().collect();
    let [.., x, y, z] = c[..] else { return false };
    if !w.is_ascii() {
        return false;
    }
    let groups = c.iter().zip(std::iter::once(&' ').chain(c.iter())).filter(|(ch, prev)| is_vowel(**ch) && !is_vowel(**prev)).count();
    is_consonant(x) && is_vowel(y) && is_consonant(z) && !matches!(z, 'w' | 'x' | 'y') && groups == 1
}

/// `-s` / `-es` / `-ies`, for plurals and third persons.
fn add_s(w: &str) -> String {
    if ["s", "x", "z", "ch", "sh"].iter().any(|s| w.ends_with(s)) || (w.ends_with('o') && w.len() <= 3) {
        format!("{w}es")
    } else if let Some(stem) = w.strip_suffix('y')
        && stem.chars().last().is_some_and(is_consonant)
    {
        format!("{stem}ies")
    } else {
        format!("{w}s")
    }
}

/// `-ed`, `-ing`, `-er` or `-est` on a regular word.
fn add_suffix(w: &str, suffix: &str) -> String {
    let starts_vowel = suffix.starts_with('e') || suffix.starts_with('i');
    if suffix == "ing" {
        if let Some(stem) = w.strip_suffix("ie") {
            return format!("{stem}ying");
        }
        if let Some(stem) = w.strip_suffix('e')
            && !w.ends_with("ee")
            && !w.ends_with("ye")
            && !w.ends_with("oe")
            && !stem.is_empty()
        {
            return format!("{stem}ing");
        }
    } else if starts_vowel {
        if w.ends_with('e') {
            return format!("{w}{}", suffix.get(1..).unwrap_or(""));
        }
        if let Some(stem) = w.strip_suffix('y')
            && stem.chars().last().is_some_and(is_consonant)
        {
            return format!("{stem}i{suffix}");
        }
    }
    if doubles(w)
        && let Some(last) = w.chars().last()
    {
        return format!("{w}{last}{suffix}");
    }
    format!("{w}{suffix}")
}

/// `word` (a lemma, maybe several words) put in `form` as part of speech `pos`.
fn inflect(d: &Data, word: &str, pos: char, form: Form) -> String {
    if form == Form::Base || word.chars().any(|c| c.is_uppercase()) {
        return word.to_string();
    }
    // A verb phrase inflects its first word (look into → looking into); a noun its last.
    let (head, rest, verbish) = match pos {
        'v' => match word.split_once(' ') {
            Some((h, r)) => (h, format!(" {r}"), true),
            None => (word, String::new(), true),
        },
        _ => match word.rsplit_once(' ') {
            Some((pre, h)) if pos == 'n' => return format!("{pre} {}", inflect(d, h, pos, form)),
            Some(_) => return comparative_phrase(word, form),
            None => (word, String::new(), false),
        },
    };
    let irregular = d.exc_forms(head, pos);
    let pick = |want: &dyn Fn(&str) -> bool| irregular.iter().copied().find(|f| want(f)).map(str::to_string);
    let out = match form {
        Form::Plural => pick(&|_| true).unwrap_or_else(|| add_s(head)),
        Form::Third => match head {
            "be" => "is".into(),
            "have" => "has".into(),
            _ => add_s(head),
        },
        Form::Past => match head {
            "be" => "was".into(),
            // The past, not the participle, when both are irregular (went, not gone).
            _ => pick(&|f| !f.ends_with("ing") && !(irregular.len() > 1 && (f.ends_with('n') || f.ends_with("ne"))))
                .unwrap_or_else(|| add_suffix(head, "ed")),
        },
        Form::Ing => add_suffix(head, "ing"),
        Form::Comparative if !verbish => {
            pick(&|f| f.ends_with("er")).unwrap_or_else(|| if long_adjective(head) { format!("more {head}") } else { add_suffix(head, "er") })
        }
        Form::Superlative if !verbish => {
            pick(&|f| f.ends_with("est")).unwrap_or_else(|| if long_adjective(head) { format!("most {head}") } else { add_suffix(head, "est") })
        }
        _ => head.to_string(),
    };
    format!("{out}{rest}")
}

fn comparative_phrase(word: &str, form: Form) -> String {
    match form {
        Form::Comparative => format!("more {word}"),
        Form::Superlative => format!("most {word}"),
        _ => word.to_string(),
    }
}

/// Adjectives of more than two syllables (or two not ending in -y) compare with more / most.
fn long_adjective(w: &str) -> bool {
    let c: Vec<char> = w.chars().collect();
    let groups = c.iter().zip(std::iter::once(&' ').chain(c.iter())).filter(|(ch, prev)| is_vowel(**ch) && !is_vowel(**prev)).count();
    groups > 2 || (groups == 2 && !w.ends_with('y') && !w.ends_with("le") && !w.ends_with("ow"))
}

/// `syn` capitalised like `like`: ALL CAPS, Capitalised or as it is.
fn match_case(syn: &str, like: &str) -> String {
    let letters: Vec<char> = like.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.len() > 1 && letters.iter().all(|c| c.is_uppercase()) {
        return syn.to_uppercase();
    }
    if like.chars().next().is_some_and(char::is_uppercase) {
        let mut c = syn.chars();
        return match c.next() {
            Some(f) => f.to_uppercase().chain(c).collect(),
            None => String::new(),
        };
    }
    syn.to_string()
}

/// One meaning of the word looked up.
#[derive(Debug, Clone, PartialEq)]
pub struct Sense {
    pub lemma: String,
    pub pos: &'static str,
    pub gloss: String,
    pub synonyms: Vec<String>,
    pub antonyms: Vec<String>,
}

/// The meanings of `word` (any form, any case), its synonyms in the same form and case.
pub fn lookup(word: &str) -> Vec<Sense> {
    let word = word.trim();
    if word.is_empty() || word.chars().count() > MAX_WORD {
        return Vec::new();
    }
    let Some(d) = data() else { return Vec::new() };
    // Curly apostrophes as typed in documents; WordNet's are straight.
    let lower = word.to_lowercase().replace('\u{2019}', "'");
    let mut out = Vec::new();
    // The reading with the most meanings first: `good` is an adjective before it is a noun.
    let mut readings: Vec<(String, char, Form, Vec<usize>)> = candidates(d, &lower)
        .into_iter()
        .map(|(l, p, f)| {
            let ids = d.lookup(&l, p);
            (l, p, f, ids)
        })
        .collect();
    readings.sort_by_key(|r| std::cmp::Reverse(r.3.len()));
    for (lemma, pos, form, ids) in readings {
        for id in ids.into_iter().take(MAX_SENSES_PER_LEMMA) {
            if out.len() >= MAX_SENSES {
                return out;
            }
            let Some(line) = d.synsets.get(id).map(|r| d.line(*r)) else { continue };
            let mut f = line.split('\t');
            let (_, gloss, words, ants) = (f.next(), f.next().unwrap_or(""), f.next().unwrap_or(""), f.next().unwrap_or(""));
            let put = |w: &str| match_case(&inflect(d, w, pos, form), word);
            let mut synonyms: Vec<String> = Vec::new();
            // Not the word itself, nor its own forms (better and best are no synonyms of good).
            let own_form = |w: &str| d.exc_lemmas(w, pos).contains(&lemma.as_str());
            for w in words.split('|').filter(|w| !w.is_empty() && !w.eq_ignore_ascii_case(&lemma) && !own_form(w)) {
                let s = put(w);
                if !s.eq_ignore_ascii_case(word) && !synonyms.contains(&s) {
                    synonyms.push(s);
                }
            }
            let antonyms: Vec<String> = ants.split('|').filter(|w| !w.is_empty()).map(put).collect();
            if synonyms.is_empty() && antonyms.is_empty() {
                continue;
            }
            out.push(Sense { lemma: lemma.clone(), pos: pos_name(pos), gloss: gloss.to_string(), synonyms, antonyms });
        }
    }
    out
}

/// The word the thesaurus looks up in the text: the selection when it's within one paragraph,
/// else the word at the caret, with where it is.
fn word_in_text(s: &Session) -> Option<(String, wordcraft_doc::Pos, wordcraft_doc::Pos)> {
    let (a, b) = s.sel.ordered();
    if a != b {
        if a.story != b.story || a.path != b.path {
            return None;
        }
        let p = s.doc.para_at(&a)?;
        let text = p.text.get(a.off..b.off)?;
        let lead = text.len() - text.trim_start().len();
        let word = text.trim();
        let (mut from, mut to) = (a.clone(), a.clone());
        from.off = a.off + lead;
        to.off = from.off + word.len();
        return Some((word.to_string(), from, to));
    }
    let p = s.doc.para_at(&a)?;
    let (x, y) = p.word_at(a.off);
    let word = p.text.get(x..y)?.trim_end();
    let (mut from, mut to) = (a.clone(), a.clone());
    from.off = x;
    to.off = x + word.len();
    Some((word.to_string(), from, to))
}

fn run(s: &mut Session, v: &Value) -> CmdResult {
    let (word, range) = match p::str(v, "word") {
        Some(w) => (w.trim().to_string(), None),
        None => match word_in_text(s) {
            Some((w, a, b)) => (w, Some(json!({"anchor": pos_json(&a), "focus": pos_json(&b)}))),
            None => (String::new(), None),
        },
    };
    let senses = lookup(&word);
    let mut flat: Vec<&str> = Vec::new();
    // A few from each meaning, so the short list spans them.
    for w in senses.iter().flat_map(|s| s.synonyms.iter().take(3)) {
        if flat.len() < MAX_FLAT && !flat.contains(&w.as_str()) {
            flat.push(w);
        }
    }
    let senses_json: Vec<Value> =
        senses.iter().map(|s| json!({"lemma": s.lemma, "pos": s.pos, "gloss": s.gloss, "synonyms": s.synonyms, "antonyms": s.antonyms})).collect();
    let mut out = json!({"word": word, "synonyms": flat, "senses": senses_json});
    if let (Some(r), Some(o)) = (range, out.as_object_mut()) {
        o.insert("range".into(), r);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn syns(word: &str) -> Vec<String> {
        lookup(word).into_iter().flat_map(|s| s.synonyms).collect()
    }

    #[test]
    fn the_data_loads() {
        let d = data().expect("thesaurus data");
        assert!(d.synsets.len() > 50_000, "{} meanings", d.synsets.len());
        assert!(d.index.len() > 50_000, "{} lemmas", d.index.len());
    }

    #[test]
    fn common_words_have_synonyms() {
        for w in ["good", "investment", "improve", "happy", "run", "quickly", "important", "money", "development"] {
            assert!(!syns(w).is_empty(), "no synonyms for {w}");
        }
        assert!(syns("good").iter().any(|s| s == "well" || s == "beneficial" || s == "estimable"), "{:?}", syns("good"));
    }

    #[test]
    fn inflected_words_get_synonyms_in_the_same_form() {
        let s = syns("improving");
        assert!(s.iter().any(|w| w == "bettering"), "{s:?}");
        assert!(s.iter().all(|w| !w.contains(' ') || w.split(' ').next().is_some_and(|h| h.ends_with("ing")) || w.ends_with("ing")), "{s:?}");
        let s = syns("investments");
        assert!(s.iter().any(|w| w == "investings" || w == "investitures" || w.ends_with('s')), "{s:?}");
        let s = syns("improved");
        assert!(s.iter().any(|w| w == "bettered"), "{s:?}");
        // Irregular forms: mice → mouse, went → go.
        assert!(lookup("mice").iter().any(|s| s.lemma == "mouse"));
        assert!(lookup("went").iter().any(|s| s.lemma == "go"));
        assert!(lookup("stopped").iter().any(|s| s.lemma == "stop"));
    }

    #[test]
    fn case_follows_the_word() {
        assert!(syns("Improving").iter().any(|w| w == "Bettering"));
        assert!(syns("GOOD").iter().all(|w| w.chars().filter(|c| c.is_alphabetic()).all(char::is_uppercase)));
    }

    #[test]
    fn inflection_rules() {
        let d = data().expect("thesaurus data");
        assert_eq!(inflect(d, "stop", 'v', Form::Ing), "stopping");
        assert_eq!(inflect(d, "make", 'v', Form::Ing), "making");
        assert_eq!(inflect(d, "die", 'v', Form::Ing), "dying");
        assert_eq!(inflect(d, "see", 'v', Form::Ing), "seeing");
        assert_eq!(inflect(d, "open", 'v', Form::Ing), "opening");
        assert_eq!(inflect(d, "carry", 'v', Form::Past), "carried");
        assert_eq!(inflect(d, "go", 'v', Form::Past), "went");
        assert_eq!(inflect(d, "look into", 'v', Form::Ing), "looking into");
        assert_eq!(inflect(d, "box", 'n', Form::Plural), "boxes");
        assert_eq!(inflect(d, "city", 'n', Form::Plural), "cities");
        assert_eq!(inflect(d, "mouse", 'n', Form::Plural), "mice");
        assert_eq!(inflect(d, "big", 'a', Form::Comparative), "bigger");
        assert_eq!(inflect(d, "good", 'a', Form::Superlative), "best");
        assert_eq!(inflect(d, "beautiful", 'a', Form::Comparative), "more beautiful");
    }

    #[test]
    fn hostile_words_find_nothing_and_never_panic() {
        for w in ["", " ", "zzzzqqq", "'", "ß", "ﬁ", "\u{301}", "a\u{0}b", &"x".repeat(10_000), "ies", "s", "ing", "ed", "ﬀing"] {
            let _ = lookup(w);
        }
        assert!(lookup(&"good ".repeat(100)).is_empty());
    }

    #[test]
    fn corrupt_data_is_an_error() {
        assert!(parse(b"not deflate").is_err());
        assert!(parse(&miniz_oxide::deflate::compress_to_vec(b"WCTHES0\n", 6)).is_err());
        assert!(parse(&miniz_oxide::deflate::compress_to_vec(b"WCTHES1\nstray\n", 6)).is_err());
        assert!(parse(&miniz_oxide::deflate::compress_to_vec(b"WCTHES1\n==index\nb\tn0\na\tn0\n", 6)).is_err());
        let d = parse(&miniz_oxide::deflate::compress_to_vec(b"WCTHES1\n==synsets\nn\tg\ta|b\t\n==index\na\tn0,zz\n", 6)).expect("tiny");
        assert_eq!(d.lookup("a", 'n'), vec![0]);
    }

    #[test]
    fn the_command_reads_the_word_at_the_caret_and_says_where_it_is() {
        let mut s = Session::new(wordcraft_doc::Document::new());
        s.run("text.insert", &json!({"text": "Prices are improving fast"})).expect("type");
        s.run("select.text", &json!({"text": "improving"})).expect("select");
        let r = s.run("review.thesaurus", &json!({})).expect("thesaurus");
        assert_eq!(r["word"], "improving");
        assert!(r["synonyms"].as_array().is_some_and(|a| !a.is_empty()), "{r}");
        assert_eq!(r["range"]["anchor"]["off"], 11);
        assert_eq!(r["range"]["focus"]["off"], 20);
        // The caret inside the word, nothing selected: the same word.
        s.run("caret.set", &json!({"pos": {"story": "body", "path": [0], "off": 14}})).expect("caret");
        let r = s.run("review.thesaurus", &json!({})).expect("thesaurus");
        assert_eq!(r["word"], "improving");
        assert_eq!(r["range"]["focus"]["off"], 20);
        // A word given: no range.
        let r = s.run("review.thesaurus", &json!({"word": "Happy"})).expect("thesaurus");
        assert!(r.get("range").is_none());
        assert!(r["senses"][0]["synonyms"].as_array().is_some_and(|a| a.iter().any(|w| w == "Glad" || w == "Cheerful")), "{r}");
    }
}
