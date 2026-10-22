use crate::settings::{Region, Settings};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioSource {
    Off,
    Mic,
    System,
}

impl AudioSource {
    pub fn all() -> Vec<AudioSource> {
        vec![AudioSource::Off, AudioSource::Mic, AudioSource::System]
    }
}

impl std::fmt::Display for AudioSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            AudioSource::Off => "No audio",
            AudioSource::Mic => "Microphone",
            AudioSource::System => "System audio",
        })
    }
}

pub struct RecordConfig {
    pub source: AudioSource,
    pub display: usize,
    pub region: Option<Region>,
    pub settings: Settings,
}
