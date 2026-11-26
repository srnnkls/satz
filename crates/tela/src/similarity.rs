//! Text similarity: word shingles with Jaccard and containment, and cosine
//! over embeddings from a caller's model.

use std::collections::{BTreeSet, btree_set};
use std::error::Error;
use std::fmt;
use std::iter::FusedIterator;

use unicode_properties::{GeneralCategoryGroup, UnicodeGeneralCategory};

#[cfg(feature = "semantic")]
mod model2vec;

#[cfg(feature = "semantic")]
pub use model2vec::{LoadError, Model2Vec};

/// A semantic model that embeds texts as vectors.
pub trait Embed: Send + Sync + fmt::Debug {
    /// The model's name, reported with every cosine score.
    fn model(&self) -> &str;

    /// The raw embedding of `text`.
    ///
    /// # Errors
    /// Returns [`EmbedError`] when the model cannot embed the text.
    fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError>;
}

/// A model that failed to embed a text.
#[derive(Debug, thiserror::Error)]
#[error("model {model:?} failed to embed a text")]
pub struct EmbedError {
    model: String,
    #[source]
    source: Box<dyn Error + Send + Sync>,
}

impl EmbedError {
    /// The failure of `model`, caused by the model's own `source` error.
    pub fn new(model: impl Into<String>, source: impl Into<Box<dyn Error + Send + Sync>>) -> Self {
        Self {
            model: model.into(),
            source: source.into(),
        }
    }

    /// The model.
    #[must_use]
    pub fn model(&self) -> &str {
        &self.model
    }
}

/// The lower-cased words of `text`: runs of letters, numbers and marks.
#[must_use]
pub fn words(text: &str) -> Vec<String> {
    lowercase(text)
        .split(|c: char| {
            !matches!(
                c.general_category_group(),
                GeneralCategoryGroup::Letter
                    | GeneralCategoryGroup::Number
                    | GeneralCategoryGroup::Mark
            )
        })
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Lower-cases each char by its simple case mapping, as Go's `strings.ToLower`
/// does: `İ` becomes `i` and a final `Σ` becomes `σ`.
pub(crate) fn lowercase(text: &str) -> String {
    text.chars()
        .map(|c| c.to_lowercase().next().unwrap_or(c))
        .collect()
}

/// The distinct runs of `size` consecutive words of a text.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Shingles(BTreeSet<String>);

impl Shingles {
    /// The shingle size of `near`, `overlap` and shared phrases.
    pub const DEFAULT_SIZE: usize = 3;

    /// The shingles of `size` words in `text`; none when `size` is 0.
    #[must_use]
    pub fn of(text: &str, size: usize) -> Self {
        if size == 0 {
            return Self::default();
        }
        Self(words(text).windows(size).map(|run| run.join(" ")).collect())
    }

    /// The number of shingles.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the text had fewer words than the shingle size.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The shingles in sorted order.
    #[must_use]
    pub fn iter(&self) -> ShingleIter<'_> {
        ShingleIter(self.0.iter())
    }

    fn shared_count(&self, other: &Self) -> usize {
        self.0.intersection(&other.0).count()
    }

    /// Shared shingles over all distinct shingles; 0 when either side is empty.
    #[must_use]
    pub fn jaccard(&self, other: &Self) -> f64 {
        if self.is_empty() || other.is_empty() {
            return 0.0;
        }
        let shared = self.shared_count(other);
        ratio(shared, self.len() + other.len() - shared)
    }

    /// Shared shingles over the smaller side's; 0 when either side is empty.
    #[must_use]
    pub fn containment(&self, other: &Self) -> f64 {
        if self.is_empty() || other.is_empty() {
            return 0.0;
        }
        ratio(self.shared_count(other), self.len().min(other.len()))
    }

    /// Up to `limit` shared shingles, in sorted order.
    #[must_use]
    pub fn shared<'a>(&'a self, other: &'a Self, limit: usize) -> Vec<&'a str> {
        self.0
            .intersection(&other.0)
            .take(limit)
            .map(String::as_str)
            .collect()
    }
}

impl<'a> IntoIterator for &'a Shingles {
    type Item = &'a str;
    type IntoIter = ShingleIter<'a>;

    fn into_iter(self) -> ShingleIter<'a> {
        self.iter()
    }
}

/// The shingles of a [`Shingles`] in sorted order.
#[derive(Debug, Clone)]
pub struct ShingleIter<'a>(btree_set::Iter<'a, String>);

impl<'a> Iterator for ShingleIter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        self.0.next().map(String::as_str)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl DoubleEndedIterator for ShingleIter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(String::as_str)
    }
}

impl ExactSizeIterator for ShingleIter<'_> {}

impl FusedIterator for ShingleIter<'_> {}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    let as_f64 = |n: usize| f64::from(u32::try_from(n).unwrap_or(u32::MAX));
    as_f64(numerator) / as_f64(denominator)
}

