use crate::triggers::normalize;

/// Whisper's usual output on silence or music.
const HALLUCINATION_PHRASES: [&str; 5] = [
    "thank you",
    "thanks for watching",
    "you",
    "bye",
    "subtitles by the amara org community",
];

/// Initial prompt biasing whisper towards the participant names, if enabled and any are set.
pub fn initial_prompt(names: &[String], enabled: bool) -> Option<String> {
    if !enabled {
        return None;
    }
    let names: Vec<&str> = names
        .iter()
        .map(|name| name.trim())
        .filter(|name| !name.is_empty())
        .collect();
    if names.is_empty() {
        return None;
    }
    Some(format!("Participants: {}.", names.join(", ")))
}

/// True when nothing outside bracketed annotations (`[..]`, `(..)`, `*..*`, `♪ ♫`) is
/// alphanumeric, e.g. whisper's `[Music]` or `(applause)`.
fn only_non_speech(text: &str) -> bool {
    let mut close = None;
    for c in text.trim().chars() {
        match close {
            Some(end) => {
                if c == end {
                    close = None;
                }
            }
            None => match c {
                '[' => close = Some(']'),
                '(' => close = Some(')'),
                '*' => close = Some('*'),
                '♪' | '♫' => {}
                c if c.is_alphanumeric() => return false,
                _ => {}
            },
        }
    }
    true
}

/// True when `text` is empty, echoes the initial prompt (or only its words), is a known
/// whisper hallucination phrase, or is only a non-speech annotation.
pub fn is_hallucination(text: &str, prompt: Option<&str>) -> bool {
    let normalised = normalize(text);
    if normalised.is_empty() || only_non_speech(text) {
        return true;
    }
    if let Some(prompt) = prompt {
        let prompt = normalize(prompt);
        if normalised == prompt {
            return true;
        }
        let prompt_tokens: Vec<&str> = prompt
            .split_whitespace()
            .filter(|token| *token != "participants")
            .collect();
        if !prompt_tokens.is_empty()
            && normalised
                .split_whitespace()
                .all(|token| prompt_tokens.contains(&token))
        {
            return true;
        }
    }
    HALLUCINATION_PHRASES.contains(&normalised.as_str())
}

#[cfg(test)]
mod tests {
    use super::{initial_prompt, is_hallucination};

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn prompt_format_and_toggles() {
        assert_eq!(
            initial_prompt(&names(&["Jordan", "Jordan Reyes"]), true).as_deref(),
            Some("Participants: Jordan, Jordan Reyes.")
        );
        assert_eq!(initial_prompt(&names(&[]), true), None);
        assert_eq!(initial_prompt(&names(&["Jordan"]), false), None);
        assert_eq!(initial_prompt(&names(&["", "  "]), true), None);
    }

    #[test]
    fn hallucination_filter_cases() {
        let prompt = initial_prompt(&names(&["Jordan"]), true);
        assert!(is_hallucination("Jordan.", prompt.as_deref()));
        assert!(!is_hallucination(
            "Jordan, what is merge sort",
            prompt.as_deref()
        ));
        assert!(is_hallucination("Thank you.", prompt.as_deref()));
        assert!(is_hallucination("Participants: Jordan.", prompt.as_deref()));
        assert!(!is_hallucination(
            "Jordan Reyes will present",
            prompt.as_deref()
        ));
    }

    #[test]
    fn non_speech_annotations() {
        for text in [
            "[Music]",
            "[BLANK_AUDIO]",
            "(applause)",
            "[ Silence ]",
            "*music*",
            "♪",
            " ♪ ♫ ",
        ] {
            assert!(is_hallucination(text, None), "{text}");
        }
        assert!(!is_hallucination(
            "Jordan [laughs] what is merge sort",
            None
        ));
    }

    #[test]
    fn hallucination_phrases_and_empty_text() {
        assert!(is_hallucination("", None));
        assert!(is_hallucination("  ... ", None));
        assert!(is_hallucination(
            "Subtitles by the Amara.org community",
            None
        ));
        assert!(is_hallucination("you", None));
        assert!(is_hallucination("Bye.", None));
        assert!(is_hallucination("Thanks for watching!", None));
        assert!(!is_hallucination(
            "thank you all for coming to algorithms",
            None
        ));
    }
}
