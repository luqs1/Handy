//! Embedding-based knowledge base: markdown chunking, cosine similarity search, SHA-256 cache.
//! Translated from: OpenOats/Sources/OpenOats/Intelligence/KnowledgeBase.swift

use crate::domain::models::KBResult;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

// MARK: - Types

/// A chunk of text from a knowledge base document, with its embedding vector.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KBChunk {
    pub text: String,
    pub source_file: String,
    pub header_context: String,
    pub embedding: Vec<f32>,
}

/// Disk cache format for embedded KB chunks.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct KBCache {
    /// Keyed by "filename:sha256hash"
    entries: HashMap<String, Vec<KBChunk>>,
    /// Fingerprint of the embedding config used to produce these vectors.
    embedding_config_fingerprint: Option<String>,
}

// MARK: - Cosine Similarity

// Swift: KnowledgeBase.swift > KnowledgeBase.cosineSimilarity(_:_:)
/// Cosine similarity between two vectors. Returns 0 for empty or mismatched lengths.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let mut dot: f32 = 0.0;
    let mut mag_a: f32 = 0.0;
    let mut mag_b: f32 = 0.0;

    for i in 0..a.len() {
        dot += a[i] * b[i];
        mag_a += a[i] * a[i];
        mag_b += b[i] * b[i];
    }

    let denom = mag_a.sqrt() * mag_b.sqrt();
    if denom == 0.0 {
        return 0.0;
    }
    dot / denom
}

// MARK: - SHA-256

// Swift: KnowledgeBase.swift > KnowledgeBase.sha256(_:)
pub fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

// MARK: - Markdown Chunking

/// A parsed section from markdown content.
struct Section {
    headers: Vec<String>, // hierarchy stack
    lines: Vec<String>,
}

// Swift: KnowledgeBase.swift > KnowledgeBase.chunkMarkdown(_:sourceFile:)
/// Splits markdown content into chunks aware of header hierarchy.
/// Merges small sections (< 80 words) and splits large ones (> 500 words) with overlap.
pub fn chunk_markdown(text: &str, source_file: &str) -> Vec<(String, String)> {
    let lines: Vec<&str> = text.lines().collect();

    let mut sections: Vec<Section> = Vec::new();
    let mut current_headers: Vec<String> = Vec::new();
    let mut current_lines: Vec<String> = Vec::new();

    for line in &lines {
        if line.starts_with('#') {
            // Flush current section
            if !current_lines.is_empty() {
                sections.push(Section {
                    headers: current_headers.clone(),
                    lines: current_lines,
                });
                current_lines = Vec::new();
            }

            // Parse header level
            let level = line.chars().take_while(|&c| c == '#').count();
            let header_text = line[level..].trim().to_string();

            // Build header stack: keep headers at higher levels, replace at current
            if level <= current_headers.len() {
                current_headers.truncate(level - 1);
            }
            current_headers.push(header_text);
        } else {
            current_lines.push(line.to_string());
        }
    }
    if !current_lines.is_empty() {
        sections.push(Section {
            headers: current_headers,
            lines: current_lines,
        });
    }

    // Merge small sections and split large ones
    let target_min = 80;
    let target_max = 500;

    let mut result: Vec<(String, String)> = Vec::new();
    let mut pending_text = String::new();
    let mut pending_header = String::new();

    for section in &sections {
        let section_text = section.lines.join("\n").trim().to_string();
        if section_text.is_empty() {
            continue;
        }

        let breadcrumb = section.headers.join(" > ");
        let word_count = section_text.split_whitespace().count();

        if word_count < target_min {
            // Merge with pending
            if pending_text.is_empty() {
                pending_text = section_text;
                pending_header = breadcrumb;
            } else {
                pending_text.push_str("\n\n");
                pending_text.push_str(&section_text);
                if !breadcrumb.is_empty() {
                    pending_header = breadcrumb;
                }
            }

            // Flush if pending is now large enough
            let pending_words = pending_text.split_whitespace().count();
            if pending_words >= target_min {
                result.push((pending_text.clone(), pending_header.clone()));
                pending_text.clear();
                pending_header.clear();
            }
        } else if word_count > target_max {
            // Flush pending first
            if !pending_text.is_empty() {
                result.push((pending_text.clone(), pending_header.clone()));
                pending_text.clear();
                pending_header.clear();
            }

            // Split large section with overlap
            let words: Vec<&str> = section_text.split_whitespace().collect();
            let overlap = target_max / 5;
            let mut start = 0;
            while start < words.len() {
                let end = (start + target_max).min(words.len());
                let chunk = words[start..end].join(" ");
                result.push((chunk, breadcrumb.clone()));
                start += target_max - overlap;
            }
        } else {
            // Flush pending first
            if !pending_text.is_empty() {
                result.push((pending_text.clone(), pending_header.clone()));
                pending_text.clear();
                pending_header.clear();
            }
            result.push((section_text, breadcrumb));
        }
    }

    // Flush remaining
    if !pending_text.is_empty() {
        result.push((pending_text, pending_header));
    }

    // If no chunks were produced, chunk the whole text
    if result.is_empty() {
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            let words: Vec<&str> = trimmed.split_whitespace().collect();
            if words.len() <= target_max {
                result.push((trimmed.to_string(), String::new()));
            } else {
                let overlap = target_max / 5;
                let mut start = 0;
                while start < words.len() {
                    let end = (start + target_max).min(words.len());
                    let chunk = words[start..end].join(" ");
                    result.push((chunk, String::new()));
                    start += target_max - overlap;
                }
            }
        }
    }

    result
}

