use crate::llm::json_slice;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const MAX_TOKENS_ASSIST: u32 = 800;
pub const MAX_TOKENS_CLASSIFY: u32 = 5;

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

pub fn parse_analysis(reply: &str) -> Analysis {
    let parsed: Option<ParsedAnalysis> =
        json_slice(reply).and_then(|slice| serde_json::from_str(slice).ok());
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
