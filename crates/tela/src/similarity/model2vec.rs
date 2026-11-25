//! `Model2Vec` static embeddings, pooled as `townsendmerino/aikit/embed`
//! pools them for henia.

mod wordpiece;

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use safetensors::tensor::TensorView;
use safetensors::{Dtype, SafeTensorError, SafeTensors};
use serde::Deserialize;

use super::{Embed, EmbedError};
use wordpiece::WordPiece;

const L2_NORM_EPSILON: f64 = 1e-12;

/// A `Model2Vec` model read from a directory holding `tokenizer.json`,
/// `config.json` and `model.safetensors`.
pub struct Model2Vec {
    name: String,
    tokenizer: WordPiece,
    embeddings: Vec<f32>,
    vocab: usize,
    dim: usize,
    mapping: Option<Vec<i64>>,
    weights: Option<Vec<f64>>,
    normalize: bool,
}

/// A model directory that failed to load.
#[derive(Debug, thiserror::Error)]
#[error("semantic model {} failed to load", model.display())]
pub struct LoadError {
    model: PathBuf,
    #[source]
    reason: Reason,
}

impl LoadError {
    /// The model directory.
    #[must_use]
    pub fn model(&self) -> &Path {
        &self.model
    }
}

#[derive(Debug, thiserror::Error)]
enum Reason {
    #[error("cannot read {file}")]
    Read {
        file: &'static str,
        #[source]
        source: io::Error,
    },
    #[error("{file} is malformed")]
    Json {
        file: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("model.safetensors is malformed")]
    Tensors(#[from] SafeTensorError),
    #[error("tokenizer.json is not a WordPiece tokenizer with BertNormalizer and BertPreTokenizer")]
    Tokenizer,
    #[error("unknown token {0:?} is in neither the vocabulary nor the added tokens")]
    UnknownToken(String),
    #[error("embedding dtype {0:?} is not float32")]
    EmbeddingDtype(String),
    #[error("tensor {name:?} has dtype {dtype}, expected {expected}")]
    TensorDtype {
        name: &'static str,
        dtype: Dtype,
        expected: Dtype,
    },
    #[error("tensor {name:?} has shape {shape:?}")]
    TensorShape {
        name: &'static str,
        shape: Vec<usize>,
    },
    #[error("model has no embedding dimensions")]
    NoDimensions,
}

#[derive(Deserialize)]
struct Config {
    #[serde(default)]
    normalize: bool,
    #[serde(default)]
    embedding_dtype: String,
}

impl Model2Vec {
    /// Loads the model in `dir`, named by the directory in scores and errors.
    ///
    /// # Errors
    /// Returns [`LoadError`] when a file is missing or malformed, or the model
    /// is not a float32 `Model2Vec` model with a `WordPiece` tokenizer.
    pub fn load(dir: impl AsRef<Path>) -> Result<Self, LoadError> {
        let dir = dir.as_ref();
        Self::read(dir).map_err(|reason| LoadError {
            model: dir.to_owned(),
            reason,
        })
    }

    fn read(dir: &Path) -> Result<Self, Reason> {
        let tokenizer = WordPiece::from_json(&read(dir, "tokenizer.json")?)?;
        let config: Config = parse(&read(dir, "config.json")?, "config.json")?;
        if !matches!(config.embedding_dtype.as_str(), "" | "float32") {
            return Err(Reason::EmbeddingDtype(config.embedding_dtype));
        }
        let bytes = read(dir, "model.safetensors")?;
        let tensors = SafeTensors::deserialize(&bytes)?;
        let embeddings = tensors.tensor("embeddings")?;
        let &[vocab, dim] = embeddings.shape() else {
            return Err(shape_error("embeddings", embeddings.shape()));
        };
        if dim == 0 {
            return Err(Reason::NoDimensions);
        }
        let embeddings = decode("embeddings", &embeddings, Dtype::F32, f32::from_le_bytes)?;
        let mapping = optional(&tensors, "mapping", vocab, Dtype::I64, i64::from_le_bytes)?;
        let weights = optional(&tensors, "weights", vocab, Dtype::F64, f64::from_le_bytes)?;
        Ok(Self {
            name: dir.display().to_string(),
            tokenizer,
            embeddings,
            vocab,
            dim,
            mapping,
            weights,
            normalize: config.normalize,
        })
    }

