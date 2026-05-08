use std::path::PathBuf;
use std::sync::Arc;
use ort::session::Session;
use tokio::sync::OnceCell;

static MODEL: OnceCell<Arc<EmbeddingModel>> = OnceCell::const_new();

const MODEL_REPO: &str = "sentence-transformers/all-MiniLM-L6-v2";
const MODEL_FILE: &str = "model.onnx";
const TOKENIZER_FILE: &str = "tokenizer.json";
const EMBEDDING_DIM: usize = 384;

/// Convert ort errors to anyhow (ort::Error is not Send+Sync).
fn ort_err(e: impl std::fmt::Display) -> anyhow::Error {
    anyhow::anyhow!("ort error: {}", e)
}

/// Wraps the ONNX session behind a Mutex since ort::Session is not Sync.
pub struct EmbeddingModel {
    session: std::sync::Mutex<Session>,
    tokenizer: tokenizers::Tokenizer,
}

unsafe impl Sync for EmbeddingModel {}

impl EmbeddingModel {
    /// Embed a single text string into a 384-dim vector.
    pub fn embed_single(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        let results = self.embed_batch(&[text])?;
        results.into_iter().next().ok_or_else(|| anyhow::anyhow!("Empty embedding result"))
    }

    /// Embed a batch of texts into 384-dim vectors.
    pub fn embed_batch(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(vec![]);
        }

        let encodings = self.tokenizer.encode_batch(texts.to_vec(), true)
            .map_err(|e| anyhow::anyhow!("Tokenization failed: {}", e))?;

        let batch_size = encodings.len();
        let max_len = encodings.iter().map(|e| e.get_ids().len()).max().unwrap_or(0);

        let mut input_ids = vec![0i64; batch_size * max_len];
        let mut attention_mask = vec![0i64; batch_size * max_len];
        let mut token_type_ids = vec![0i64; batch_size * max_len];

        for (i, enc) in encodings.iter().enumerate() {
            for (j, &id) in enc.get_ids().iter().enumerate() {
                input_ids[i * max_len + j] = id as i64;
            }
            for (j, &mask) in enc.get_attention_mask().iter().enumerate() {
                attention_mask[i * max_len + j] = mask as i64;
            }
            for (j, &tid) in enc.get_type_ids().iter().enumerate() {
                token_type_ids[i * max_len + j] = tid as i64;
            }
        }

        let ids_tensor = ort::value::Tensor::from_array(
            (vec![batch_size as i64, max_len as i64], input_ids)
        ).map_err(ort_err)?;
        let mask_tensor = ort::value::Tensor::from_array(
            (vec![batch_size as i64, max_len as i64], attention_mask)
        ).map_err(ort_err)?;
        let type_tensor = ort::value::Tensor::from_array(
            (vec![batch_size as i64, max_len as i64], token_type_ids)
        ).map_err(ort_err)?;

        let mut session = self.session.lock()
            .map_err(|e| anyhow::anyhow!("Session lock poisoned: {}", e))?;

        let inputs = ort::inputs![
            "input_ids" => ids_tensor,
            "attention_mask" => mask_tensor,
            "token_type_ids" => type_tensor,
        ];

        let outputs = session.run(inputs).map_err(ort_err)?;

        // Output 0: last_hidden_state — extract as flat f32 slice
        let output_value = &outputs[0];
        let (shape, raw_data) = output_value.try_extract_tensor::<f32>().map_err(ort_err)?;

        // shape: [batch_size, seq_len, 384]
        let out_seq_len = shape[1] as usize;

        let mut results = Vec::with_capacity(batch_size);

        for i in 0..batch_size {
            let active_tokens = encodings[i].get_attention_mask().iter()
                .filter(|&&m| m == 1)
                .count();

            // Mean pool over non-padding tokens
            let mut embedding = vec![0.0f32; EMBEDDING_DIM];
            if active_tokens > 0 {
                let batch_offset = i * out_seq_len * EMBEDDING_DIM;
                for j in 0..active_tokens {
                    let tok_offset = batch_offset + j * EMBEDDING_DIM;
                    for k in 0..EMBEDDING_DIM {
                        embedding[k] += raw_data[tok_offset + k];
                    }
                }
                let scale = 1.0 / active_tokens as f32;
                for v in &mut embedding {
                    *v *= scale;
                }
            }

            // L2 normalize
            let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 0.0 {
                for v in &mut embedding {
                    *v /= norm;
                }
            }

            results.push(embedding);
        }

        Ok(results)
    }
}

/// Get or initialize the embedding model (lazy singleton).
pub async fn get_model(data_dir: &str) -> anyhow::Result<Arc<EmbeddingModel>> {
    MODEL.get_or_try_init(|| async {
        let model_dir = ensure_model_downloaded(data_dir).await?;
        let dir = model_dir.clone();
        tokio::task::spawn_blocking(move || load_model(&dir))
            .await
            .map_err(|e| anyhow::anyhow!("Model load task failed: {}", e))?
    }).await.cloned()
}

