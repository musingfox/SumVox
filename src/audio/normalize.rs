// Loudness normalization via ffmpeg's `loudnorm` filter.
//
// ElevenLabs output (especially the highly expressive `eleven_v3` model) is not
// loudness-normalized, so notifications swing loud/soft between — and within —
// generations. Running each clip through `loudnorm` evens it out to a fixed
// integrated loudness target before playback.
//
// We use the two-pass (measure → apply) flow: a single-pass `loudnorm` runs in
// a time-varying "dynamic" mode that misses the target by several LUFS and is
// inconsistent across clips. The first pass measures the clip, the second pass
// feeds those measurements back with `linear=true` so ffmpeg applies one fixed
// gain — accurate and consistent from clip to clip.

use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};

/// Monotonic per-call counter so concurrent calls — even within one process,
/// where the PID is identical — never collide on temp-file names.
static CALL_SEQ: AtomicU64 = AtomicU64::new(0);

/// EBU R128 integrated loudness target (LUFS). -16 is a common target for
/// speech played back on consumer devices.
const TARGET_I: &str = "-16";
/// Max true peak (dBTP), leaving headroom to avoid clipping.
const TARGET_TP: &str = "-1.5";
/// Loudness range. Lower values compress dynamic swings more aggressively,
/// which is exactly the "loud/soft" problem we want to tame.
const TARGET_LRA: &str = "11";

/// Measurements emitted by `loudnorm`'s first pass (`print_format=json`).
///
/// ffmpeg encodes each value as a JSON *string* (e.g. `"input_i" : "-15.66"`),
/// so we parse it through `de_finite_f64`, which both converts to `f64` and
/// rejects non-finite values — ffmpeg emits `"inf"` for silent clips — rather
/// than letting them flow verbatim into the second pass's filter string.
#[derive(serde::Deserialize)]
struct LoudnormStats {
    #[serde(deserialize_with = "de_finite_f64")]
    input_i: f64,
    #[serde(deserialize_with = "de_finite_f64")]
    input_tp: f64,
    #[serde(deserialize_with = "de_finite_f64")]
    input_lra: f64,
    #[serde(deserialize_with = "de_finite_f64")]
    input_thresh: f64,
    #[serde(deserialize_with = "de_finite_f64")]
    target_offset: f64,
}

/// Parse loudnorm's stringified number into a finite `f64`, rejecting `inf` /
/// `nan` / non-numeric so a bad measurement becomes a clean `None` upstream.
fn de_finite_f64<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let s = String::deserialize(deserializer)?;
    let v: f64 = s.parse().map_err(serde::de::Error::custom)?;
    if v.is_finite() {
        Ok(v)
    } else {
        Err(serde::de::Error::custom(format!("non-finite value: {}", s)))
    }
}

/// Normalize the loudness of `audio_data` (any format ffmpeg can decode) and
/// return WAV bytes.
///
/// Returns `None` when ffmpeg is unavailable or any step fails, so the caller
/// can fall back to playing the original audio unmodified.
pub fn normalize_to_wav(audio_data: &[u8], temp_prefix: &str) -> Option<Vec<u8>> {
    let dir = std::env::temp_dir();
    // Qualify temp names with PID + a per-call counter so concurrent calls
    // (across or within a process) never clobber each other's files. The input
    // keeps an `.mp3` extension so ffmpeg's demuxer detection is unambiguous.
    let id = format!(
        "{}_{}",
        std::process::id(),
        CALL_SEQ.fetch_add(1, Ordering::Relaxed)
    );
    let in_path = dir.join(format!("{}_norm_{}_in.mp3", temp_prefix, id));
    let out_path = dir.join(format!("{}_norm_{}_out.wav", temp_prefix, id));

    if let Err(e) = std::fs::File::create(&in_path).and_then(|mut f| f.write_all(audio_data)) {
        tracing::debug!("loudnorm: failed to write temp input: {}", e);
        return None;
    }

    let result = (|| {
        let stats = measure(&in_path)?;
        apply(&in_path, &out_path, &stats)?;
        std::fs::read(&out_path)
            .map_err(|e| tracing::debug!("loudnorm: failed to read normalized output: {}", e))
            .ok()
    })();

    let _ = std::fs::remove_file(&in_path);
    let _ = std::fs::remove_file(&out_path);
    result
}

/// First pass: measure the clip's loudness. Returns parsed stats, or `None` if
/// ffmpeg is missing, exits non-zero, or its JSON can't be parsed.
fn measure(in_path: &std::path::Path) -> Option<LoudnormStats> {
    let output = std::process::Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-nostats")
        .arg("-i")
        .arg(in_path)
        .arg("-af")
        .arg(format!(
            "loudnorm=I={}:TP={}:LRA={}:print_format=json",
            TARGET_I, TARGET_TP, TARGET_LRA
        ))
        .arg("-f")
        .arg("null")
        .arg("-")
        .stdout(std::process::Stdio::null())
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        Ok(_) => {
            tracing::debug!("loudnorm: measure pass exited non-zero, skipping normalization");
            return None;
        }
        Err(e) => {
            tracing::debug!(
                "loudnorm: ffmpeg unavailable ({}), skipping normalization",
                e
            );
            return None;
        }
    };

    // loudnorm prints its JSON block to stderr; with -hide_banner -nostats it's
    // the only `{ ... }` there, so the last one is reliably the stats block.
    let stderr = String::from_utf8_lossy(&output.stderr);
    let json = stderr.rfind('{').and_then(|start| {
        stderr[start..]
            .rfind('}')
            .map(|end| &stderr[start..start + end + 1])
    })?;

    match serde_json::from_str::<LoudnormStats>(json) {
        Ok(stats) => Some(stats),
        Err(e) => {
            tracing::debug!("loudnorm: failed to parse measure JSON ({})", e);
            None
        }
    }
}

/// Second pass: apply a single fixed gain (`linear=true`) using the measured
/// values, writing a normalized WAV to `out_path`.
fn apply(
    in_path: &std::path::Path,
    out_path: &std::path::Path,
    stats: &LoudnormStats,
) -> Option<()> {
    let output = std::process::Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-nostats")
        .arg("-y")
        .arg("-i")
        .arg(in_path)
        .arg("-af")
        .arg(format!(
            "loudnorm=I={}:TP={}:LRA={}:measured_I={:.2}:measured_TP={:.2}:measured_LRA={:.2}:measured_thresh={:.2}:offset={:.2}:linear=true:print_format=summary",
            TARGET_I,
            TARGET_TP,
            TARGET_LRA,
            stats.input_i,
            stats.input_tp,
            stats.input_lra,
            stats.input_thresh,
            stats.target_offset,
        ))
        .arg("-f")
        .arg("wav")
        .arg(out_path)
        .stdout(std::process::Stdio::null())
        .output();

    match output {
        Ok(o) if o.status.success() => Some(()),
        Ok(o) => {
            tracing::debug!(
                "loudnorm: apply pass failed ({}): {}",
                o.status,
                String::from_utf8_lossy(&o.stderr).trim()
            );
            None
        }
        Err(e) => {
            tracing::debug!("loudnorm: apply pass could not run ({})", e);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_returns_none_on_garbage_input() {
        // Non-decodable bytes: ffmpeg (if present) fails to measure, ffmpeg-absent
        // also yields None. Either way the caller must get None and fall back.
        let result = normalize_to_wav(&[0xDE, 0xAD, 0xBE, 0xEF], "sumvox_test_norm");
        assert!(result.is_none());
    }
}
