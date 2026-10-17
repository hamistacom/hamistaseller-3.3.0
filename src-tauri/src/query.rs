//! What the user typed in «کالا»: one or more products, how they combine, and what to leave out.
//!
//! Syntax (the product page builds it from chips; typing it works too):
//!   `عسل یک کیلویی | عسل شهد آزادی -موم`
//!   * `|` or « یا »: alternatives — an offer about ANY of them is kept (each is searched);
//!   * words inside one alternative are ALL required (spelling-tolerant, see market::matches);
//!   * `-word`: offers whose title has that word are left out.
//!
//! Spelling never zeroes a search: Persian/Arabic letters and digits, half-spaces, number words
//! («یک»، «نیم»، «دو») and units («کیلو»، «کیلویی»، «kg»، «۱۰۰۰ گرم») are normalised, so
//! «عسل یک کیلویی», «عسل 1kg», «عسل ۱۰۰۰ گرمی» are the same request. A size in the request must match
//! the size in the title (1 kg ≠ 500 g); joined / split spellings («شهدآزادی» / «شهد آزادی») match.
use crate::market::{norm, size_grams, tok_eq, words};

const NUM_WORDS: &[(&str, &str)] = &[
    ("نیم", "0.5"),
    ("یک", "1"),
    ("یه", "1"),
    ("دو", "2"),
    ("سه", "3"),
    ("چهار", "4"),
    ("پنج", "5"),
    ("شش", "6"),
    ("شیش", "6"),
    ("هفت", "7"),
    ("هشت", "8"),
    ("نه", "9"),
    ("ده", "10"),
    ("دوازده", "12"),
    ("بیست", "20"),
    ("صد", "100"),
    ("دویست", "200"),
    ("سیصد", "300"),
    ("پانصد", "500"),
    ("هزار", "1000"),
];

/// Unit words a number may be written with («کیلویی», «گرمی», «لیتری», «kg» …).
fn is_unit(w: &str) -> bool {
    ["کیلو", "کیلوگرم", "گرم", "لیتر", "ليتر", "میلی", "سی‌سی", "سی", "مثقال", "kg", "gr", "g", "ml", "l", "cc"]
        .iter()
        .any(|u| w == *u || (u.chars().count() >= 2 && w.starts_with(u)))
}

/// Number words right before a unit become digits: «یک کیلویی» → «1 کیلویی», «نیم کیلو» → «0.5 کیلو».
/// Elsewhere they stay words («چهل گیاه» is a honey variety, not 40).
pub fn number_words(s: &str) -> String {
    let n = norm(s);
    let toks: Vec<&str> = n.split_whitespace().collect();
    let mut out: Vec<String> = Vec::with_capacity(toks.len());
    for (i, t) in toks.iter().enumerate() {
        let next = toks.get(i + 1).copied().unwrap_or("");
        match NUM_WORDS.iter().find(|(w, _)| w == t) {
            Some((_, d)) if is_unit(next) => out.push((*d).to_string()),
            _ => out.push((*t).to_string()),
        }
    }
    out.join(" ")
}

/// The pack size a text states, in grams / millilitres (number words understood).
pub fn size_of(text: &str) -> Option<f64> {
    size_grams(&number_words(text))
}

/// One alternative: its required words (sizes taken out) and the size it asks for.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Alt {
    /// As typed (what the platforms are searched with).
    pub text: String,
    pub words: Vec<String>,
    pub size: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Spec {
    pub alts: Vec<Alt>,
    pub exclude: Vec<String>,
}

fn is_number(w: &str) -> bool {
    !w.is_empty() && w.chars().all(|c| c.is_ascii_digit() || c == '.')
}

impl Spec {
    pub fn parse(q: &str) -> Spec {
        let mut spec = Spec::default();
        let q = q.replace(" یا ", " | ").replace('،', " | ");
        for part in q.split('|') {
            let mut keep: Vec<&str> = vec![];
            for tok in part.split_whitespace() {
                match tok.strip_prefix('-').or_else(|| tok.strip_prefix('−')) {
                    Some(x) if !x.is_empty() => spec.exclude.extend(words(x)),
                    _ => keep.push(tok),
                }
            }
            let text = keep.join(" ");
            if text.trim().is_empty() {
                continue;
            }
            let size = size_of(&text);
            let nw = words(&number_words(&text));
            // the size is matched as a number, not as words («1» + «کیلویی» need not be in the title)
            let words: Vec<String> = if size.is_some() {
                let mut v = vec![];
                let mut skip_unit = false;
                for w in &nw {
                    if is_number(w) {
                        skip_unit = true;
                        continue;
                    }
                    if skip_unit && is_unit(w) {
                        skip_unit = false;
                        continue;
                    }
                    // «1kg» written together
                    if w.chars().next().is_some_and(|c| c.is_ascii_digit()) && is_unit(w.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.')) {
                        continue;
                    }
                    skip_unit = false;
                    v.push(w.clone());
                }
                v
            } else {
                nw
            };
            spec.alts.push(Alt { text: text.trim().to_string(), words, size });
        }
        spec.exclude.sort();
        spec.exclude.dedup();
        spec
    }

    /// The first alternative's words (the product's name: brands and sub-categories are mined around it).
    pub fn head(&self) -> Vec<String> {
        self.alts.first().map(|a| a.words.clone()).unwrap_or_default()
    }

    /// What each platform is searched for.
    pub fn searches(&self) -> Vec<String> {
        let mut v: Vec<String> = vec![];
        for a in &self.alts {
            if !v.iter().any(|x| words(x) == words(&a.text)) {
                v.push(a.text.clone());
            }
        }
        v
    }

    /// The offer's title is about one of the alternatives and names none of the excluded words.
    pub fn matches(&self, title: &str, exact: bool) -> bool {
        let tw = words(title);
        if self.exclude.iter().any(|x