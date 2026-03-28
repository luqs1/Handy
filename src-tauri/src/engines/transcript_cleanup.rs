//! Batch transcript cleanup: chunks records into time-based blocks
//! and sends each to an LLM for filler-word removal and punctuation fixes.
//! Translated from: OpenOats/Sources/OpenOats/Intelligence/TranscriptCleanupEngine.swift

use crate::domain::models::SessionRecord;
use regex::Regex;

/// Duration of each chunk in milliseconds (2.5 minutes).
const CHUNK_DURATION_MS: i64 = 150_000;

/// The system prompt instructing the LLM how to clean up transcripts.
pub const CLEANUP_SYSTEM_PROMPT: &str = "\
You are a transcript cleanup assistant. Your job is to clean up raw speech-to-text output.\n\
\n\
Rules:\n\
- Remove filler words (um, uh, like, you know, sort of, kind of, I mean, basically, actually, right, so, well) \
when they add no meaning.\n\
- Fix punctuation and capitalization.\n\
- Preserve the original meaning exactly. Do not rephrase, summarize, or add content.\n\
- Keep the exact same number of lines in the same order.\n\
- Each line starts with a timestamp and speaker prefix in the format: [HH:MM:SS] Speaker: text\n\
- Return the cleaned lines in the same format, one per line.\n\
- Do not add any commentary, explanation, or extra text.";

// Swift: TranscriptCleanupEngine.swift > TranscriptCleanupEngine.chunkRecords(_:)
/// Splits records into chunks of approximately 2.5 minutes based on timestamps.
pub fn chunk_records(records: &[SessionRecord]) -> Vec<Vec<SessionRecord>> {
    let first = match records.first() {
        Some(r) => r,
        None => return Vec::new(),
    };

    let mut chunks: Vec<Vec<SessionRecord>> = Vec::new();
    let mut current_chunk: Vec<SessionRecord> = Vec::new();
    let mut chunk_start = first.timestamp;

    for record in records {
        let elapsed = record.timestamp - chunk_start;
        if elapsed >= CHUNK_DURATION_MS && !current_chunk.is_empty() {
            chunks.push(current_chunk);
            current_chunk = vec![record.clone()];
            chunk_start = record.timestamp;
        } else {
            current_chunk.push(record.clone());
        }
    }

    if !current_chunk.is_empty() {
        chunks.push(current_chunk);
    }

    chunks
}

// Swift: TranscriptCleanupEngine.swift > TranscriptCleanupEngine.processChunk(_:client:apiKey:model:baseURL:)
/// Formats a chunk of records into a prompt for the LLM.
pub fn format_chunk_prompt(records: &[SessionRecord]) -> String {
    records
        .iter()
        .map(|r| {
            let label = r.speaker.display_label();
            let text = r.refined_text.as_deref().unwrap_or(&r.text);
            let ts = chrono::DateTime::from_timestamp_millis(r.timestamp)
                .map(|dt| dt.format("%H:%M:%S").to_string())
                .unwrap_or_else(|| "??:??:??".to_string());
            format!("[{}] {}: {}", ts, label, text)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// Swift: TranscriptCleanupEngine.swift > TranscriptCleanupEngine.parseResponse(_:originalRecords:)
/// Parses the LLM response back into session records, stripping the
/// `[HH:MM:SS] Speaker: ` prefix from each line.
/// Returns None if the line count doesn't match (fall back to originals).
pub fn parse_response(
    response: &str,
    original_records: &[SessionRecord],
) -> Option<Vec<SessionRecord>> {
    let response_lines: Vec<&str> = response
        .split('\n')
        .filter(|line| !line.trim().is_empty())
        .collect();

    if response_lines.len() != original_records.len() {
        return None;
    }

    let prefix_re = Regex::new(r"^\[\d{2}:\d{2}:\d{2}\]\s+\w+:\s*").unwrap();

    let mut updated: Vec<SessionRecord> = Vec::with_capacity(original_records.len());

    for (line, original) in response_lines.iter().zip(original_records.iter()) {
        let cleaned_text = if prefix_re.is_match(line) {
            prefix_re.replace(line, "").to_string()
        } else {
            line.trim().to_string()
        };

        let refined = if cleaned_text.is_empty() {
            None // preserve original text
        } else {
            Some(cleaned_text)
        };

        updated.push(original.with_refined_text(refined));
    }

    Some(updated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::utterance::Speaker;

    fn make_record(text: &str, speaker: Speaker, timestamp: i64) -> SessionRecord {
        SessionRecord::new(speaker, text.to_string(), timestamp)
    }

    // -- chunk_records tests --

    #[test]
    fn chunk_records_empty() {
        assert!(chunk_records(&[]).is_empty());
    }

    #[test]
    fn chunk_records_single_chunk() {
        // All within 150 seconds (150_000 ms)
        let records = vec![
            make_record("hello", Speaker::You, 0),
            make_record("hi", Speaker::Them, 10_000),
            make_record("how are you", Speaker::You, 60_000),
        ];
        let chunks = chunk_records(&records);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].len(), 3);
    }

    #[test]
    fn chunk_records_splits_at_150_seconds() {
        let records = vec![
            make_record("first", Speaker::You, 0),
            make_record("second", Speaker::Them, 100_000),      // 100s
            make_record("third", Speaker::You, 160_000),        // 160s → new chunk
            make_record("fourth", Speaker::Them, 200_000),      // 200s
        ];
        let chunks = chunk_records(&records);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].len(), 2); // first, second
        assert_eq!(chunks[1].len(), 2); // third, fourth
    }

    #[test]
    fn chunk_records_three_chunks() {
        let records = vec![
            make_record("a", Speaker::You, 0),
            make_record("b", Speaker::Them, 160_000),    // 160s → chunk 2
            make_record("c", Speaker::You, 320_000),     // 320s → chunk 3
        ];
        let chunks = chunk_records(&records);
        assert_eq!(chunks.len(), 3);
    }

    // -- parse_response tests --

    #[test]
    fn parse_response_matching_line_count() {
        let records = vec![
            make_record("um hello there", Speaker::You, 1000),
            make_record("uh hi how are you", Speaker::Them, 2000),
        ];
        let response = "[00:00:01] You: Hello there.\n[00:00:02] Them: Hi, how are you?";

        let result = parse_response(response, &records).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].refined_text.as_deref(), Some("Hello there."));
        assert_eq!(result[1].refined_text.as_deref(), Some("Hi, how are you?"));
        // Original text preserved
        assert_eq!(result[0].text, "um hello there");
    }

    #[test]
    fn parse_response_mismatched_count_returns_none() {
        let records = vec![
            make_record("hello", Speaker::You, 1000),
            make_record("world", Speaker::Them, 2000),
        ];
        let response = "[00:00:01] You: Hello.\n[00:00:02] Them: World.\nExtra line";

        assert!(parse_response(response, &records).is_none());
    }

    #[test]
    fn parse_response_without_prefix_strips_whitespace() {
        let records = vec![
            make_record("hello", Speaker::You, 1000),
        ];
        // No prefix — just trimmed text
        let response = "  Hello there.  ";

        let result = parse_response(response, &records).unwrap();
        assert_eq!(result[0].refined_text.as_deref(), Some("Hello there."));
    }

    #[test]
    fn parse_response_empty_line_preserves_original() {
        let records = vec![
            make_record("keep me", Speaker::You, 1000),
        ];
        let response = "[00:00:01] You: ";

        let result = parse_response(response, &records).unwrap();
        // Empty cleaned text → refined_text is None (preserve original)
        assert!(result[0].refined_text.is_none());
    }
}
