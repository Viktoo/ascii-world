//! Kokoro: the open voice model (Apache 2.0) that turns phonemes into
//! speech, run through ONNX Runtime. Its voices are style vectors that blend:
//! a character's voice is a weighted mix of a few of them.

use anyhow::{Context, Result, anyhow, bail};
use ort::session::Session;
use ort::value::Tensor;
use std::collections::HashMap;
use std::path::Path;

pub const SAMPLE_RATE: u32 = 24000;
const STYLE: usize = 256;
/// Most phonemes the model takes at once.
const MAX_TOKENS: usize = 510;

pub struct Kokoro {
    _env: ort::environment::Environment,
    session: Session,
    /// Each voice: one 256-wide style row per phoneme count (510 rows).
    voices: HashMap<String, Vec<f32>>,
    vocab: HashMap<char, i64>,
}

impl Kokoro {
    pub fn load(model: &Path, voices: &Path, threads: usize) -> Result<Kokoro> {
        let env = ort::init().with_name("pocket-voice").build().map_err(|e| anyhow!("{e}"))?;
        let session = Session::builder(&env)
            .map_err(|e| anyhow!("{e}"))?
            .with_intra_threads(threads)
            .map_err(|e| anyhow!("{e}"))?
            .with_inter_threads(1)
            .map_err(|e| anyhow!("{e}"))?
            .commit_from_file(model)
            .map_err(|e| anyhow!("{e}"))
            .with_context(|| format!("loading {}", model.display()))?;
        let voices = read_voices(&std::fs::read(voices).with_context(|| format!("reading {}", voices.display()))?)?;
        let raw: HashMap<String, i64> = serde_json::from_str(include_str!("vocab.json"))?;
        let vocab = raw.into_iter().filter_map(|(k, v)| k.chars().next().map(|c| (c, v))).collect();
        Ok(Kokoro { _env: env, session, voices, vocab })
    }

    pub fn has(&self, voice: &str) -> bool {
        self.voices.contains_key(voice)
    }

    /// The blended style for an utterance of `n` phonemes.
    fn style(&self, blend: &[(&str, f32)], n: usize) -> Vec<f32> {
        let row = n.clamp(1, MAX_TOKENS) - 1;
        let total: f32 = blend.iter().filter(|(v, _)| self.has(v)).map(|(_, w)| *w).sum::<f32>().max(1e-6);
        let mut s = vec![0.0f32; STYLE];
        for (v, w) in blend {
            if let Some(rows) = self.voices.get(*v) {
                for (o, x) in s.iter_mut().zip(&rows[row * STYLE..(row + 1) * STYLE]) {
                    *o += x * w / total;
                }
            }
        }
        s
    }

    fn tokens(&self, phonemes: &str) -> Vec<i64> {
        phonemes.chars().filter_map(|c| self.vocab.get(&c).copied()).collect()
    }

    /// Speak phonemes in the blended voice, `speed` 1 = as the model talks.
    /// Long text goes in pieces cut at punctuation.
    pub fn speak(&mut self, phonemes: &str, blend: &[(&str, f32)], speed: f32) -> Result<Vec<f32>> {
        let mut out = Vec::new();
        for piece in pieces(phonemes) {
            let ids = self.tokens(&piece);
            if ids.is_empty() {
                continue;
            }
            let style = self.style(blend, ids.len());
            let mut padded = Vec::with_capacity(ids.len() + 2);
            padded.push(0);
            padded.extend_from_slice(&ids);
            padded.push(0);
            let n = padded.len();
            let tokens = Tensor::<i64>::from_array(([1usize, n], padded)).map_err(|e| anyhow!("{e}"))?;
            let style = Tensor::<f32>::from_array(([1usize, STYLE], style)).map_err(|e| anyhow!("{e}"))?;
            let speed = Tensor::<f32>::from_array(([1usize], vec![speed.clamp(0.5, 2.0)])).map_err(|e| anyhow!("{e}"))?;
            let res = self.session.run(ort::inputs!["tokens" => tokens, "style" => style, "speed" => speed]).map_err(|e| anyhow!("{e}"))?;
            let (_, audio) = res["audio"].try_extract_tensor::<f32>().map_err(|e| anyhow!("{e}"))?;
            out.extend_from_slice(audio);
        }
        Ok(out)
    }
}