// MARK: - Search (with pre-loaded chunks)

// Swift: KnowledgeBase.swift > KnowledgeBase.search(queries:topK:)
/// Search pre-loaded chunks using cosine similarity. Multi-query with max-score fusion.
pub fn search_chunks(
    chunks: &[KBChunk],
    query_embeddings: &[Vec<f32>],
    top_k: usize,
) -> Vec<KBResult> {
    if chunks.is_empty() || query_embeddings.is_empty() {
        return Vec::new();
    }

    // Score fusion: for each chunk, take max cosine similarity across all queries
    let mut best_scores: HashMap<usize, f32> = HashMap::new();

    for query_emb in query_embeddings {
        for (i, chunk) in chunks.iter().enumerate() {
            let sim = cosine_similarity(query_emb, &chunk.embedding);
            if sim > 0.1 {
                let entry = best_scores.entry(i).or_insert(0.0);
                *entry = entry.max(sim);
            }
        }
    }

    let mut scored: Vec<(usize, f32)> = best_scores.into_iter().collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    scored
        .into_iter()
        .take(top_k)
        .map(|(idx, score)| {
            let chunk = &chunks[idx];
            KBResult::new(
                chunk.text.clone(),
                chunk.source_file.clone(),
                chunk.header_context.clone(),
                score as f64,
            )
        })
        .collect()
}

// MARK: - Cache I/O

// Swift: KnowledgeBase.swift > KnowledgeBase.loadCache()
pub fn load_cache(cache_path: &PathBuf) -> KBCache {
    match fs::read_to_string(cache_path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or(KBCache {
            entries: HashMap::new(),
            embedding_config_fingerprint: None,
        }),
        Err(_) => KBCache {
            entries: HashMap::new(),
            embedding_config_fingerprint: None,
        },
    }
}

