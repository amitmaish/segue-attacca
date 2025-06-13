mod image;
mod music;

use std::{
    collections::HashMap,
    ops::Deref,
    sync::{Arc, RwLock},
};

use dioxus::desktop::use_window;
use dioxus::prelude::*;
use music::{play_audio, MusicLibrary, Track};
use tracing::info;

const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/main.css");

fn main() {
    tracing_subscriber::fmt::init();
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    use_window().window.set_always_on_top(false);

    let _sound = use_coroutine(play_audio);

    let mut file = use_signal(String::new);

    let mut music_library = use_signal(MusicLibrary::default);
    let mut tracks_signal = use_signal(Vec::<Arc<RwLock<Track>>>::new);
    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: MAIN_CSS }

        input {
            r#type: "file",
            directory: true,

            onchange: move |evt| {
                if let Some(file_engine) = &evt.files() {
                    let files = file_engine.files();
                    for directory_path in files {

                        let library = MusicLibrary::new_from_path(&directory_path);
                        tracks_signal.set(library.get_tracks_signal());
                        music_library.set(MusicLibrary::new_from_path(&directory_path));

                        file.set(directory_path);
                    }
                }
            }
        }
        // img { src: image_to_url(r#"/Users/amit/Desktop/shuffle/Zhea Erose - Dreamsura/cover.jpg"#)? }

        track_file_tree {
            name: "root",
            tracks: tracks_signal,
        }
    }
}

#[component]
pub fn track_file_tree(name: String, tracks: Signal<Vec<Arc<RwLock<Track>>>>) -> Element {
    #[derive(Debug, Default)]
    struct Folder {
        name: String,
        folders: HashMap<String, Folder>,
        files: Vec<String>,
    }

    impl Folder {
        fn add_to_dir(&mut self, mut path_components: Vec<&str>, name: String) {
            if path_components.len() >= 2 {
                let folder = path_components.pop().unwrap();

                if let Some(directory) = self.folders.get_mut(folder) {
                    directory.add_to_dir(path_components, name);
                } else {
                    info!("new folder: {folder}");
                    let mut directory = Folder {
                        name: folder.into(),
                        ..Default::default()
                    };
                    directory.add_to_dir(path_components, name);
                    self.folders.insert(folder.into(), directory);
                }
            } else {
                self.files.push(name);
            }
        }

        fn to_rsx(&self) -> Element {
            let folders = self.folders.iter().map(|f| f.1.to_rsx());
            rsx! {
                details {
                    summary {
                        "{self.name}"
                    }
                    ul { {folders} }
                    ul { for file in self.files.clone() {
                            li { key: "{file}", "{file}" }
                        }
                    }
                }
            }
        }
    }

    let mut folder = Folder {
        name,
        ..Default::default()
    };

    for track in tracks() {
        let track = track.read().unwrap();
        let track = track.deref();

        let path_components: Vec<&str> = track.path().split("/").collect();
        let path_components = path_components.into_iter().rev().collect();
        folder.add_to_dir(path_components, track.name().to_string());
    }

    info!("{folder:?}");

    folder.to_rsx()
}
