// Audio module - audio file playback support

pub mod afplay;
pub mod file;
pub mod normalize;
pub mod wav_header;

pub use file::AudioFileProvider;
