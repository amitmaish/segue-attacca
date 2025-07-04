use std::{
    collections::HashSet,
    fs::{DirEntry, File, read_dir},
    hash::{Hash, RandomState},
    io::{BufReader, Write},
    path::Path,
    sync::{Arc, RwLock, Weak},
};

use color_eyre::Result;
use rayon::prelude::*;
use scc::HashMap;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use uuid::Uuid;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct MusicLibrary {
    pub path: Box<str>,
    tracks: Vec<Arc<RwLock<Track>>>,
    playlists: Vec<Arc<RwLock<Playlist>>>,
    artists: Vec<Arc<str>>,
    pub tags: Vec<Arc<str>>,
}

impl MusicLibrary {
    pub fn new_from_path(path: &str) -> Result<MusicLibrary> {
        let mut lib = MusicLibrary {
            path: path.into(),
            tracks: Vec::new(),
            playlists: Vec::new(),
            artists: Vec::new(),
            tags: Vec::new(),
        };

        let dir = read_dir(path)?;
        let prefix = Path::new(path);

        let mut read_queue: Vec<DirEntry> = dir.flatten().collect();

        match File::open(format!("{path}/music_library.json")) {
            Ok(file) => {
                info!("opened music_library.json");
                let reader = BufReader::new(file);

                match serde_json::from_reader(reader) {
                    Ok(library) => {
                        info!("successfully parsed json file");
                        lib = library;
                        if lib.path != path.into() {
                            lib.path = path.into();
                        }
                    }
                    Err(e) => warn!("couldn't parse json file: {e}"),
                }
            }
            Err(e) => {
                warn!("couldn't open json file {e}");
            }
        }

        let mut visited_track_paths: HashSet<Box<str>> =
            HashSet::from_par_iter(lib.tracks.clone().into_par_iter().map(|track| {
                if let Ok(read) = track.read() {
                    read.path.clone()
                } else {
                    "".into()
                }
            }));

        while let Some(item) = read_queue.pop() {
            let file_type;
            if let Ok(filetype) = item.file_type() {
                file_type = filetype;
            } else {
                continue;
            }
            if file_type.is_file() {
                let file_name = &item.file_name();

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
                if extension == "wav" || extension == "mp3" || extension == "flac" {
                    let item_full_path = item.path();
                    let item_path;
                    if let Ok(no_prefix) = item_full_path.strip_prefix(prefix) {
                        item_path = no_prefix;
                    } else {
                        unreachable!();
                    }
                    let path: Box<str>;
                    if let Some(temp) = item_path.to_str() {
                        path = temp.into();
                    } else {
                        continue;
                    }

                    if !visited_track_paths.contains(&path) {
                        let track = Arc::new(RwLock::new(Track {
                            path: path.clone(),
                            name,
                            ..Default::default()
                        }));

                        visited_track_paths.insert(path);

                        lib.tracks.push(Arc::clone(&track));
                    }
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

        let artists = scc::HashMap::with_hasher(RandomState::new());
        let tags = scc::HashMap::with_hasher(RandomState::new());
        let tracks = scc::HashMap::with_hasher(RandomState::new());
        let playlists = scc::HashMap::with_hasher(RandomState::new());

        lib.tracks.clone().par_iter().for_each(|track_lock| {
            if let Ok(mut track) = track_lock.write() {
                let _ = tracks.insert(track.path.to_string(), Arc::clone(track_lock));

                let artist = track.artist.clone();
                if let Some(artist) = artist {
                    let artist_key = artist.to_string();
                    if artists
                        .insert(artist_key.clone(), Arc::clone(&artist))
                        .is_err()
                    {
                        track.artist = artists.read(&artist_key, |_, v| v.clone());
                    }
                }
                let tags_dedup = Vec::from_par_iter(track.tags.clone().par_iter().map(|tag| {
                    if tags.insert(tag.to_string(), Arc::clone(tag)).is_err() {
                        let tag_key = tag.to_string();
                        if let Some(tag_dedup) = tags.read(&tag_key, |_k, v| Arc::clone(v)) {
                            tag_dedup
                        } else {
                            unreachable!()
                        }
                    } else {
                        Arc::clone(tag)
                    }
                }));
                track.tags = tags_dedup;
            }
        });
        lib.playlists.par_iter().for_each(|playlist_lock| {
            if let Ok(playlist) = playlist_lock.read() {
                if playlists
                    .insert(playlist.uuid, Arc::clone(playlist_lock))
                    .is_err()
                {
                    unreachable!()
                }
            }
        });
        lib.playlists.par_iter().for_each(|playlist_lock| {
            if let Ok(mut playlist) = playlist_lock.write() {
                fn dedup_item(
                    item: &PlaylistItem,
                    tracks: &HashMap<String, Arc<RwLock<Track>>>,
                    playlists: &HashMap<Uuid, Arc<RwLock<Playlist>>>,
                ) -> Option<PlaylistItem> {
                    match item {
                        PlaylistItem::Track(rw_lock) => {
                            if let Ok(track) = rw_lock.read() {
                                tracks
                                    .read(&track.path.to_string(), |_, track| Arc::clone(track))
                                    .map(PlaylistItem::Track)
                            } else {
                                unreachable!()
                            }
                        }
                        PlaylistItem::Playlist(weak) => {
                            if let Some(playlist_lock) = weak.upgrade() {
                                if let Ok(playlist_read) = playlist_lock.read() {
                                    playlists
                                        .read(&playlist_read.uuid, |_, v| Arc::downgrade(v))
                                        .map(PlaylistItem::Playlist)
                                } else {
                                    unreachable!()
                                }
                            } else {
                                None
                            }
                        }
                        PlaylistItem::Block(playlist_items) => {
                            let items_dedup = playlist_items
                                .par_iter()
                                .filter_map(|item| dedup_item(item, tracks, playlists))
                                .collect();
                            Some(PlaylistItem::Block(items_dedup))
                        }
                    }
                }

                playlist.items = playlist
                    .items
                    .par_iter()
                    .filter_map(|item| dedup_item(item, &tracks, &playlists))
                    .collect();
            }
        });

        let mut temp = Vec::new();
        artists.scan(|_k, v| {
            temp.push(Arc::clone(v));
        });
        lib.artists = temp;

        let mut temp = Vec::new();
        tags.scan(|_k, v| {
            temp.push(Arc::clone(v));
        });
        lib.tags = temp;

        Ok(lib)
    }

    pub fn get_tracks(&self) -> &[Arc<RwLock<Track>>] {
        &self.tracks
    }

    pub fn add_tag(&mut self, track: &Arc<RwLock<Track>>, tag: &str) {
        let known_tag = self
            .tags
            .iter()
            .find(|known_tag| known_tag.as_ref() == tag)
            .cloned();

        if let Ok(mut track) = track.write() {
            if let Some(tag) = known_tag {
                track.tags.push(tag);
            } else {
                let tag: Arc<str> = tag.into();
                self.tags.push(Arc::clone(&tag));
                track.tags.push(Arc::clone(&tag));
            }
        }
        self.tags.sort_by_key(|tag| tag.to_lowercase());
    }

    pub fn gc_tags(&mut self) {
        let temp: Vec<(usize, Arc<str>)> = self
            .tags
            .iter()
            .enumerate()
            .filter(|(_, tag)| Arc::strong_count(tag) > 1)
            .map(|(i, tag)| (i, Arc::clone(tag)))
            .collect();
        for (i, _) in temp {
            if i < self.tags.len() {
                continue;
            }
            self.tags.remove(i);
        }

        let mut indecies = Vec::new();
        for (i, tag) in self.tags.iter().enumerate() {
            if Arc::strong_count(tag) > 1 {
                continue;
            } else {
                indecies.push(i);
            }
        }
        for i in indecies {
            self.tags.remove(i);
        }
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

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Track {
    pub path: Box<str>,
    pub name: Box<str>,
    pub artist: Option<Arc<str>>,
    pub album_art: Option<String>,
    pub tags: Vec<Arc<str>>,
}

impl Track {
    pub fn add_tag(&mut self, tag: &str) {
        self.tags.push(tag.into());
    }
}

impl Hash for Track {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.path.hash(state);
        self.name.hash(state);
        self.artist
            .as_ref()
            .map(|string| string.as_ref())
            .hash(state);
        self.album_art.hash(state);
        let mut tags: Vec<&str> = self.tags.iter().map(|tag| tag.as_ref()).collect();
        tags.sort_by_key(|t| t.to_lowercase());
        tags.hash(state);
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Playlist {
    name: Box<str>,
    items: Vec<PlaylistItem>,

    uuid: Uuid,
}

impl Default for Playlist {
    fn default() -> Self {
        Self {
            name: Default::default(),
            items: Default::default(),
            uuid: Uuid::new_v4(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum PlaylistItem {
    Track(Arc<RwLock<Track>>),
    Playlist(Weak<RwLock<Playlist>>),
    Block(Vec<PlaylistItem>),
}
