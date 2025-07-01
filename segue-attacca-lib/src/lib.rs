use music_library::MusicLibrary;
use playback::PlaybackEngine;

pub mod music_library;
mod playback;

pub struct AppState {
    _library: MusicLibrary,

    _playback_engine: PlaybackEngine,
}

pub enum AppMessage {}

