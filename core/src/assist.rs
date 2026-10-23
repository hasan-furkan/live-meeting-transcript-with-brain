use serde::{Deserialize, Serialize};
use ts_rs::TS;

// Reasoning models spend output tokens on a thinking block before the text: with 800 the
// JSON was cut off mid-answer, and with 5 the YES/NO never arrived.
pub const MAX_TOKENS_ASSIST: u32 = 4000;
pub const MAX_TOKENS_CLASSIFY: u32 = 1000;

const ASSIST_HEAD: &str = "You help a university student during a live English lecture. The student was just addressed by the lecturer. From the transcript excerpt (and the screenshot, if provided) identify the question the lecturer asked the student and write a concise answer the student can say aloud (at most 3 sentences). Reply with ONLY a JSON object: {";
const ASSIST_FIELDS: &str = "\"question\": string, \"suggested_answer\": string, \"confidence\": \"low\"|\"medium\"|\"high\"";

const CLASSIFY_SYSTEM: &str = "Decide whether the last part of this lecture transcript contains a question the lecturer addressed to the class and expects a student to answer now. Rhetorical questions do not count. Reply with only YES or NO.";

#[derive(Serialize, TS, Clone, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Confidence {
    Low,
    Medium,
    High,
}

#[derive(Serialize, TS, Clone, Debug, PartialEq)]
#[ts(export)]
pub struct Analysis {
    pub question: String,
    pub suggested_answer: String,
    pub confidence: Option<Confidence>,
    pub explanation: Option<String>,
    pub raw: Option<String>,
}

pub fn assist_prompt(
    course: &str,
    recent: &str,
    has_image: bool,
    explanation_language: &str,
) -> (String, String) {
    let language = sanitize_language(explanation_language);
    let mut fields = ASSIST_FIELDS.to_string();
    if !language.is_empty() {
        fields.push_str(", \"explanation\": string");
    }
    let shape = format!("{{{fields}}}.");
    let mut system = format!("{ASSIST_HEAD}{shape}");
    if !language.is_empty() {
        system.push_str(&format!(
            " Also add \"explanation\": 1-2 sentences in {language} explaining what the question asks and why the answer is correct. Keep \"question\" and \"suggested_answer\" in the lecture's language."
        ));
    }

    let mut user = format!("Course: {course}\n\nTranscript (most recent last):\n{recent}");
    if has_image {
        user.push_str("\n\nA screenshot of the lecture screen is attached.");
    }
    (system, user)
}

/// Keeps the language from breaking out of the prompt or the JSON shape.
fn sanitize_language(language: &str) -> String {
    language
        .trim()
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, '"' | '{' | '}'))
        .take(40)
        .collect()
}

#[derive(Deserialize)]
struct ParsedAnalysis {
    #[serde(default)]
    question: String,
    #[serde(default)]
    suggested_answer: String,
    #[serde(default)]
    confidence: Option<String>,
    #[serde(default)]
    explanation: Option<String>,
}

/// Reads the first JSON object in the reply; a cut-off or malformed one is salvaged field
/// by field, so the card shows text rather than raw JSON.
pub fn parse_analysis(reply: &str) -> Analysis {
    let parsed = first_object(reply).or_else(|| salvage(reply));
    match parsed {
        Some(parsed) => Analysis {
            question: parsed.question,
            suggested_answer: parsed.suggested_answer,
            confidence: parse_confidence(parsed.confidence.as_deref()),
            explanation: parsed.explanation.filter(|text| !text.trim().is_empty()),
            raw: None,
        },
        None => Analysis {
            question: String::new(),
            suggested_answer: String::new(),
            confidence: None,
            explanation: None,
            raw: Some(reply.to_string()),
        },
    }
}

/// The first JSON object that carries a question or an answer; `{}` in prose is skipped.
fn first_object(reply: &str) -> Option<ParsedAnalysis> {
    reply.match_indices('{').find_map(|(start, _)| {
        serde_json::Deserializer::from_str(&reply[start..])
            .into_iter::<ParsedAnalysis>()
            .next()?
            .ok()
            .filter(|parsed| !parsed.question.is_empty() || !parsed.suggested_answer.is_empty())
    })
}

fn salvage(reply: &str) -> Option<ParsedAnalysis> {
    let parsed = ParsedAnalysis {
        question: string_field(reply, "question").unwrap_or_default(),
        suggested_answer: string_field(reply, "suggested_answer").unwrap_or_default(),
        confidence: string_field(reply, "confidence"),
        explanation: string_field(reply, "explanation"),
    };
    (!parsed.question.is_empty() || !parsed.suggested_answer.is_empty()).then_some(parsed)
}

