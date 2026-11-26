//! `Model2Vec` static embeddings through `model2vec-rs`.

use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

use model2vec_rs::model::StaticModel;

use super::{Embed, EmbedError};

/// A `Model2Vec` model read from a local directory.
pub struct Model2Vec {
    name: String,
    model: StaticModel,
}

/// A model directory that failed to load.
#[derive(Debug, thiserror::Error)]
#[error("semantic model {} failed to load", model.display())]
pub struct LoadError {
    model: PathBuf,
    #[source]
    source: Box<dyn Error + Send + Sync>,
}

impl LoadError {
    /// The model directory.
    #[must_use]
    pub fn model(&self) -> &Path {
        &self.model
    }
}

impl Model2Vec {
    /// Loads the model in `dir`, named by the directory in scores and errors.
    ///
    /// # Errors
    /// Returns [`LoadError`] when the directory holds no readable `Model2Vec`
    /// model.
    pub fn load(dir: impl AsRef<Path>) -> Result<Self, LoadError> {
        let dir = dir.as_ref();
        let model =
            StaticModel::from_pretrained(dir, None, None, None).map_err(|error| LoadError {
                model: dir.to_owned(),
                source: error.into(),
            })?;
        Ok(Self {
            name: dir.display().to_string(),
            model,
        })
    }

    /// The embedding of `text`; the zero vector when no token has a row.
    #[must_use]
    pub fn encode(&self, text: &str) -> Vec<f32> {
        self.model.encode_single(text)
    }
}

impl Embed for Model2Vec {
    fn model(&self) -> &str {
        &self.name
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
        Ok(self.encode(text))
    }
}

impl fmt::Debug for Model2Vec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Model2Vec")
            .field("name", &self.name)
            .field("dim", &self.model.dim())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use std::env;

    use super::*;
    use crate::similarity::Embedding;
    use crate::similarity::tests::{GATE, GATE_PARAPHRASED, GATE_REWORDED};

    /// The pinned model, or none to skip a test; a run under `CI` or with
    /// `PARITY_MODEL` set needs it.
    fn pinned_model() -> Option<Model2Vec> {
        let required = env::var_os("PARITY_MODEL").is_some()
            || env::var_os("CI").is_some_and(|ci| !ci.is_empty() && ci != "false");
        let dir = env::var_os("PARITY_MODEL").map(PathBuf::from).or_else(|| {
            let cache = env::var_os("XDG_CACHE_HOME")
                .map(PathBuf::from)
                .or_else(|| env::home_dir().map(|home| home.join(".cache")))?;
            Some(cache.join("satz/models/potion-base-2M-389b9f6"))
        });
        if let Some(dir) = dir.filter(|dir| dir.is_dir()) {
            return Some(Model2Vec::load(dir).unwrap());
        }
        assert!(!required, "no pinned model; run `mise run model`");
        eprintln!("skipped: no pinned model; run `mise run model`");
        None
    }

    fn cosine(model: &Model2Vec, a: &str, b: &str) -> f64 {
        let a = Embedding::new(&model.encode(a)).unwrap();
        let b = Embedding::new(&model.encode(b)).unwrap();
        a.cosine(&b)
    }

    #[test]
    fn similar_scores_match_henia() {
        let Some(model) = pinned_model() else {
            return;
        };
        for (a, b, henia) in [
            (GATE, GATE_REWORDED, 0.981_988_842_390_909_3),
            (GATE, GATE_PARAPHRASED, 0.743_085_395_641_899_9),
            (GATE_REWORDED, GATE_PARAPHRASED, 0.797_896_089_802_974_4),
        ] {
            let score = cosine(&model, a, b);
            assert!((score - henia).abs() < 1e-6, "{score} != {henia}");
        }
    }

    #[test]
    fn empty_text_embeds_as_the_zero_vector() {
        let Some(model) = pinned_model() else {
            return;
        };
        assert!(model.encode("").iter().all(|&x| x == 0.0));
    }

    #[test]
    fn missing_directory_names_the_model() {
        let error = Model2Vec::load("no/such/model").unwrap_err();
        assert_eq!(error.model(), Path::new("no/such/model"));
    }
}
