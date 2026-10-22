use serde::Serialize;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use ts_rs::TS;

/// How long a term stays quiet after it fires.
const COOLDOWN: Duration = Duration::from_secs(20);

const QUESTION_CUES: [&str; 8] = [
    "who can",
    "anyone",
    "anybody",
    "what do you think",
    "can someone",
    "does anyone",
    "any questions",
    "any ideas",
];

/// Lowercase, fold Turkish diacritics, replace punctuation with spaces, collapse whitespace.
pub fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut space = false;
    for c in text.chars() {
        let folded = match c {
            'İ' | 'I' | 'ı' => 'i',
            'Ş' | 'ş' => 's',
            'Ğ' | 'ğ' => 'g',
            'Ü' | 'ü' => 'u',
            'Ö' | 'ö' => 'o',
            'Ç' | 'ç' => 'c',
            _ => c.to_lowercase().next().unwrap_or(c),
        };
        if folded.is_alphanumeric() {
            if space && !out.is_empty() {
                out.push(' ');
            }
            space = false;
            out.push(folded);
        } else {
            space = true;
        }
    }
    out
}

/// Ends with `?` or contains one of the general-question cue phrases.
pub fn looks_like_question(line: &str) -> bool {
    if line.trim_end().ends_with('?') {
        return true;
    }
    let normalized = normalize(line);
    QUESTION_CUES.iter().any(|cue| normalized.contains(cue))
}

#[derive(Serialize, TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub enum HitKind {
    Name,
    Keyword,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    pub kind: HitKind,
    pub term: String,
    pub context: String,
}

struct Entry {
    kind: HitKind,
    term: String,
    tokens: Vec<String>,
    /// Normalised term, also the cooldown key.
    key: String,
}

pub struct Matcher {
    entries: Vec<Entry>,
    last_hit: HashMap<String, Instant>,
}

impl Matcher {
    pub fn new(names: &[String], keywords: &[String]) -> Self {
        let entries = names
            .iter()
            .map(|term| (HitKind::Name, term))
            .chain(keywords.iter().map(|term| (HitKind::Keyword, term)))
            .filter_map(|(kind, term)| {
                let key = normalize(term);
                let tokens: Vec<String> = key.split_whitespace().map(str::to_string).collect();
                if tokens.is_empty() {
                    None
                } else {
                    Some(Entry {
                        kind,
                        term: term.clone(),
                        tokens,
                        key,
                    })
                }
            })
            .collect();
        Self {
            entries,
            last_hit: HashMap::new(),
        }
    }

    pub fn scan(&mut self, text: &str, now: Instant) -> Vec<Hit> {
        let normalized = normalize(text);
        let tokens: Vec<String> = normalized.split_whitespace().map(str::to_string).collect();
        let mut hits = Vec::new();
        for i in 0..self.entries.len() {
            let (term, key) = {
                let entry = &self.entries[i];
                if !entry_matches(entry, &tokens) {
                    continue;
                }
                (entry.term.clone(), entry.key.clone())
            };
            let cooled = self
                .last_hit
                .get(&key)
                .is_none_or(|last| now.duration_since(*last) >= COOLDOWN);
            if !cooled {
                continue;
            }
            self.last_hit.insert(key, now);
            hits.push(Hit {
                kind: self.entries[i].kind.clone(),
                term,
                context: context(text),
            });
        }
        hits
    }
}

fn entry_matches(entry: &Entry, tokens: &[String]) -> bool {
    let n = entry.tokens.len();
    if tokens.len() < n {
        return false;
    }
    let fuzzy = entry.kind == HitKind::Name && entry.key.chars().count() >= 5;
    tokens.windows(n).any(|window| {
        window == entry.tokens.as_slice()
            || (fuzzy && levenshtein(&window.join(" "), &entry.key) <= 1)
    })
}

/// Last 200 chars of the trimmed input, cut on a char boundary.
fn context(text: &str) -> String {
    let trimmed = text.trim();
    let count = trimmed.chars().count();
    if count <= 200 {
        return trimmed.to_string();
    }
    trimmed.chars().skip(count - 200).collect()
}

