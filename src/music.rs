use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    fs::{read_dir, DirEntry, File},
    io::{self, BufReader, Write},
    path::Path,
    sync::{Arc, RwLock, Weak},
};

use dioxus::prelude::*;
use futures_util::StreamExt;
use rand::{rng, seq::SliceRandom};
use rodio::{Decoder, OutputStream, Sink};
use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;
use thiserror::Error;
use tracing::{info, warn};

#[derive(Error, Debug)]
pub enum MusicLibraryError {
    #[error("couldn't open {0}")]
    IOError(#[from] io::Error),
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct MusicLibrary {
    path: Box<str>,
    tracks: Vec<Arc<RwLock<Track>>>,
    playlists: Vec<Arc<RwLock<Playlist>>>,
    tags: HashMap<Arc<str>, Vec<Weak<RwLock<Track>>>>,
}

impl MusicLibrary {
    pub fn new_from_path(path: &str) -> Result<MusicLibrary, MusicLibraryError> {
        let mut lib = MusicLibrary::default();
        lib.path = path.into();

        let dir = read_dir(path)?;
        let prefix = Path::new(path);

        let mut read_queue: Vec<DirEntry> = dir.flatten().collect();

        while let Some(item) = read_queue.pop() {
            let file_type;
            if let Ok(filetype) = item.file_type() {
                file_type = filetype;
            } else {
                continue;
            }
            if file_type.is_file() {
                let file_name = &item.file_name();

                if file_name == OsStr::new("music_library.json") {
                    let filename;
                    if let Some(temp) = file_name.to_str() {
                        filename = temp;
                    } else {
                        continue;
                    }

                    let file = match File::open(format!("{path}/{filename}")) {
                        Ok(temp) => temp,
                        Err(err) => {
                            warn!("couldn't open json file {err}");
                            continue;
                        }
                    };
                    let reader = BufReader::new(file);

                    if let Ok(library) = serde_json::from_reader(reader) {
                        lib = library;
                        if lib.path != path.into() {
                            lib.path = path.into();
                        }

                        break;
                    }
                }

                let extension;
                if let Some(temp) = Path::new(file_name).extension() {
                    extension = temp;
                } else {
                    continue;
                }

                let name;
                if let Some(temp) = file_name.to_str() {
                    name = temp.into()
                } else {
                    continue;
                }
                if extension == "wav" || extension == "mp3" {
                    let item_full_path = item.path();
                    let item_path;
                    if let Ok(no_prefix) = item_full_path.strip_prefix(prefix) {
                        item_path = no_prefix;
                    } else {
                        unreachable!();
                    }
                    info!("{item_full_path:?}");
                    let path: Box<str>;
                    if let Some(temp) = item_path.to_str() {
                        path = temp.into();
                    } else {
                        continue;
                    }

                    let track = Arc::new(RwLock::new(Track {
                        path: path.clone(),
                        name,
                        ..Default::default()
                    }));

                    lib.tracks.push(Arc::clone(&track));
                }
            } else if file_type.is_dir() {
                let dir;
                if let Ok(temp) = read_dir(item.path()) {
                    dir = temp;
                } else {
                    continue;
                }
                dir.flatten().for_each(|item| {
                    read_queue.push(item);
                });
            }
        }

        Ok(lib)
    }

    pub fn get_tracks_signal(&self) -> Vec<Arc<RwLock<Track>>> {
        self.tracks.clone()
    }
}

impl Drop for MusicLibrary {
    fn drop(&mut self) {
        if let Ok(json) = serde_json::to_vec_pretty(self) {
            let path = &self.path;
            if let Ok(mut file) = File::create(format!("{path}/music_library.json")) {
                let _ = file.write_all(json.as_ref());
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Track {
    path: Box<str>,
    name: Box<str>,
    artist: Option<Arc<str>>,
    // features: Option<Vec<Arc<str>>>,
    album_art: Option<String>,
    tags: HashSet<Arc<str>>,
}

impl Track {
    pub fn _add_tag(&mut self, tag: Arc<str>) {
        self.tags.insert(tag);
    }

    pub fn _has_tag(&self, tag: Arc<str>) -> bool {
        self.tags.contains(&tag)
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn _tags(&self) -> &HashSet<Arc<str>> {
        &self.tags
    }
}

#[derive(Serialize, Deserialize, Debug, SmartDefault, Clone)]
pub struct Playlist {
    pub name: Option<String>,
    pub items: Vec<PlaylistItem>,
    playback_mode: PlaybackMode,

    #[serde(skip)]
    play_queue: Vec<PlaylistItem>,
    #[serde(skip)]
    #[default(true)]
    first: bool,
}

#[derive(Serialize, Deserialize, Debug, SmartDefault, Clone)]
pub enum PlaylistItem {
    #[default]
    Track(Arc<RwLock<Track>>),
    Playlist(Arc<RwLock<Playlist>>),
}

/// sets the order that a playlist will play back its items
#[derive(Serialize, Deserialize, Debug, Default, Clone, Copy)]
pub enum PlaybackMode {
    /// the playlist will play all the tracks in order before ending
    #[default]
    Continuous,
    /// the playlist will play all the tracks in a random order before ending
    Shuffle,
    /// the playlist will play all the tracks in order and then repeat until manually ended
    LoopContinuous,
    /// the playlist will play all the tracks in a random order and then re-randomize the order and
    /// then repeat until manually ended
    LoopShuffle,
}

impl Iterator for Playlist {
    type Item = PlaylistItem;

    fn next(&mut self) -> Option<PlaylistItem> {
        if self.items.is_empty() {
            return None;
        }

        if self.first {
            self.first = false;
            match self.playback_mode {
                PlaybackMode::Shuffle | PlaybackMode::LoopShuffle => {
                    self.play_queue = self.items.to_vec();
                    self.play_queue.shuffle(&mut rng());
                }
                PlaybackMode::Continuous | PlaybackMode::LoopContinuous => {
                    self.play_queue = self.items.iter().cloned().rev().collect();
                }
            }
        }

        let next = self.play_queue.pop();

        if next.is_some() {
            return next;
        }

        match self.playback_mode {
            PlaybackMode::LoopContinuous | PlaybackMode::LoopShuffle => {
                self.first = true;
                self.next()
            }
            PlaybackMode::Continuous | PlaybackMode::Shuffle => None,
        }
    }
}

#[derive(Error, Debug)]
pub enum AudioError {
    #[error("couldn't initialize default audio source")]
    Stream(#[from] rodio::StreamError),
    #[error("couldn't create audio sink")]
    Play(#[from] rodio::PlayError),
    #[error("couldn't decode audio file")]
    Decode(#[from] rodio::decoder::DecoderError),

    #[error("couldn't open file")]
    IO(#[from] io::Error),
}

pub async fn play_audio(mut rx: UnboundedReceiver<()>) -> Result<(), AudioError> {
    let (_stream, stream_handle) = OutputStream::try_default()?;
    let sink = Sink::try_new(&stream_handle)?;

    let file = BufReader::new(File::open("assets/honey.wav")?);
    let source = Decoder::new(file)?;

    sink.append(source);

    let file = BufReader::new(File::open("assets/silver_lullaby.wav")?);
    let source = Decoder::new(file)?;

    sink.append(source);

    while let Some(_message) = rx.next().await {}

    Ok(())
}