// Swift: KnowledgeBase.swift > KnowledgeBase.saveCache(_:)
pub fn save_cache(cache: &KBCache, cache_path: &PathBuf) {
    if let Ok(json) = serde_json::to_string(cache) {
        let _ = fs::write(cache_path, json);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- cosine_similarity tests --

    #[test]
    fn cosine_similarity_identical() {
        let v = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&v, &v);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_opposite() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_empty() {
        assert_eq!(cosine_similarity(&[], &[]), 0.0);
    }

    #[test]
    fn cosine_similarity_mismatched_length() {
        assert_eq!(cosine_similarity(&[1.0], &[1.0, 2.0]), 0.0);
    }

    // -- sha256 tests --

    #[test]
    fn sha256_consistency() {
        let hash1 = sha256_hex("hello world");
        let hash2 = sha256_hex("hello world");
        assert_eq!(hash1, hash2);
        assert_ne!(hash1, sha256_hex("hello world!"));
    }

    // -- chunk_markdown tests --

    #[test]
    fn chunk_markdown_single_section_no_headers() {
        let text = "Just some plain text without any headers. ".repeat(20); // ~100 words
        let chunks = chunk_markdown(&text, "test.md");
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].1.is_empty()); // no header context
    }

    #[test]
    fn chunk_markdown_multiple_headers() {
        // Each section has ~100 words (above min threshold of 80)
        let word_block = "word ".repeat(100);
        let text = format!(
            "# Section One\n{}\n# Section Two\n{}\n# Section Three\n{}",
            word_block, word_block, word_block
        );
        let chunks = chunk_markdown(&text, "test.md");
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].1, "Section One");
        assert_eq!(chunks[1].1, "Section Two");
        assert_eq!(chunks[2].1, "Section Three");
    }

    #[test]
    fn chunk_markdown_merges_small_sections() {
        // Each section is way below 80 words → should merge
        let text = "# A\nShort text.\n# B\nAlso short.";
        let chunks = chunk_markdown(&text, "test.md");
        // Both sections are small so they get merged into one
        assert_eq!(chunks.len(), 1);
    }

    #[test]
    fn chunk_markdown_splits_large_sections() {
        // One giant section > 500 words
        let big_text = format!("# Big Section\n{}", "word ".repeat(600));
        let chunks = chunk_markdown(&big_text, "test.md");
        assert!(chunks.len() >= 2); // should be split
        assert_eq!(chunks[0].1, "Big Section");
    }

    #[test]
    fn chunk_markdown_header_hierarchy() {
        let word_block = "word ".repeat(100);
        let text = format!(
            "# Top\n## Nested\n{}\n## Another\n{}",
            word_block, word_block
        );
        let chunks = chunk_markdown(&text, "test.md");
        // Should have breadcrumb like "Top > Nested"
        assert!(chunks.iter().any(|(_, h)| h.contains(" > ")));
    }

    // -- search_chunks tests --

    #[test]
    fn search_returns_top_k() {
        let chunks = vec![
            KBChunk {
                text: "about rust".to_string(),
                source_file: "a.md".to_string(),
                header_context: "".to_string(),
                embedding: vec![1.0, 0.0, 0.0],
            },
            KBChunk {
                text: "about python".to_string(),
                source_file: "b.md".to_string(),
                header_context: "".to_string(),
                embedding: vec![0.0, 1.0, 0.0],
            },
            KBChunk {
                text: "about rust and python".to_string(),
                source_file: "c.md".to_string(),
                header_context: "".to_string(),
                embedding: vec![0.7, 0.7, 0.0],
            },
        ];

        // Query vector closest to "about rust"
        let query = vec![vec![0.9, 0.1, 0.0]];
        let results = search_chunks(&chunks, &query, 2);
        assert_eq!(results.len(), 2);
        // First result should be "about rust" (highest cosine with query)
        assert_eq!(results[0].text, "about rust");
    }

    #[test]
    fn search_multi_query_fusion() {
        let chunks = vec![
            KBChunk {
                text: "topic A".to_string(),
                source_file: "a.md".to_string(),
                header_context: "".to_string(),
                embedding: vec![1.0, 0.0],
            },
            KBChunk {
                text: "topic B".to_string(),
                source_file: "b.md".to_string(),
                header_context: "".to_string(),
                embedding: vec![0.0, 1.0],
            },
        ];

        // Two queries: one close to A, one close to B
        let queries = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
        let results = search_chunks(&chunks, &queries, 2);
        // Both chunks should appear since each matches one query
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn search_empty_chunks() {
        let results = search_chunks(&[], &[vec![1.0, 0.0]], 5);
        assert!(results.is_empty());
    }

    // -- cache tests --

    #[test]
    fn cache_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kb_cache.json");

        let mut cache = KBCache {
            entries: HashMap::new(),
            embedding_config_fingerprint: Some("test-fp".to_string()),
        };
        cache.entries.insert(
            "file.md:abc123".to_string(),
            vec![KBChunk {
                text: "hello".to_string(),
                source_file: "file.md".to_string(),
                header_context: "".to_string(),
                embedding: vec![1.0, 2.0, 3.0],
            }],
        );

        save_cache(&cache, &path);
        let loaded = load_cache(&path);
        assert_eq!(
            loaded.embedding_config_fingerprint,
            Some("test-fp".to_string())
        );
        assert_eq!(loaded.entries.len(), 1);
        assert_eq!(loaded.entries["file.md:abc123"][0].text, "hello");
    }
}
