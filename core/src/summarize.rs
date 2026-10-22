use crate::llm::{LlmClient, LlmError, json_slice};
use crate::vault::SummaryBody;
use serde::Deserialize;

const MAX_WORDS: usize = 12_000;
const MAX_TOKENS_SUMMARY: u32 = 4000;
const MAX_TOKENS_NOTES: u32 = 2000;

const SUMMARY_SYSTEM: &str = "You summarize university lecture transcripts. Reply with ONLY a JSON object: {\"summary\": string, \"notes\": string, \"action_items\": [string]}. Write the summary as a short paragraph. Write notes as a markdown bullet list grouped by topic. Action items are homework, deadlines, exams and readings, with dates if they were stated. Write in English.";

const NOTES_SYSTEM: &str = "Write detailed markdown bullet notes of this lecture transcript part, including any homework, deadlines or exams mentioned.";

pub fn chunk_words(text: &str, max_words: usize) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    words
        .chunks(max_words.max(1))
        .map(|chunk| chunk.join(" "))
        .collect()
}

pub async fn summarize(
    client: &LlmClient,
    course: &str,
    transcript: &str,
) -> Result<SummaryBody, LlmError> {
    let chunks = chunk_words(transcript, MAX_WORDS);
    let reply = if chunks.len() <= 1 {
        let user = format!(
            "Course: {course}\n\nTranscript:\n{}",
            chunks.first().map_or("", String::as_str)
        );
        client
            .complete(SUMMARY_SYSTEM, &user, None, MAX_TOKENS_SUMMARY)
            .await?
    } else {
        let mut parts = Vec::with_capacity(chunks.len());
        for (index, chunk) in chunks.iter().enumerate() {
            let user = format!(
                "Course: {course}\n\nTranscript part {}/{}:\n{chunk}",
                index + 1,
                chunks.len()
            );
            parts.push(
                client
                    .complete(NOTES_SYSTEM, &user, None, MAX_TOKENS_NOTES)
                    .await?,
            );
        }
        let user = format!(
            "Course: {course}\n\nNotes from the transcript parts:\n{}",
            parts.join("\n\n")
        );
        client
            .complete(SUMMARY_SYSTEM, &user, None, MAX_TOKENS_SUMMARY)
            .await?
    };
    Ok(parse_summary(&reply))
}

#[derive(Deserialize)]
struct ParsedSummary {
    #[serde(default)]
    summary: String,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    action_items: Vec<String>,
}

fn parse_summary(reply: &str) -> SummaryBody {
    let parsed: Option<ParsedSummary> =
        json_slice(reply).and_then(|slice| serde_json::from_str(slice).ok());
    match parsed {
        Some(parsed) => SummaryBody {
            summary: parsed.summary,
            notes: parsed.notes,
            action_items: parsed.action_items,
        },
        None => SummaryBody {
            summary: String::new(),
            notes: reply.to_string(),
            action_items: Vec::new(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_http::MockServer;
    use serde_json::{Value, json};

    fn text_reply(text: &str) -> String {
        json!({ "content": [{ "type": "text", "text": text }] }).to_string()
    }

    fn summary_json(summary: &str, notes: &str, actions: &[&str]) -> String {
        json!({ "summary": summary, "notes": notes, "action_items": actions }).to_string()
    }

    fn client(server: &MockServer) -> LlmClient {
        LlmClient::new(&server.url, "test-key".into(), "test-model")
    }

    #[test]
    fn chunk_words_exact_boundaries() {
        let words: Vec<String> = (0..25).map(|i| format!("w{i}")).collect();
        let text = words.join(" ");
        let chunks = chunk_words(&text, 10);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].split_whitespace().count(), 10);
        assert_eq!(chunks[1].split_whitespace().count(), 10);
        assert_eq!(chunks[2].split_whitespace().count(), 5);
        assert_eq!(chunks[0].split_whitespace().next(), Some("w0"));
        assert_eq!(chunks[2].split_whitespace().last(), Some("w24"));
        assert!(chunk_words("", 10).is_empty());
    }

    #[tokio::test]
    async fn summarize_chunks_long_transcript() {
        let text = vec!["lecture"; 30_000].join(" ");
        let server = MockServer::start(vec![
            (200, text_reply("- point one")),
            (200, text_reply("- point two")),
            (200, text_reply("- point three")),
            (
                200,
                text_reply(&summary_json("A lecture.", "- notes", &["Read chapter 2"])),
            ),
        ])
        .await;
        let body = summarize(&client(&server), "Algorithms 101", &text)
            .await
            .unwrap();
        assert_eq!(body.summary, "A lecture.");
        assert_eq!(body.notes, "- notes");
        assert_eq!(body.action_items, vec!["Read chapter 2".to_string()]);

        let requests = server.requests();
        assert_eq!(requests.len(), 4);
        for request in &requests[..3] {
            let sent: Value = serde_json::from_str(&request.body).unwrap();
            assert_eq!(sent["max_tokens"], MAX_TOKENS_NOTES);
            assert_eq!(sent["system"], NOTES_SYSTEM);
        }
        let reduce: Value = serde_json::from_str(&requests[3].body).unwrap();
        assert_eq!(reduce["max_tokens"], MAX_TOKENS_SUMMARY);
        assert_eq!(reduce["system"], SUMMARY_SYSTEM);
        let reduce_text = reduce["messages"][0]["content"][0]["text"]
            .as_str()
            .unwrap();
        assert!(reduce_text.contains("- point two"), "{reduce_text}");
    }

    #[tokio::test]
    async fn summarize_short_transcript_single_call() {
        let server = MockServer::start(vec![(
            200,
            text_reply(&summary_json("Short.", "- note", &[])),
        )])
        .await;
        let body = summarize(&client(&server), "Algorithms 101", "a few words")
            .await
            .unwrap();
        assert_eq!(body.summary, "Short.");
        assert_eq!(body.notes, "- note");
        assert!(body.action_items.is_empty());

        let requests = server.requests();
        assert_eq!(requests.len(), 1);
        let sent: Value = serde_json::from_str(&requests[0].body).unwrap();
        assert_eq!(sent["max_tokens"], MAX_TOKENS_SUMMARY);
        assert_eq!(sent["system"], SUMMARY_SYSTEM);
    }

    #[tokio::test]
    async fn summarize_unparsable_reply_keeps_raw_notes() {
        let server = MockServer::start(vec![(200, text_reply("plain notes"))]).await;
        let body = summarize(&client(&server), "Algorithms 101", "few words")
            .await
            .unwrap();
        assert_eq!(body.summary, "");
        assert_eq!(body.notes, "plain notes");
        assert!(body.action_items.is_empty());
    }
}
