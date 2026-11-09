use anyhow::{anyhow, Result};
use serde::de::DeserializeOwned;

/// Parse JSON out of a model response that may be wrapped in prose or code
/// fences. Tries the raw string, every fenced block, then the first balanced
/// JSON object/array found in the text.
pub fn parse_json<T: DeserializeOwned>(raw: &str) -> Result<T> {
    let candidates = candidates(raw);
    let mut last_error = None;
    for candidate in &candidates {
        match serde_json::from_str::<T>(candidate) {
            Ok(value) => return Ok(value),
            Err(error) => last_error = Some(error),
        }
    }
    Err(anyhow!(
        "model response was not valid JSON ({}); first 400 characters: {}",
        last_error
            .map(|e| e.to_string())
            .unwrap_or_else(|| "no JSON value found".into()),
        crate::util::truncate(raw.trim(), 400)
    ))
}

fn candidates(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return out;
    }
    out.push(trimmed.to_string());

    for block in fenced_blocks(trimmed) {
        out.push(block);
    }

    if let Some(start) = trimmed.find(['{', '[']) {
        if let Some(end) = balanced_end(trimmed, start) {
            out.push(trimmed[start..=end].to_string());
        }
    }
    out
}

fn fenced_blocks(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find("```") {
        let after = &rest[open + 3..];
        let Some(newline) = after.find('\n') else {
            break;
        };
        let body = &after[newline + 1..];
        let Some(close) = body.find("```") else { break };
        let block = body[..close].trim();
        if !block.is_empty() {
            out.push(block.to_string());
        }
        rest = &body[close + 3..];
    }
    out
}

fn balanced_end(text: &str, start: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    for (index, byte) in bytes.iter().enumerate().skip(start) {
        if in_string {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' | b'[' => depth += 1,
            b'}' | b']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Payload {
        name: String,
        count: u32,
    }

    #[test]
    fn parses_plain_json() {
        let value: Payload = parse_json(r#"{"name":"ok","count":2}"#).unwrap();
        assert_eq!(value.name, "ok");
        assert_eq!(value.count, 2);
    }

    #[test]
    fn parses_fenced_json_with_prose() {
        let raw = "Here is the blueprint:\n```json\n{\"name\":\"game\",\"count\":1}\n```\nEnjoy.";
        let value: Payload = parse_json(raw).unwrap();
        assert_eq!(value.name, "game");
    }

    #[test]
    fn parses_object_embedded_in_prose() {
        let raw = "Sure. {\"name\":\"a { tricky } name\",\"count\":3} trailing text";
        let value: Payload = parse_json(raw).unwrap();
        assert_eq!(value.name, "a { tricky } name");
        assert_eq!(value.count, 3);
    }

    #[test]
    fn reports_invalid_json() {
        assert!(parse_json::<Payload>("no json here").is_err());
    }

    #[test]
    fn handles_braces_inside_strings() {
        let raw = r#"{"name":"has } brace","count":4}"#;
        let value: Payload = parse_json(raw).unwrap();
        assert_eq!(value.name, "has } brace");
    }
}
