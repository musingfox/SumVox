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

/// EBU R128 integrated loudness target (LUFS). -16 is a common target for
/// speech played back on consumer devices.
const TARGET_I: &str = "-16";
/// Max true peak (dBTP), leaving headroom to avoid clipping.
const TARGET_TP: &str = "-1.5";
/// Loudness range. Lower values compress dynamic swings more aggressively,
/// which is exactly the "loud/soft" problem we want to tame.
const TARGET_LRA: &str = "11";

/// Measurements emitted by `loudnorm`'s first pass (`print_format=json`).
#[derive(serde::Deserialize)]
struct LoudnormStats {
    input_i: String,
    input_tp: String,
    input_lra: String,
    input_thresh: String,
    target_offset: String,
}

/// Normalize the loudness of `audio_data` (any format ffmpeg can decode) and
/// return WAV bytes.
///
/// Returns `None` when ffmpeg is unavailable or any step fails, so the caller
/// can fall back to playing the original audio unmodified.
pub fn normalize_to_wav(audio_data: &[u8], temp_prefix: &str) -> Option<Vec<u8>> {
    let dir = std::env::temp_dir();
    let in_path = dir.join(format!("{}_norm_in", temp_prefix));
    let out_path = dir.join(format!("{}_norm_out.wav", temp_prefix));

    if let Err(e) = std::fs::File::create(&in_path).and_then(|mut f| f.write_all(audio_data)) {
        tracing::debug!("loudnorm: failed to write temp input: {}", e);
        return None;
    }

    let result = (|| {
        let stats = measure(&in_path)?;
        apply(&in_path, &out_path, &stats)?;
        std::fs::read(&out_path).ok()
    })();

    let _ = std::fs::remove_file(&in_path);
    let _ = std::fs::remove_file(&out_path);
    result
}

/// First pass: measure the clip's loudness. Returns parsed stats, or `None` if
/// ffmpeg is missing, exits non-zero, or its JSON can't be parsed.
fn measure(in_path: &std::path::Path) -> Option<LoudnormStats> {
    let output = std::process::Command::new("ffmpeg")
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

    // loudnorm prints its JSON block to stderr; it's the last `{ ... }` there.
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
    let status = std::process::Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(in_path)
        .arg("-af")
        .arg(format!(
            "loudnorm=I={}:TP={}:LRA={}:measured_I={}:measured_TP={}:measured_LRA={}:measured_thresh={}:offset={}:linear=true:print_format=summary",
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
        .stderr(std::process::Stdio::null())
        .status();

    match status {
        Ok(s) if s.success() => Some(()),
        _ => {
            tracing::debug!("loudnorm: apply pass failed, skipping normalization");
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