/// The string value of `"key": "..."`, decoding escapes; a value cut off by the token limit
/// ends with an ellipsis. Raw newlines, which strict JSON rejects, are kept.
fn string_field(text: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{key}\"");
    // Skip mentions that are not `"key": "` (e.g. the schema echoed as `"question": string`).
    let value = text.match_indices(&pattern).find_map(|(at, _)| {
        let after_key = &text[at + pattern.len()..];
        after_key
            .trim_start()
            .strip_prefix(':')?
            .trim_start()
            .strip_prefix('"')
    })?;
    let mut out = String::new();
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('u') => {
                    let hex: String = chars
                        .clone()
                        .take(4)
                        .take_while(char::is_ascii_hexdigit)
                        .collect();
                    for _ in 0..hex.len() {
                        chars.next();
                    }
                    let decoded = (hex.len() == 4)
                        .then(|| u32::from_str_radix(&hex, 16).ok())
                        .flatten()
                        .and_then(char::from_u32);
                    // shortcut: surrogate pairs (emoji) are dropped; the JSON path handles them.
                    if let Some(decoded) = decoded {
                        out.push(decoded);
                    }
                }
                Some(other) => out.push(other),
                None => break,
            },
            other => out.push(other),
        }
    }
    let cut = out.trim_end();
    (!cut.is_empty()).then(|| format!("{cut}…"))
}

fn parse_confidence(value: Option<&str>) -> Option<Confidence> {
    match value?.trim().to_ascii_lowercase().as_str() {
        "low" => Some(Confidence::Low),
        "medium" => Some(Confidence::Medium),
        "high" => Some(Confidence::High),
        _ => None,
    }
}

pub fn classify_prompt(course: &str, recent: &str) -> (String, String) {
    (
        CLASSIFY_SYSTEM.to_string(),
        format!("Course: {course}\n\n{recent}"),
    )
}

