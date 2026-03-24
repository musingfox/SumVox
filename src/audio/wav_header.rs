// WAV header construction utilities
// Creates RIFF WAV headers and complete WAV files from raw PCM data

/// Create a 44-byte RIFF WAV header
///
/// # Arguments
/// * `sample_rate` - Sample rate in Hz (e.g., 24000, 44100)
/// * `num_channels` - Number of channels (1 = mono, 2 = stereo)
/// * `bits_per_sample` - Bits per sample (8, 16, 24, 32)
///
/// # Returns
/// 44-byte WAV header as Vec<u8>
pub fn create_wav_header(sample_rate: u32, num_channels: u16, bits_per_sample: u16) -> Vec<u8> {
    let mut header = Vec::with_capacity(44);

    // RIFF header
    header.extend_from_slice(b"RIFF");
    // File size - 8 (placeholder, will be correct when data is appended)
    header.extend_from_slice(&36u32.to_le_bytes());
    header.extend_from_slice(b"WAVE");

    // fmt chunk
    header.extend_from_slice(b"fmt ");
    header.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    header.extend_from_slice(&1u16.to_le_bytes()); // audio format (1 = PCM)
    header.extend_from_slice(&num_channels.to_le_bytes());
    header.extend_from_slice(&sample_rate.to_le_bytes());

    // Byte rate = sample_rate * num_channels * bits_per_sample / 8
    let byte_rate = sample_rate * u32::from(num_channels) * u32::from(bits_per_sample) / 8;
    header.extend_from_slice(&byte_rate.to_le_bytes());

    // Block align = num_channels * bits_per_sample / 8
    let block_align = num_channels * bits_per_sample / 8;
    header.extend_from_slice(&block_align.to_le_bytes());

    header.extend_from_slice(&bits_per_sample.to_le_bytes());

    // data chunk header
    header.extend_from_slice(b"data");
    // Data size (placeholder)
    header.extend_from_slice(&0u32.to_le_bytes());

    header
}

/// Create a complete WAV file (header + PCM data)
///
/// # Arguments
/// * `pcm_data` - Raw PCM audio data bytes
/// * `sample_rate` - Sample rate in Hz (e.g., 24000, 44100)
/// * `num_channels` - Number of channels (1 = mono, 2 = stereo)
/// * `bits_per_sample` - Bits per sample (8, 16, 24, 32)
///
/// # Returns
/// Complete WAV file as Vec<u8> (44-byte header + pcm_data)
pub fn create_wav_file(
    pcm_data: &[u8],
    sample_rate: u32,
    num_channels: u16,
    bits_per_sample: u16,
) -> Vec<u8> {
    let mut wav = create_wav_header(sample_rate, num_channels, bits_per_sample);

    // Update RIFF chunk size (file size - 8)
    let riff_size = (36 + pcm_data.len()) as u32;
    wav[4..8].copy_from_slice(&riff_size.to_le_bytes());

    // Update data chunk size
    let data_size = pcm_data.len() as u32;
    wav[40..44].copy_from_slice(&data_size.to_le_bytes());

    // Append PCM data
    wav.extend_from_slice(pcm_data);

    wav
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wav_header_24khz_mono_16bit() {
        let header = create_wav_header(24000, 1, 16);
        assert_eq!(header.len(), 44);
        assert_eq!(&header[0..4], b"RIFF");
        assert_eq!(&header[8..12], b"WAVE");
        assert_eq!(&header[22..24], &1u16.to_le_bytes()); // mono
        assert_eq!(&header[24..28], &24000u32.to_le_bytes()); // sample rate
        assert_eq!(&header[34..36], &16u16.to_le_bytes()); // bits per sample
    }

    #[test]
    fn test_wav_header_44khz_stereo_16bit() {
        let header = create_wav_header(44100, 2, 16);
        assert_eq!(header.len(), 44);
        assert_eq!(&header[22..24], &2u16.to_le_bytes()); // stereo
        assert_eq!(&header[24..28], &44100u32.to_le_bytes()); // sample rate
        assert_eq!(&header[34..36], &16u16.to_le_bytes()); // bits per sample
    }

    #[test]
    fn test_wav_header_byte_rate_calculation() {
        // 24000 Hz, mono, 16-bit = 24000 * 1 * 16/8 = 48000 bytes/sec
        let header = create_wav_header(24000, 1, 16);
        let byte_rate = u32::from_le_bytes([header[28], header[29], header[30], header[31]]);
        assert_eq!(byte_rate, 48000);
    }

    #[test]
    fn test_wav_header_block_align_calculation() {
        // stereo, 16-bit = 2 * 16/8 = 4 bytes per sample block
        let header = create_wav_header(44100, 2, 16);
        let block_align = u16::from_le_bytes([header[32], header[33]]);
        assert_eq!(block_align, 4);
    }

    #[test]
    fn test_wav_file_construction() {
        let pcm = vec![0x00, 0x01, 0x02, 0x03];
        let wav = create_wav_file(&pcm, 24000, 1, 16);
        assert_eq!(wav.len(), 48); // 44 header + 4 data
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[44..48], &pcm);
    }

    #[test]
    fn test_wav_file_riff_size() {
        let pcm = vec![0x00; 1000];
        let wav = create_wav_file(&pcm, 24000, 1, 16);
        // RIFF size at bytes 4-7 should be 36 + 1000 = 1036
        let riff_size = u32::from_le_bytes([wav[4], wav[5], wav[6], wav[7]]);
        assert_eq!(riff_size, 1036);
    }

    #[test]
    fn test_wav_file_data_size() {
        let pcm = vec![0x00; 1000];
        let wav = create_wav_file(&pcm, 24000, 1, 16);
        // Data size at bytes 40-43 should be 1000
        let data_size = u32::from_le_bytes([wav[40], wav[41], wav[42], wav[43]]);
        assert_eq!(data_size, 1000);
    }

    #[test]
    fn test_wav_file_empty_pcm() {
        let pcm: Vec<u8> = vec![];
        let wav = create_wav_file(&pcm, 24000, 1, 16);
        assert_eq!(wav.len(), 44); // header only
        let data_size = u32::from_le_bytes([wav[40], wav[41], wav[42], wav[43]]);
        assert_eq!(data_size, 0);
    }
}
