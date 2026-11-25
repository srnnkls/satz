//! The BERT `WordPiece` tokenizer of `tokenizer.json`, as aikit implements it:
//! added tokens carved from the raw text, then `BertNormalizer`,
//! `BertPreTokenizer` and greedy longest-match `WordPiece`.

use std::collections::{BTreeMap, HashMap};

use serde::Deserialize;
use unicode_normalization::UnicodeNormalization;
use unicode_properties::{GeneralCategory, GeneralCategoryGroup, UnicodeGeneralCategory};

use super::{Reason, parse};
use crate::similarity::lowercase;

const DEFAULT_PREFIX: &str = "##";
const DEFAULT_MAX_CHARS: usize = 100;

pub(super) struct WordPiece {
    vocab: HashMap<String, u32>,
    added: Vec<(String, u32)>,
    unknown: u32,
    prefix: String,
    max_chars: usize,
    normalize: Vec<Normalize>,
}

/// One `BertNormalizer` step.
#[derive(Clone, Copy)]
enum Normalize {
    CleanText,
    SpaceCjk,
    StripAccents,
    Lowercase,
}

impl Normalize {
    fn apply(self, text: &str) -> String {
        match self {
            Self::CleanText => text
                .chars()
                .filter(|&c| c != '\0' && c != char::REPLACEMENT_CHARACTER && !is_control(c))
                .map(|c| if c.is_whitespace() { ' ' } else { c })
                .collect(),
            Self::SpaceCjk => {
                let mut spaced = String::with_capacity(text.len());
                for c in text.chars() {
                    if is_cjk(c) {
                        spaced.extend([' ', c, ' ']);
                    } else {
                        spaced.push(c);
                    }
                }
                spaced
            }
            Self::StripAccents => text
                .nfd()
                .filter(|&c| c.general_category() != GeneralCategory::NonspacingMark)
                .collect(),
            Self::Lowercase => lowercase(text),
        }
    }
}

#[derive(Deserialize)]
struct File {
    #[serde(default)]
    added_tokens: Vec<AddedToken>,
    normalizer: Normalizer,
    pre_tokenizer: PreTokenizer,
    model: Model,
}