pub fn parse_classify(reply: &str) -> bool {
    reply
        .trim()
        .trim_end_matches(|c: char| c.is_ascii_punctuation())
        .eq_ignore_ascii_case("yes")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_analysis_plain_fenced_and_garbage() {
        let plain = r#"{"question":"What is O(n log n)?","suggested_answer":"A growth bound.","confidence":"high"}"#;
        let analysis = parse_analysis(plain);
        assert_eq!(analysis.question, "What is O(n log n)?");
        assert_eq!(analysis.suggested_answer, "A growth bound.");
        assert_eq!(analysis.confidence, Some(Confidence::High));
        assert_eq!(analysis.explanation, None);
        assert_eq!(analysis.raw, None);

        let fenced = format!("```json\n{plain}\n```");
        let analysis = parse_analysis(&fenced);
        assert_eq!(analysis.question, "What is O(n log n)?");
        assert_eq!(analysis.confidence, Some(Confidence::High));
        assert_eq!(analysis.raw, None);

        let garbage = "I think the answer is 42.";
        let analysis = parse_analysis(garbage);
        assert_eq!(analysis.question, "");
        assert_eq!(analysis.suggested_answer, "");
        assert_eq!(analysis.confidence, None);
        assert_eq!(analysis.raw, Some(garbage.to_string()));
    }

    #[test]
    fn parse_analysis_ignores_text_and_braces_after_the_object() {
        let reply = r#"Here you go: {"question":"Q","suggested_answer":"A","confidence":"low"} Note: use {} for dicts."#;
        let analysis = parse_analysis(reply);
        assert_eq!(analysis.question, "Q");
        assert_eq!(analysis.suggested_answer, "A");
        assert_eq!(analysis.raw, None);
    }

    #[test]
    fn parse_analysis_salvages_reply_cut_by_token_limit() {
        // Real shape from the API when max_tokens ran out mid-answer.
        let reply = r#"{"question":"How would you count words?","suggested_answer":"Split the sentence: def word_count(s): counts = {}; for w in s.split(): counts[w] = counts.get(w, 0) + "#;
        let analysis = parse_analysis(reply);
        assert_eq!(analysis.question, "How would you count words?");
        assert_eq!(
            analysis.suggested_answer,
            "Split the sentence: def word_count(s): counts = {}; for w in s.split(): counts[w] = counts.get(w, 0) +…"
        );
        assert_eq!(analysis.confidence, None);
        assert_eq!(analysis.raw, None);
    }

    #[test]
    fn parse_analysis_salvages_raw_newlines_and_escapes() {
        // A literal newline inside the string makes strict JSON parsing fail.
        let reply = "{\"question\":\"Say \\\"hi\\\"\",\"suggested_answer\":\"line1\nline2 \\u00e9\",\"confidence\":\"high\"}";
        let analysis = parse_analysis(reply);
        assert_eq!(analysis.question, "Say \"hi\"");
        assert_eq!(analysis.suggested_answer, "line1\nline2 é");
        assert_eq!(analysis.confidence, Some(Confidence::High));
        assert_eq!(analysis.raw, None);
    }

    #[test]
    fn parse_analysis_skips_empty_object_before_the_answer() {
        let reply = r#"Use counts = {} then: {"question":"Q","suggested_answer":"A"}"#;
        let analysis = parse_analysis(reply);
        assert_eq!(analysis.question, "Q");
        assert_eq!(analysis.suggested_answer, "A");
        assert_eq!(analysis.raw, None);
    }

    #[test]
    fn salvage_skips_echoed_schema_and_short_unicode_escape() {
        let reply = "Schema: \"question\": string. {\"question\":\"Q \\u00\",\"suggested_answer\":\"A\nB\"}";
        let analysis = parse_analysis(reply);
        assert_eq!(analysis.question, "Q ");
        assert_eq!(analysis.suggested_answer, "A\nB");
    }

    #[test]
    fn salvage_handles_odd_inputs_without_panicking() {
        for reply in [
            "",
            "\"question\"",
            "\"question\":",
            "\"question\": \"",
            "\"question\": \"\\",
            "{\"question\": \"é",
        ] {
            let analysis = parse_analysis(reply);
            assert_eq!(analysis.suggested_answer, "", "{reply}");
        }
    }

    #[test]
    fn parse_analysis_unknown_confidence_is_none() {
        let analysis =
            parse_analysis(r#"{"question":"Q","suggested_answer":"A","confidence":"certain"}"#);
        assert_eq!(analysis.question, "Q");
        assert_eq!(analysis.confidence, None);
    }

    #[test]
    fn parse_analysis_reads_explanation_and_treats_empty_as_none() {
        let analysis = parse_analysis(
            r#"{"question":"Q","suggested_answer":"A","explanation":"The question asks what a heap is."}"#,
        );
        assert_eq!(
            analysis.explanation.as_deref(),
            Some("The question asks what a heap is.")
        );

        let empty = parse_analysis(r#"{"question":"Q","suggested_answer":"A","explanation":"  "}"#);
        assert_eq!(empty.explanation, None);
    }

    #[test]
    fn parse_classify_variants() {
        assert!(parse_classify("YES"));
        assert!(parse_classify("yes."));
        assert!(parse_classify("  Yes  "));
        assert!(!parse_classify("No"));
        assert!(!parse_classify("maybe"));
    }

    #[test]
    fn prompts_contain_course_and_image_note() {
        let (system, user) = assist_prompt("Algorithms 101", "What is a heap?", false, "");
        assert!(system.contains("\"question\""));
        assert_eq!(system, format!("{ASSIST_HEAD}{{{ASSIST_FIELDS}}}."));
        assert!(!system.contains("explanation"));
        assert!(user.contains("Course: Algorithms 101"));
        assert!(user.ends_with("What is a heap?"));

        let (_, with_image) = assist_prompt("Algorithms 101", "text", true, "");
        assert!(with_image.ends_with("A screenshot of the lecture screen is attached."));

        let (system, user) = classify_prompt("Algorithms 101", "Any questions?");
        assert!(system.contains("YES or NO"));
        assert_eq!(user, "Course: Algorithms 101\n\nAny questions?");
    }

    #[test]
    fn explanation_language_adds_prompt_sentence_and_json_field() {
        let (system, _) = assist_prompt("Algorithms 101", "text", false, "  Turkish  ");
        assert!(system.contains("in Turkish explaining"));
        assert!(system.contains("\"explanation\": string"));
        assert!(system.contains("\"explanation\": 1-2 sentences"));
        assert!(
            system.contains("Keep \"question\" and \"suggested_answer\" in the lecture's language")
        );
    }

    #[test]
    fn language_is_sanitised_and_capped_at_40_chars() {
        assert_eq!(sanitize_language("  Turkish  "), "Turkish");
        assert_eq!(sanitize_language("De\"u{t}s\nch"), "Deutsch");
        assert_eq!(sanitize_language("x".repeat(50).as_str()), "x".repeat(40));
        assert_eq!(sanitize_language("  \n\t "), "");
    }
}