/// A unit-length embedding.
#[derive(Debug, Clone, PartialEq)]
pub struct Embedding(Vec<f64>);

/// Why a raw vector does not normalize.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum EmbeddingError {
    /// A component is NaN or infinite.
    #[error("embedding has a non-finite component")]
    NonFinite,
    /// Every component is zero.
    #[error("embedding is the zero vector")]
    Zero,
}

impl Embedding {
    /// Normalizes a model's raw vector.
    ///
    /// # Errors
    /// Returns [`EmbeddingError::NonFinite`] for a NaN or infinite component
    /// and [`EmbeddingError::Zero`] for the zero vector.
    pub fn new(raw: &[f32]) -> Result<Self, EmbeddingError> {
        let mut norm = 0.0_f64;
        for &value in raw {
            let x = f64::from(value);
            if !x.is_finite() {
                return Err(EmbeddingError::NonFinite);
            }
            norm = norm.hypot(x);
        }
        if norm == 0.0 {
            return Err(EmbeddingError::Zero);
        }
        Ok(Self(raw.iter().map(|&x| f64::from(x) / norm).collect()))
    }

    /// The cosine similarity with `other`, clamped to `[-1, 1]`.
    #[must_use]
    pub fn cosine(&self, other: &Self) -> f64 {
        self.0
            .iter()
            .zip(&other.0)
            .map(|(a, b)| a * b)
            .sum::<f64>()
            .clamp(-1.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The paragraphs of the similarity parity input, `tests/parity/similarity.md`.
    pub(super) const GATE: &str =
        "The gate runs every formatter, linter and test before a commit lands on the main branch.";
    pub(super) const GATE_REWORDED: &str = "The gate runs every formatter, linter and the tests before any commit lands on the main branch today.";
    pub(super) const GATE_PARAPHRASED: &str =
        "Before a change lands on main, the gate executes all formatters, linters and tests first.";

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-12
    }

    #[test]
    fn words_split_on_punctuation_and_lowercase() {
        assert_eq!(
            words("The Gate, runs—tests!"),
            ["the", "gate", "runs", "tests"]
        );
    }

    #[test]
    fn words_keep_marks_and_split_on_symbols() {
        assert_eq!(words("cafe\u{301} x\u{24B6}y"), ["cafe\u{301}", "x", "y"]);
    }

    #[test]
    fn words_lowercase_by_simple_case_mapping() {
        assert_eq!(words("İSTANBUL ΟΔΟΣ"), ["istanbul", "οδοσ"]);
    }

    #[test]
    fn near_and_overlap_scores_match_henia() {
        let gate = Shingles::of(GATE, Shingles::DEFAULT_SIZE);
        let reworded = Shingles::of(GATE_REWORDED, Shingles::DEFAULT_SIZE);
        assert!(close(gate.jaccard(&reworded), 0.428_571_428_571_428_55));
        assert!(close(gate.containment(&reworded), 0.642_857_142_857_142_9));
        let paraphrased = Shingles::of(GATE_PARAPHRASED, Shingles::DEFAULT_SIZE);
        assert!(gate.containment(&paraphrased) < 0.1);
        assert!(reworded.containment(&paraphrased) < 0.1);
    }

    #[test]
    fn shingles_need_enough_words() {
        assert!(Shingles::of("two words", 3).is_empty());
        assert!(Shingles::of("any text", 0).is_empty());
        assert_eq!(Shingles::of("a b c d", 3).len(), 2);
    }

    #[test]
    fn jaccard_and_containment_count_shared_shingles() {
        let a = Shingles::of("a b c d e", 3);
        let b = Shingles::of("b c d e f g", 3);
        assert!(close(a.jaccard(&b), 2.0 / 5.0));
        assert!(close(a.containment(&b), 2.0 / 3.0));
        assert_eq!(a.shared(&b, 1), ["b c d"]);
        assert!(close(a.jaccard(&Shingles::default()), 0.0));
    }

    #[test]
    fn embeddings_normalize_and_reject_degenerate_vectors() {
        let a = Embedding::new(&[3.0, 4.0]).unwrap();
        let b = Embedding::new(&[4.0, 3.0]).unwrap();
        assert!(close(a.cosine(&b), 24.0 / 25.0));
        assert_eq!(Embedding::new(&[0.0, 0.0]), Err(EmbeddingError::Zero));
        assert_eq!(Embedding::new(&[f32::NAN]), Err(EmbeddingError::NonFinite));
    }

    #[test]
    fn cosine_is_clamped_to_one() {
        let a = Embedding::new(&[1.0, 1.0, 4.0]).unwrap();
        assert!(a.0.iter().map(|x| x * x).sum::<f64>() > 1.0);
        assert!(a.cosine(&a) <= 1.0);
    }
}