#[derive(Deserialize)]
struct AddedToken {
    id: u32,
    content: String,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum Normalizer {
    BertNormalizer {
        #[serde(default)]
        clean_text: bool,
        #[serde(default)]
        handle_chinese_chars: bool,
        strip_accents: Option<bool>,
        #[serde(default)]
        lowercase: bool,
    },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum PreTokenizer {
    BertPreTokenizer,
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum Model {
    WordPiece {
        #[serde(default)]
        unk_token: String,
        #[serde(default)]
        continuing_subword_prefix: String,
        #[serde(default)]
        max_input_chars_per_word: i64,
        vocab: HashMap<String, u32>,
    },
    #[serde(other)]
    Other,
}

impl WordPiece {
    pub(super) fn from_json(bytes: &[u8]) -> Result<Self, Reason> {
        let file: File = parse(bytes, "tokenizer.json")?;
        let (
            Normalizer::BertNormalizer {
                clean_text,
                handle_chinese_chars,
                strip_accents,
                lowercase,
            },
            PreTokenizer::BertPreTokenizer,
            Model::WordPiece {
                unk_token,
                continuing_subword_prefix,
                max_input_chars_per_word,
                vocab,
            },
        ) = (file.normalizer, file.pre_tokenizer, file.model)
        else {
            return Err(Reason::Tokenizer);
        };
        let added: BTreeMap<String, u32> = file
            .added_tokens
            .into_iter()
            .map(|token| (token.content, token.id))
            .collect();
        let unknown = vocab
            .get(&unk_token)
            .or_else(|| added.get(&unk_token))
            .copied()
            .ok_or(Reason::UnknownToken(unk_token))?;
        let mut added: Vec<_> = added.into_iter().collect();
        added.sort_by_key(|(content, _)| std::cmp::Reverse(content.len()));
        Ok(Self {
            vocab,
            added,
            unknown,
            prefix: if continuing_subword_prefix.is_empty() {
                DEFAULT_PREFIX.to_owned()
            } else {
                continuing_subword_prefix
            },
            max_chars: usize::try_from(max_input_chars_per_word)
                .ok()
                .filter(|&max| max > 0)
                .unwrap_or(DEFAULT_MAX_CHARS),
            normalize: [
                (clean_text, Normalize::CleanText),
                (handle_chinese_chars, Normalize::SpaceCjk),
                (strip_accents.unwrap_or(lowercase), Normalize::StripAccents),
                (lowercase, Normalize::Lowercase),
            ]
            .into_iter()
            .filter_map(|(enabled, step)| enabled.then_some(step))
            .collect(),
        })
    }

    /// The token ids of `text`, without `[CLS]` and `[SEP]`.
    pub(super) fn encode(&self, text: &str) -> Vec<u32> {
        let mut ids = Vec::new();
        let mut start = 0;
        for (i, _) in text.char_indices() {
            if i < start {
                continue;
            }
            if let Some((content, id)) = self.added_at(&text[i..]) {
                self.encode_segment(&text[start..i], &mut ids);
                ids.push(id);
                start = i + content.len();
            }
        }
        self.encode_segment(&text[start..], &mut ids);
        ids
    }

    fn added_at(&self, text: &str) -> Option<(&str, u32)> {
        self.added
            .iter()
            .find(|(content, _)| text.starts_with(content.as_str()))
            .map(|(content, id)| (content.as_str(), *id))
    }

    fn encode_segment(&self, text: &str, ids: &mut Vec<u32>) {
        let normalized = self
            .normalize
            .iter()
            .fold(text.to_owned(), |text, step| step.apply(&text));
        for word in pre_tokenize(&normalized) {
            match self.added.iter().find(|(content, _)| content == word) {
                Some(&(_, id)) => ids.push(id),
                None => self.encode_word(word, ids),
            }
        }
    }

    fn encode_word(&self, word: &str, ids: &mut Vec<u32>) {
        if word.chars().count() > self.max_chars {
            ids.push(self.unknown);
            return;
        }
        let mut pieces = Vec::new();
        let mut candidate = String::new();
        let mut start = 0;
        while start < word.len() {
            let rest = &word[start..];
            let longest = rest
                .char_indices()
                .map(|(i, c)| i + c.len_utf8())
                .rev()
                .find_map(|end| {
                    candidate.clear();
                    if start > 0 {
                        candidate.push_str(&self.prefix);
                    }
                    candidate.push_str(&rest[..end]);
                    self.vocab.get(&candidate).map(|&id| (end, id))
                });
            let Some((end, id)) = longest else {
                ids.push(self.unknown);
                return;
            };
            pieces.push(id);
            start += end;
        }
        ids.extend(pieces);
    }
}

/// Splits on whitespace, then splits each punctuation char into its own word.
fn pre_tokenize(text: &str) -> Vec<&str> {
    let mut words = Vec::new();
    for part in text.split_whitespace() {
        let mut start = 0;
        for (i, c) in part.char_indices().filter(|&(_, c)| is_punctuation(c)) {
            let end = i + c.len_utf8();
            words.extend(
                [&part[start..i], &part[i..end]]
                    .into_iter()
                    .filter(|word| !word.is_empty()),
            );
            start = end;
        }
        if start < part.len() {
            words.push(&part[start..]);
        }
    }
    words
}

fn is_control(c: char) -> bool {
    !matches!(c, '\t' | '\n' | '\r')
        && matches!(
            c.general_category(),
            GeneralCategory::Control | GeneralCategory::Format
        )
}

fn is_punctuation(c: char) -> bool {
    c.is_ascii_punctuation() || c.general_category_group() == GeneralCategoryGroup::Punctuation
}

fn is_cjk(c: char) -> bool {
    matches!(
        c,
        '\u{4E00}'..='\u{9FFF}'
            | '\u{3400}'..='\u{4DBF}'
            | '\u{20000}'..='\u{2A6DF}'
            | '\u{2A700}'..='\u{2B73F}'
            | '\u{2B740}'..='\u{2B81F}'
            | '\u{2B820}'..='\u{2CEAF}'
            | '\u{F900}'..='\u{FAFF}'
            | '\u{2F800}'..='\u{2FA1F}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokenizer() -> WordPiece {
        WordPiece::from_json(
            r###"{
                "added_tokens": [
                    {"id": 0, "content": "[UNK]"},
                    {"id": 1, "content": "[CLS]"}
                ],
                "normalizer": {
                    "type": "BertNormalizer",
                    "clean_text": true,
                    "handle_chinese_chars": true,
                    "strip_accents": null,
                    "lowercase": true
                },
                "pre_tokenizer": {"type": "BertPreTokenizer"},
                "model": {
                    "type": "WordPiece",
                    "unk_token": "[UNK]",
                    "continuing_subword_prefix": "##",
                    "max_input_chars_per_word": 8,
                    "vocab": {
                        "[UNK]": 0, "[CLS]": 1, "cafe": 2, "run": 3, "##s": 4,
                        ",": 5, "字": 6, "a": 7, "##a": 8
                    }
                }
            }"###
                .as_bytes(),
        )
        .unwrap()
    }

    #[test]
    fn splits_punctuation_and_continues_words() {
        assert_eq!(tokenizer().encode("Runs,\trun"), [3, 4, 5, 3]);
    }

    #[test]
    fn strips_accents_and_spaces_cjk() {
        assert_eq!(tokenizer().encode("Café字cafe"), [2, 6, 2]);
    }

    #[test]
    fn carves_added_tokens_from_the_raw_text() {
        assert_eq!(tokenizer().encode("run[CLS]runs"), [3, 1, 3, 4]);
    }

    #[test]
    fn unknown_and_overlong_words_are_one_unknown_token() {
        assert_eq!(
            tokenizer().encode("runx aaaaaaaaa aaaaaaaa"),
            [0, 0, 7, 8, 8, 8, 8, 8, 8, 8]
        );
    }

    #[test]
    fn rejects_other_tokenizers() {
        let json = br#"{
            "normalizer": {"type": "NFKC"},
            "pre_tokenizer": {"type": "Metaspace"},
            "model": {"type": "Unigram", "vocab": [["a", 0.0]]}
        }"#;
        assert!(matches!(WordPiece::from_json(json), Err(Reason::Tokenizer)));
    }
}
