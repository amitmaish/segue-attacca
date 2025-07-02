use std::{
    collections::HashMap,
    ffi::OsStr,
    fs::{DirEntry, File, read_dir},
    io::{BufReader, Write},
    path::Path,
    rc::Rc,
    sync::RwLock,
};

use color_eyre::Result;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct MusicLibrary {
    pub path: Box<str>,
    tracks: Vec<Rc<RwLock<Track>>>,
    playlists: Vec<Rc<RwLock<Playlist>>>,
    artists: Vec<Rc<str>>,
    pub tags: Vec<Rc<str>>,
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
                    info!("found music_library.json");
                    let filename;
                    if let Some(temp) = file_name.to_str() {
                        filename = temp;
                    } else {
                        continue;
                    }

                    let file = match File::open(format!("{path}/{filename}")) {
                        Ok(temp) => {
                            info!("opened json file");
                            temp
                        }
                        Err(err) => {
                            warn!("couldn't open json file {err}");
                            continue;
                        }
                    };
                    let reader = BufReader::new(file);

                    match serde_json::from_reader(reader) {
                        Ok(library) => {
                            info!("successfully parsed json file");
                            lib = library;
                            if lib.path != path.into() {
                                lib.path = path.into();
                            }
                            break;
                        }
                        Err(e) => warn!("couldn't parse json file: {e}"),
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
                    let path: Box<str>;
                    if let Some(temp) = item_path.to_str() {
                        path = temp.into();
                    } else {
                        continue;
                    }

                    let track = Rc::new(RwLock::new(Track {
                        path: path.clone(),
                        name,
                        ..Default::default()
                    }));

                    lib.tracks.push(Rc::clone(&track));
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

        let mut artists = HashMap::<String, Rc<str>>::new();
        let mut tags = HashMap::<String, Rc<str>>::new();
        let mut tracks = HashMap::<String, Rc<RwLock<Track>>>::new();
        for track_lock in lib.tracks.clone() {
            if let Ok(mut track) = track_lock.write() {
                tracks.insert(track.path.to_string(), Rc::clone(&track_lock));
                if let Some(artist) = track.artist.as_ref() {
                    if let Some(artist_dedupe) = artists.get(artist.as_ref()) {
                        track.artist = Some(Rc::clone(artist_dedupe));
                    } else {
                        artists.insert(artist.as_ref().to_string(), Rc::clone(artist));
                    }
                }
                let mut tags_dedup = Vec::new();
                for tag in track.tags.clone() {
                    if let Some(tag_dedup) = tags.get(tag.as_ref()) {
                        tags_dedup.push(Rc::clone(tag_dedup));
                    } else {
                        tags.insert(tag.as_ref().to_string(), Rc::clone(&tag));
                        tags_dedup.push(Rc::clone(&tag));
                    }
                }
                track.tags = tags_dedup;
            }
        }

        // TODO deduplicate playlists. maybe only deduplicate playlists with names and treat other
        // playlists as inline playlists?

        lib.artists = artists.values().cloned().collect();
        lib.tags = tags.values().cloned().collect();

        Ok(lib)
    }

    pub fn get_tracks(&self) -> &[Rc<RwLock<Track>>] {
        &self.tracks
    }

    pub fn add_tag(&mut self, track: &Rc<RwLock<Track>>, tag: &str) {
        let known_tag = self
            .tags
            .iter()
            .find(|known_tag| known_tag.as_ref() == tag)
            .cloned();

        if let Ok(mut track) = track.write() {
            if let Some(tag) = known_tag {
                track.tags.push(tag);
            } else {
                let tag: Rc<str> = tag.into();
                self.tags.push(Rc::clone(&tag));
                track.tags.push(Rc::clone(&tag));
            }
        }
        self.tags.sort_by_key(|tag| tag.to_lowercase());
    }

    pub fn gc_tags(&mut self) {
        let temp: Vec<(usize, Rc<str>)> = self
            .tags
            .iter()
            .enumerate()
            .filter(|(_, tag)| Rc::strong_count(tag) > 1)
            .map(|(i, tag)| (i, Rc::clone(tag)))
            .collect();
        for (i, _) in temp {
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
    pub artist: Option<Rc<str>>,
    pub album_art: Option<String>,
    pub tags: Vec<Rc<str>>,
}

impl Track {
    pub fn add_tag(&mut self, tag: &str) {
        self.tags.push(tag.into());
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Playlist {
    name: Option<Box<str>>,
    items: Vec<PlaylistItem>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PlaylistItem {
    Track(Rc<RwLock<Track>>),
    Playlist(Rc<RwLock<Playlist>>),
}
