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
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

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
        // let playlists = scc::HashMap::with_hasher(RandomState::new());

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
        // lib.playlists
        //     .clone()
        //     .par_iter()
        //     .for_each(move |playlist_lock| {
        //         if let Ok(mut playlist) = playlist_lock.write() {
        //             let playlist_items_dedup =
        //                 Vec::from_par_iter(playlist.items.par_iter().map(|item| match item {
        //                     PlaylistItem::Track(rw_lock) => todo!(),
        //                     PlaylistItem::Playlist(weak) => todo!(),
        //                 }));
        //             playlist.items = playlist_items_dedup;
        //         }
        //     });

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

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Playlist {
    name: Option<Box<str>>,
    items: Vec<PlaylistItem>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PlaylistItem {
    Track(Arc<RwLock<Track>>),
    Playlist(Weak<RwLock<Playlist>>),
}
