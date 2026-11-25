//! Text similarity: word shingles with Jaccard and containment, and cosine
//! over embeddings from a caller's model.

use std::collections::BTreeSet;
use std::sync::Arc;

/// Embeds a text as a vector; the one behavior a caller supplies.
pub type Embedder = Arc<dyn Fn(&str) -> Vec<f32> + Send + Sync>;

/// The lower-cased words of `text`: runs of letters and digits.
#[must_use]
pub fn words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The distinct runs of `size` consecutive words of a text.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Shingles(BTreeSet<String>);

impl Shingles {
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

fn ratio(numerator: usize, denominator: usize) -> f64 {
    let as_f64 = |n: usize| f64::from(u32::try_from(n).unwrap_or(u32::MAX));
    as_f64(numerator) / as_f64(denominator)
}

/// A unit-length embedding.
#[derive(Debug, Clone, PartialEq)]
pub struct Embedding(Vec<f64>);

/// An embedding with a NaN or infinite component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("semantic model returned a non-finite embedding")]
pub struct NonFinite;

impl Embedding {
    /// Normalizes a model's raw vector; `None` for the zero vector.
    ///
    /// # Errors
    /// Returns [`NonFinite`] when a component is NaN or infinite.
    pub fn new(raw: &[f32]) -> Result<Option<Self>, NonFinite> {
        let mut norm = 0.0_f64;
        for &value in raw {
            let x = f64::from(value);
            if !x.is_finite() {
                return Err(NonFinite);
            }
            norm = norm.hypot(x);
        }
        if norm == 0.0 {
            return Ok(None);
        }
        Ok(Some(Self(
            raw.iter().map(|&x| f64::from(x) / norm).collect(),
        )))
    }

    /// The cosine similarity with `other`.
    #[must_use]
    pub fn cosine(&self, other: &Self) -> f64 {
        self.0.iter().zip(&other.0).map(|(a, b)| a * b).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn embeddings_normalize_and_reject_non_finite() {
        let a = Embedding::new(&[3.0, 4.0]).unwrap().unwrap();
        let b = Embedding::new(&[4.0, 3.0]).unwrap().unwrap();
        assert!(close(a.cosine(&b), 24.0 / 25.0));
        assert_eq!(Embedding::new(&[0.0, 0.0]), Ok(None));
        assert_eq!(Embedding::new(&[f32::NAN]), Err(NonFinite));
    }
}
