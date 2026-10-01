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
    fn test_wav_header_fields() {
        // (sample_rate, channels, byte_rate, block_align)
        for (rate, channels, byte_rate, block_align) in
            [(24000u32, 1u16, 48000u32, 2u16), (44100, 2, 176400, 4)]
        {
            let header = create_wav_header(rate, channels, 16);
            assert_eq!(header.len(), 44);
            assert_eq!(&header[0..4], b"RIFF");
            assert_eq!(&header[8..12], b"WAVE");
            assert_eq!(&header[22..24], &channels.to_le_bytes());
            assert_eq!(&header[24..28], &rate.to_le_bytes());
            assert_eq!(&header[28..32], &byte_rate.to_le_bytes());
            assert_eq!(&header[32..34], &block_align.to_le_bytes());
            assert_eq!(&header[34..36], &16u16.to_le_bytes());
        }
    }

    #[test]
    fn test_wav_file_construction() {
        let pcm = vec![0x00, 0x01, 0x02, 0x03];
        let wav = create_wav_file(&pcm, 24000, 1, 16);
        assert_eq!(wav.len(), 48); // 44 header + 4 data
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[44..48], &pcm);

        // (pcm length, expected RIFF size at 4..8 = 36 + data, data size at 40..44)
        for (len, riff_size) in [(0usize, 36u32), (1000, 1036)] {
            let wav = create_wav_file(&vec![0u8; len], 24000, 1, 16);
            assert_eq!(wav.len(), 44 + len);
            assert_eq!(&wav[4..8], &riff_size.to_le_bytes());
            assert_eq!(&wav[40..44], &(len as u32).to_le_bytes());
        }
    }
}