/// Cut phonemes into pieces the model can take, at punctuation, then spaces.
fn pieces(ph: &str) -> Vec<String> {
    let chars: Vec<char> = ph.chars().collect();
    if chars.len() <= MAX_TOKENS {
        return vec![ph.to_string()];
    }
    let mut out = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let end = (start + MAX_TOKENS).min(chars.len());
        let mut cut = end;
        if end < chars.len() {
            let window = &chars[start..end];
            cut = window.iter().rposition(|c| ".!?;:,".contains(*c)).or_else(|| window.iter().rposition(|c| *c == ' ')).map(|i| start + i + 1).unwrap_or(end);
        }
        out.push(chars[start..cut].iter().collect());
        start = cut;
    }
    out
}

/// The voices file: a zip (stored, as numpy writes it) of `.npy` arrays,
/// each 510 × 1 × 256 little-endian f32.
fn read_voices(b: &[u8]) -> Result<HashMap<String, Vec<f32>>> {
    let u16le = |i: usize| -> Result<usize> { b.get(i..i + 2).map(|s| u16::from_le_bytes([s[0], s[1]]) as usize).ok_or_else(|| anyhow!("voices: truncated")) };
    let u32le = |i: usize| -> Result<usize> { b.get(i..i + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]) as usize).ok_or_else(|| anyhow!("voices: truncated")) };
    let mut out = HashMap::new();
    let mut i = 0;
    while i + 30 <= b.len() && b[i..i + 4] == [0x50, 0x4b, 0x03, 0x04] {
        let method = u16le(i + 8)?;
        let mut size = u32le(i + 18)?;
        let nlen = u16le(i + 26)?;
        let xlen = u16le(i + 28)?;
        let name = String::from_utf8_lossy(&b[i + 30..i + 30 + nlen]).to_string();
        // Zip64: the real sizes sit in the extra field.
        if size == 0xFFFF_FFFF {
            let mut x = i + 30 + nlen;
            let xend = x + xlen;
            while x + 4 <= xend {
                let id = u16le(x)?;
                let len = u16le(x + 2)?;
                if id == 1 && len >= 16 {
                    let s = b.get(x + 12..x + 20).ok_or_else(|| anyhow!("voices: truncated"))?;
                    size = u64::from_le_bytes(s.try_into()?) as usize;
                }
                x += 4 + len;
            }
        }
        let data = i + 30 + nlen + xlen;
        if method != 0 {
            bail!("voices: {name} is compressed");
        }
        let body = b.get(data..data + size).ok_or_else(|| anyhow!("voices: truncated"))?;
        if let (Some(stem), Some(v)) = (name.strip_suffix(".npy"), npy_f32(body)) {
            out.insert(stem.to_string(), v);
        }
        i = data + size;
    }
    if out.is_empty() {
        bail!("voices: no voices found");
    }
    Ok(out)
}

fn npy_f32(b: &[u8]) -> Option<Vec<f32>> {
    if b.len() < 10 || &b[..6] != b"\x93NUMPY" {
        return None;
    }
    let (hlen, start) = if b[6] == 1 { (u16::from_le_bytes([b[8], b[9]]) as usize, 10) } else { (u32::from_le_bytes([b[8], b[9], b[10], b[11]]) as usize, 12) };
    let header = std::str::from_utf8(b.get(start..start + hlen)?).ok()?;
    if !header.contains("<f4") || header.contains("'fortran_order': True") {
        return None;
    }
    let data = b.get(start + hlen..)?;
    let v: Vec<f32> = data.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
    (v.len() >= STYLE * MAX_TOKENS).then_some(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_phonemes_cut_at_punctuation() {
        let s = format!("{}. {}", "a".repeat(400), "b".repeat(300));
        let p = pieces(&s);
        assert_eq!(p.len(), 2);
        assert!(p[0].ends_with('.'));
        assert!(p.iter().all(|x| x.chars().count() <= MAX_TOKENS));
    }
}