/// Check if the ONNX model is downloaded and ready.
pub fn is_model_ready(data_dir: &str) -> bool {
    let dir = model_dir(data_dir);
    dir.join(MODEL_FILE).exists() && dir.join(TOKENIZER_FILE).exists()
}

fn model_dir(data_dir: &str) -> PathBuf {
    PathBuf::from(data_dir).join("models").join("all-MiniLM-L6-v2")
}

async fn ensure_model_downloaded(data_dir: &str) -> anyhow::Result<PathBuf> {
    let dir = model_dir(data_dir);
    std::fs::create_dir_all(&dir)?;

    let model_path = dir.join(MODEL_FILE);
    let tokenizer_path = dir.join(TOKENIZER_FILE);

    let client = reqwest::Client::new();

    if !model_path.exists() {
        let url = format!(
            "https://huggingface.co/{}/resolve/main/onnx/model.onnx",
            MODEL_REPO
        );
        tracing::info!("Downloading ONNX model from {}...", url);
        download_file(&client, &url, &model_path).await?;
        tracing::info!("ONNX model downloaded ({:.1} MB)",
            std::fs::metadata(&model_path)?.len() as f64 / 1_048_576.0);
    }

    if !tokenizer_path.exists() {
        let url = format!(
            "https://huggingface.co/{}/resolve/main/tokenizer.json",
            MODEL_REPO
        );
        tracing::info!("Downloading tokenizer from {}...", url);
        download_file(&client, &url, &tokenizer_path).await?;
        tracing::info!("Tokenizer downloaded");
    }

    Ok(dir)
}

async fn download_file(client: &reqwest::Client, url: &str, dest: &PathBuf) -> anyhow::Result<()> {
    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        anyhow::bail!("Download failed: HTTP {}", resp.status());
    }
    let bytes = resp.bytes().await?;
    std::fs::write(dest, &bytes)?;
    Ok(())
}

fn load_model(dir: &PathBuf) -> anyhow::Result<Arc<EmbeddingModel>> {
    let model_path = dir.join(MODEL_FILE);
    let tokenizer_path = dir.join(TOKENIZER_FILE);

    tracing::info!("Loading ONNX embedding model from {}", dir.display());

    let session = Session::builder()
        .map_err(ort_err)?
        .with_intra_threads(2)
        .map_err(ort_err)?
        .commit_from_file(&model_path)
        .map_err(ort_err)?;

    let tokenizer = tokenizers::Tokenizer::from_file(&tokenizer_path)
        .map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {}", e))?;

    tracing::info!("ONNX embedding model loaded (384-dim, all-MiniLM-L6-v2)");

    Ok(Arc::new(EmbeddingModel {
        session: std::sync::Mutex::new(session),
        tokenizer,
    }))
}

/// Cosine similarity between two normalized vectors.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

/// Serialize a f32 vector to bytes for SQLite BLOB storage.
pub fn vec_to_bytes(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Deserialize bytes from SQLite BLOB back to f32 vector.
pub fn bytes_to_vec(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vec_to_bytes_roundtrip() {
        let original = vec![1.0f32, -2.5, 0.0, 3.14159, f32::MAX, f32::MIN];
        let bytes = vec_to_bytes(&original);
        let recovered = bytes_to_vec(&bytes);
        assert_eq!(original, recovered);
    }

    #[test]
    fn test_vec_to_bytes_empty() {
        let empty: Vec<f32> = vec![];
        let bytes = vec_to_bytes(&empty);
        assert!(bytes.is_empty());
        let recovered = bytes_to_vec(&bytes);
        assert!(recovered.is_empty());
    }

    #[test]
    fn test_vec_to_bytes_size() {
        let v = vec![0.0f32; 384]; // EMBEDDING_DIM
        let bytes = vec_to_bytes(&v);
        assert_eq!(bytes.len(), 384 * 4); // 1536 bytes
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &a) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &b)).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![-1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_normalized_vectors() {
        // Two normalized vectors at ~45 degrees
        let a = vec![0.7071, 0.7071, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim > 0.7 && sim < 0.71, "Expected ~0.707, got {}", sim);
    }

    #[test]
    fn test_cosine_similarity_empty() {
        let a: Vec<f32> = vec![];
        let b: Vec<f32> = vec![];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn test_model_dir_path() {
        let dir = model_dir("/some/data");
        assert!(dir.ends_with("models/all-MiniLM-L6-v2") || dir.ends_with("models\\all-MiniLM-L6-v2"));
    }

    #[test]
    fn test_is_model_ready_missing_dir() {
        assert!(!is_model_ready("/nonexistent/path/that/does/not/exist"));
    }
}