    /// The weighted mean of the rows of `text`'s tokens, L2-normalized when
    /// the model's config asks for it; the zero vector when no token has a row.
    #[must_use]
    pub fn encode(&self, text: &str) -> Vec<f32> {
        let mut sum = vec![0.0_f64; self.dim];
        let mut total = 0.0_f64;
        for id in self.tokenizer.encode(text) {
            let Some((row, weight)) = self.row(id) else {
                continue;
            };
            for (acc, &x) in sum.iter_mut().zip(row) {
                *acc += f64::from(x) * weight;
            }
            total += weight;
        }
        if total == 0.0 {
            return vec![0.0; self.dim];
        }
        let mean = sum.into_iter().map(|acc| narrow(acc / total)).collect();
        if self.normalize {
            l2_normalize(mean)
        } else {
            mean
        }
    }

    fn row(&self, id: u32) -> Option<(&[f32], f64)> {
        let id = usize::try_from(id).ok().filter(|&id| id < self.vocab)?;
        let row = match &self.mapping {
            Some(mapping) => usize::try_from(*mapping.get(id)?)
                .ok()
                .filter(|&row| row < self.vocab)?,
            None => id,
        };
        let weight = match &self.weights {
            Some(weights) => *weights.get(id)?,
            None => 1.0,
        };
        let start = row * self.dim;
        Some((self.embeddings.get(start..start + self.dim)?, weight))
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
            .field("vocab", &self.vocab)
            .field("dim", &self.dim)
            .field("normalize", &self.normalize)
            .finish_non_exhaustive()
    }
}

fn read(dir: &Path, file: &'static str) -> Result<Vec<u8>, Reason> {
    fs::read(dir.join(file)).map_err(|source| Reason::Read { file, source })
}

fn parse<'de, T: Deserialize<'de>>(bytes: &'de [u8], file: &'static str) -> Result<T, Reason> {
    serde_json::from_slice(bytes).map_err(|source| Reason::Json { file, source })
}

fn shape_error(name: &'static str, shape: &[usize]) -> Reason {
    Reason::TensorShape {
        name,
        shape: shape.to_vec(),
    }
}

fn decode<T, const N: usize>(
    name: &'static str,
    tensor: &TensorView<'_>,
    expected: Dtype,
    from_le_bytes: fn([u8; N]) -> T,
) -> Result<Vec<T>, Reason> {
    if tensor.dtype() != expected {
        return Err(Reason::TensorDtype {
            name,
            dtype: tensor.dtype(),
            expected,
        });
    }
    Ok(tensor
        .data()
        .as_chunks::<N>()
        .0
        .iter()
        .map(|&bytes| from_le_bytes(bytes))
        .collect())
}

fn optional<T, const N: usize>(
    tensors: &SafeTensors<'_>,
    name: &'static str,
    vocab: usize,
    expected: Dtype,
    from_le_bytes: fn([u8; N]) -> T,
) -> Result<Option<Vec<T>>, Reason> {
    let Ok(tensor) = tensors.tensor(name) else {
        return Ok(None);
    };
    if tensor.shape() != [vocab] {
        return Err(shape_error(name, tensor.shape()));
    }
    decode(name, &tensor, expected, from_le_bytes).map(Some)
}

fn l2_normalize(mut vector: Vec<f32>) -> Vec<f32> {
    let norm = vector
        .iter()
        .map(|&x| f64::from(x) * f64::from(x))
        .sum::<f64>()
        .sqrt();
    for x in &mut vector {
        *x = if norm < L2_NORM_EPSILON {
            0.0
        } else {
            narrow(f64::from(*x) / norm)
        };
    }
    vector
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "Model2Vec vectors are f32, rounded from f64 pooling as aikit rounds them"
)]
fn narrow(x: f64) -> f32 {
    x as f32
}

#[cfg(test)]
mod tests {
    use std::env;

    use super::*;
    use crate::similarity::Embedding;
    use crate::similarity::tests::{GATE, GATE_PARAPHRASED, GATE_REWORDED};

    /// The pinned model `mise run model` fetches, or `$PARITY_MODEL`.
    fn pinned_model() -> Option<Model2Vec> {
        let dir = env::var_os("PARITY_MODEL").map(PathBuf::from).or_else(|| {
            let cache = env::var_os("XDG_CACHE_HOME")
                .map(PathBuf::from)
                .or_else(|| env::home_dir().map(|home| home.join(".cache")))?;
            Some(cache.join("tela/models/potion-base-2M-389b9f6"))
        })?;
        if !dir.is_dir() {
            eprintln!(
                "skipped: no model at {}; run `mise run model`",
                dir.display()
            );
            return None;
        }
        Some(Model2Vec::load(dir).unwrap())
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
        assert!(matches!(
            error.reason,
            Reason::Read {
                file: "tokenizer.json",
                ..
            }
        ));
    }
}
