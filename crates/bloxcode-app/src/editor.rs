//! Minimal, allocation-light Luau highlighter. Greyscale by design: structure
//! is communicated with luminance, not hue.

use eframe::egui::{self, text::LayoutJob, Color32, FontId, TextFormat};

use crate::theme::{
    CODE_COMMENT, CODE_FUNCTION, CODE_KEYWORD, CODE_NUMBER, CODE_PUNCT, CODE_STRING, CODE_TEXT,
};

const KEYWORDS: &[&str] = &[
    "and", "break", "continue", "do", "else", "elseif", "end", "false", "for", "function", "if",
    "in", "local", "nil", "not", "or", "repeat", "return", "then", "true", "until", "while",
    "export", "type", "typeof", "self", "task",
];

pub fn highlight(text: &str, font: FontId) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = f32::INFINITY;
    for token in tokenize(text) {
        job.append(
            token.text,
            0.0,
            TextFormat {
                font_id: font.clone(),
                color: token.color,
                ..Default::default()
            },
        );
    }
    job
}

struct Token<'a> {
    text: &'a str,
    color: Color32,
}

fn tokenize(text: &str) -> Vec<Token<'_>> {
    let bytes = text.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0usize;

    while index < text.len() {
        let start = index;
        let ch = text[index..]
            .chars()
            .next()
            .expect("index is a char boundary");

        if ch == '-' && bytes.get(index + 1) == Some(&b'-') {
            let after = &text[index + 2..];
            if after.starts_with("[[") || after.starts_with("[=") {
                if let Some(end) = after.find("]]") {
                    index = index + 2 + end + 2;
                } else {
                    index = text.len();
                }
            } else {
                while index < text.len() && bytes[index] != b'\n' {
                    index += 1;
                }
            }
            tokens.push(Token {
                text: &text[start..index],
                color: CODE_COMMENT,
            });
        } else if ch == '"' || ch == '\'' {
            index += ch.len_utf8();
            while index < text.len() {
                let current = text[index..].chars().next().unwrap();
                if current == '\\' {
                    index += 1;
                    if let Some(escaped) = text[index..].chars().next() {
                        index += escaped.len_utf8();
                    }
                    continue;
                }
                index += current.len_utf8();
                if current == ch || current == '\n' {
                    break;
                }
            }
            tokens.push(Token {
                text: &text[start..index],
                color: CODE_STRING,
            });
        } else if ch == '['
            && (bytes.get(index + 1) == Some(&b'[') || bytes.get(index + 1) == Some(&b'='))
        {
            if let Some(end) = text[index..].find("]]") {
                index = index + end + 2;
            } else {
                index = text.len();
            }
            tokens.push(Token {
                text: &text[start..index],
                color: CODE_STRING,
            });
        } else if ch.is_ascii_digit() {
            while index < text.len() {
                let byte = bytes[index];
                if byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'_' {
                    index += 1;
                } else {
                    break;
                }
            }
            tokens.push(Token {
                text: &text[start..index],
                color: CODE_NUMBER,
            });
        } else if ch.is_alphabetic() || ch == '_' {
            while index < text.len() {
                let current = text[index..].chars().next().unwrap();
                if current.is_alphanumeric() || current == '_' {
                    index += current.len_utf8();
                } else {
                    break;
                }
            }
            let word = &text[start..index];
            let color = if KEYWORDS.contains(&word) {
                CODE_KEYWORD
            } else if text[index..]
                .chars()
                .find(|c| !c.is_whitespace())
                .map(|next| next == '(' || next == '"' || next == '\'')
                .unwrap_or(false)
            {
                CODE_FUNCTION
            } else {
                CODE_TEXT
            };
            tokens.push(Token { text: word, color });
        } else if ch.is_whitespace() {
            while index < text.len() {
                let current = text[index..].chars().next().unwrap();
                if current.is_whitespace() {
                    index += current.len_utf8();
                } else {
                    break;
                }
            }
            tokens.push(Token {
                text: &text[start..index],
                color: CODE_TEXT,
            });
        } else {
            index += ch.len_utf8();
            let color = if "()[]{}.,;:+-*/%^#=<>~".contains(ch) {
                CODE_PUNCT
            } else {
                CODE_TEXT
            };
            tokens.push(Token {
                text: &text[start..index],
                color,
            });
        }
    }

    tokens
}

/// Seven-pixel-wide digit column plus breathing room.
pub fn gutter_width(line_count: usize) -> f32 {
    let digits = line_count.max(9).to_string().len() as f32;
    22.0 + digits * 7.0
}

pub fn row_height(ui: &egui::Ui, font: &FontId) -> f32 {
    ui.fonts(|fonts| fonts.row_height(font))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlighting_preserves_full_text() {
        let source = "local x = 1 -- comment\nprint(\"hi\")\n";
        let tokens = tokenize(source);
        let joined: String = tokens.iter().map(|token| token.text).collect();
        assert_eq!(joined, source);
    }

    #[test]
    fn comments_are_detected() {
        let tokens = tokenize("-- hello\nlocal x = 1");
        assert_eq!(tokens[0].color, CODE_COMMENT);
    }

    #[test]
    fn block_comments_are_single_token() {
        let tokens = tokenize("--[[ multi\nline ]]\nlocal x = 1");
        assert_eq!(tokens[0].color, CODE_COMMENT);
        assert!(tokens[0].text.contains("multi"));
    }

    #[test]
    fn multiline_strings_survive_tokenizing() {
        let source = "local s = [[ a\nb ]]\n";
        let tokens = tokenize(source);
        let joined: String = tokens.iter().map(|token| token.text).collect();
        assert_eq!(joined, source);
    }
}
