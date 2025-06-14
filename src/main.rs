mod image;
mod music;

use std::{
    collections::HashMap,
    env,
    fs::File,
    io::{BufReader, Write},
    ops::Deref,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, RwLock,
    },
};

use dioxus::desktop::use_window;
use dioxus::prelude::*;
use music::{play_audio, MusicLibrary, Track};
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};

const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/main.css");

fn main() {
    tracing_subscriber::fmt::init();
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    use_window().window.set_always_on_top(false);

    let app_manager = AppManager::default();
    let app_handle = app_manager.tx();
    app_manager.handle_messages();

    let _sound = use_coroutine(|_rx: UnboundedReceiver<()>| async move {
        if let Err(error) = play_audio(_rx).await {
            error!("failed to initialize audio: {error}");
        }
    });

    let mut file = use_signal(String::new);

    let mut tracks_signal = use_signal(Vec::<Arc<RwLock<Track>>>::new);

    let (temptx, temprx) = channel();
    app_handle.send(AppMessage::GetLibraryPath(temptx))?;
    let music_library = if let Ok(temp) = temprx.recv() {
        match temp {
            Some(path) => {
                info!("searching for music library at {path}");
                let lib = MusicLibrary::new_from_path(path.deref()).ok();
                if let Some(lib) = &lib {
                    tracks_signal.set(lib.get_tracks());
                }
                lib
            }
            _ => None,
        }
    } else {
        None
    };
    let mut music_library = use_signal(move || {
        info!("found {music_library:?}");
        music_library
    });
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
                        let res = app_handle.send(AppMessage::SetLibraryPath(directory_path.clone()));
                        if res.is_err() {
                            error!("app handle dropped");
                        }

                        if let Ok(library) = MusicLibrary::new_from_path(&directory_path) {
                            tracks_signal.set(library.get_tracks());
                            music_library.set(Some(library));
                        } else {
                            warn!("invalid path")
                        }

                        file.set(directory_path);
                    }
                }
            }
        }

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
            let self_name = &self.name;
            info!("add {name} to {self_name}");
            if path_components.len() >= 2 {
                let folder;
                if let Some(temp) = path_components.pop() {
                    folder = temp;
                } else {
                    unreachable!()
                }

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

    for temp in tracks() {
        let track;
        if let Ok(temp) = temp.read() {
            track = temp;
        } else {
            continue;
        }
        let track = track.deref();

        let path_components: Vec<&str> = track.path().split("/").collect();
        let path_components = path_components.into_iter().rev().collect();
        folder.add_to_dir(path_components, track.name().to_string());
    }

    info!("{folder:?}");

    let rsx = folder.to_rsx();

    rsx! {
        div {
            class: "file-tree",
            {rsx}
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct AppConfig {
    pub path_to_music_library: Option<Arc<str>>,
}

impl AppConfig {
    fn from_conf() -> Self {
        let home: String;
        if let Some(path) = env::home_dir() {
            home = path.to_string_lossy().into();
        } else {
            return AppConfig::default();
        }

        let path = format!("{home}/.config/segue-attacca.json");

        let file = match File::open(path) {
            Ok(temp) => temp,
            Err(err) => {
                warn!("couldn't open json file: {err}");
                return AppConfig::default();
            }
        };
        let reader = BufReader::new(file);

        let conf = serde_json::from_reader(reader).unwrap_or_default();
        info!("your apps config is: {conf:?}");
        conf
    }

    fn update_conf_file(&mut self) {
        let home: String;
        if let Some(path) = env::home_dir() {
            home = path.to_string_lossy().into();
        } else {
            return;
        }
        if let Ok(json) = serde_json::to_vec_pretty(self) {
            if let Ok(mut file) = File::create(format!("{home}/.config/segue-attacca.json")) {
                let _ = file.write_all(json.as_ref());
            }
        }
    }
}

struct AppManager {
    rx: Receiver<AppMessage>,
    tx: Sender<AppMessage>,

    conf: AppConfig,
}

impl AppManager {
    fn tx(&self) -> Sender<AppMessage> {
        self.tx.clone()
    }

    fn handle_messages(mut self) {
        rayon::spawn(move || {
            while let Ok(message) = self.rx.recv() {
                info!("message {message:?}");
                match message {
                    AppMessage::GetLibraryPath(sender) => {
                        let path = self.conf.path_to_music_library.clone();
                        if let Err(e) = sender.send(path) {
                            error!("Send error from AppMessage {e}");
                        }
                    }
                    AppMessage::SetLibraryPath(path) => {
                        self.conf.path_to_music_library = Some(path.into());
                        self.conf.update_conf_file();
                    }
                    AppMessage::Quit => break,
                }
            }
        });
    }
}

impl Default for AppManager {
    fn default() -> Self {
        let (tx, rx) = channel();
        let conf = AppConfig::from_conf();
        Self { rx, tx, conf }
    }
}

#[derive(Debug)]
pub enum AppMessage {
    GetLibraryPath(Sender<Option<Arc<str>>>),
    SetLibraryPath(String),
    Quit,
}