fn levenshtein(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for (i, ca) in a.chars().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != *cb);
            cur[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

pub struct RateLimiter {
    min_gap: Duration,
    last: Option<Instant>,
}

impl RateLimiter {
    pub fn new(min_gap: Duration) -> Self {
        Self {
            min_gap,
            last: None,
        }
    }

    pub fn allow(&mut self, now: Instant) -> bool {
        if self
            .last
            .is_none_or(|last| now.duration_since(last) >= self.min_gap)
        {
            self.last = Some(now);
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn normalize_folds_turkish() {
        assert_eq!(normalize("JORDAN, Çağrı İşık!"), "jordan cagri isik");
    }

    #[test]
    fn name_exact_and_fuzzy() {
        let mut m = Matcher::new(&["Jordan".into()], &[]);
        let t = Instant::now();
        assert_eq!(m.scan("so, Jordon, what's the answer", t).len(), 1); // distance 1, len 6
    }

    #[test]
    fn short_name_needs_exact() {
        let mut m = Matcher::new(&["Ali".into()], &[]);
        assert!(m.scan("all of you", Instant::now()).is_empty());
        assert!(
            m.scan("Ali please", Instant::now() + Duration::from_secs(30))
                .len()
                == 1
        );
    }

    #[test]
    fn no_substring_match() {
        let mut m = Matcher::new(&["Jordan".into()], &[]);
        assert!(m.scan("jordanian history", Instant::now()).is_empty());
    }

    #[test]
    fn multiword_variant() {
        let mut m = Matcher::new(&["Jordan Reyes".into()], &[]);
        assert_eq!(m.scan("thanks jordan reyes", Instant::now()).len(), 1);
    }

    #[test]
    fn cooldown_20s() {
        let mut m = Matcher::new(&["Jordan".into()], &["exam".into()]);
        let t = Instant::now();
        assert_eq!(m.scan("Jordan", t).len(), 1);
        assert!(m.scan("Jordan", t + Duration::from_secs(19)).is_empty());
        assert_eq!(m.scan("Jordan", t + Duration::from_secs(21)).len(), 1);
    }

    #[test]
    fn keyword_exact_only() {
        let mut m = Matcher::new(&[], &["exam".into()]);
        assert!(m.scan("examples", Instant::now()).is_empty());
        assert_eq!(
            m.scan("the exam is friday", Instant::now())[0].kind,
            HitKind::Keyword
        );
    }

    #[test]
    fn empty_and_whitespace_variants_ignored() {
        let mut m = Matcher::new(&["".into(), "  ".into()], &[]);
        assert!(m.scan("anything at all", Instant::now()).is_empty());
    }

    #[test]
    fn question_heuristic() {
        assert!(looks_like_question("Who can tell me the runtime"));
        assert!(looks_like_question("Is that clear?"));
        assert!(!looks_like_question("This is a statement."));
    }

    #[test]
    fn rate_limiter_30s() {
        let mut r = RateLimiter::new(Duration::from_secs(30));
        let t = Instant::now();
        assert!(r.allow(t));
        assert!(!r.allow(t + Duration::from_secs(29)));
        assert!(r.allow(t + Duration::from_secs(31)));
    }

    #[test]
    fn cooldown_is_per_term() {
        let mut m = Matcher::new(&["Jordan".into()], &["exam".into()]);
        let t = Instant::now();
        assert_eq!(m.scan("Jordan", t).len(), 1);
        assert_eq!(m.scan("exam", t + Duration::from_secs(1)).len(), 1);
    }

    #[test]
    fn fuzzy_needs_five_normalised_chars() {
        let mut m = Matcher::new(&["Jo".into()], &[]);
        assert!(m.scan("Joe", Instant::now()).is_empty());
    }

    #[test]
    fn context_is_trimmed_and_capped_at_200_chars() {
        let mut m = Matcher::new(&["Jordan".into()], &["exam".into()]);
        assert_eq!(m.scan("  Jordan  ", Instant::now())[0].context, "Jordan");

        let long = format!("{} Jordan", "ş".repeat(300));
        let hit = &m.scan(&long, Instant::now() + Duration::from_secs(30))[0];
        assert!(hit.context.ends_with("Jordan"));
        assert_eq!(hit.context.chars().count(), 200);
    }
}
