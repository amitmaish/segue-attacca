use std::{
    collections::HashMap,
    fmt::Display,
    ops::Deref,
    rc::{Rc, Weak},
    sync::RwLock,
    thread,
};

use color_eyre::Result;
use ratatui::{
    DefaultTerminal, Frame,
    crossterm::event::{self, Event as CE},
    layout::{Constraint, Layout},
    prelude::Text,
    style::{Color, Style, Stylize},
    widgets::{Block, BorderType, List, ListState, Paragraph, StatefulWidget, Widget, Wrap},
};
use ratatui_image::{StatefulImage, picker::Picker, protocol::StatefulProtocol};
use segue_attacca_lib::music_library::{MusicLibrary, Track};
use strum::Display;
use tokio::sync::{
    mpsc::{Receiver, Sender, channel},
    oneshot,
};

const SELECTED_COLOR: Color = Color::LightMagenta;
const DEFAULT_COLOR: Color = Color::LightBlue;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    color_eyre::install()?;

    let mut state = AppState::new();

    let terminal = ratatui::init();
    let result = run(terminal, &mut state).await;
    ratatui::restore();
    result
}

async fn run(mut terminal: DefaultTerminal, state: &mut AppState) -> Result<()> {
    state.list = state
        .library
        .get_tracks()
        .iter()
        .map(|track| TrackInspector::new(Rc::downgrade(track)))
        .collect();

    let tx = state.event_tx.clone();
    thread::spawn(move || handle_terminal_events(tx));

    loop {
        terminal.draw(|f| render(f, state))?;
        if let Some(event) = state.event_rx.recv().await {
            match event {
                Event::KeyPressed(key_code) => match key_code {
                    KeyCode::Escape | KeyCode::Char('q') => break Ok(()),
                    KeyCode::Char(c) => match c {
                        'j' => {
                            state.list_state.select_next();
                            if let Some(track) = state
                                .library
                                .get_tracks()
                                .get(state.list_state().selected().unwrap_or(0))
                            {
                                state.selected_track = Some(Rc::clone(track));
                            } else {
                                state.selected_track = None;
                            }
                        }
                        'k' => {
                            state.list_state.select_previous();
                            if let Some(track) = state
                                .library
                                .get_tracks()
                                .get(state.list_state().selected().unwrap_or(0))
                            {
                                state.selected_track = Some(Rc::clone(track));
                            } else {
                                state.selected_track = None;
                            }
                        }
                        '1' => state.selected_panel = SelectedPanel::TrackList,
                        '2' => state.selected_panel = SelectedPanel::Inspector,

                        _ => continue,
                    },
                },
                Event::Redraw => continue,
            }
        }
    }
}

fn handle_terminal_events(tx: Sender<Event>) -> Result<()> {
    loop {
        match event::read()? {
            CE::FocusGained => (),
            CE::FocusLost => (),
            CE::Key(key_event) => match key_event.code {
                event::KeyCode::Backspace => (),
                event::KeyCode::Enter => (),
                event::KeyCode::Left => (),
                event::KeyCode::Right => (),
                event::KeyCode::Up => (),
                event::KeyCode::Down => (),
                event::KeyCode::Home => (),
                event::KeyCode::End => (),
                event::KeyCode::PageUp => (),
                event::KeyCode::PageDown => (),
                event::KeyCode::Tab => (),
                event::KeyCode::BackTab => (),
                event::KeyCode::Delete => (),
                event::KeyCode::Insert => (),
                event::KeyCode::F(_) => (),
                event::KeyCode::Char(c) => {
                    if (tx.blocking_send(Event::KeyPressed(KeyCode::Char(c)))).is_ok()
                        && tx.blocking_send(Event::Redraw).is_ok()
                    {
                        continue;
                    }
                    break Ok(());
                }
                event::KeyCode::Null => (),
                event::KeyCode::Esc => {
                    if (tx.blocking_send(Event::KeyPressed(KeyCode::Escape))).is_ok()
                        && tx.blocking_send(Event::Redraw).is_ok()
                    {
                        continue;
                    }
                    break Ok(());
                }
                event::KeyCode::CapsLock => (),
                event::KeyCode::ScrollLock => (),
                event::KeyCode::NumLock => (),
                event::KeyCode::PrintScreen => (),
                event::KeyCode::Pause => (),
                event::KeyCode::Menu => (),
                event::KeyCode::KeypadBegin => (),
                event::KeyCode::Media(_media_key_code) => (),
                event::KeyCode::Modifier(_modifier_key_code) => (),
            },
            CE::Mouse(_mouse_event) => (),
            CE::Paste(_) => (),
            CE::Resize(_, _) => (),
        }
    }
}

fn render(frame: &mut Frame, state: &mut AppState) {
    let layout = Layout::horizontal([Constraint::Fill(3), Constraint::Fill(1)]);
    let [list_area, inspector_area] = layout.areas(frame.area());

    let mut list = List::new(state.list.deref())
        .block(
            Block::bordered()
                .title(" [1] segue attacca ")
                .border_type(BorderType::Rounded),
        )
        .fg(DEFAULT_COLOR)
        .highlight_style(Style::new().reversed())
        .repeat_highlight_symbol(true);
    let mut inspector = Block::bordered()
        .title(" [2] inspector ")
        .border_type(BorderType::Rounded)
        .fg(DEFAULT_COLOR);

    match state.selected_panel {
        SelectedPanel::TrackList => list = list.fg(SELECTED_COLOR),
        SelectedPanel::Inspector => inspector = inspector.fg(SELECTED_COLOR),
    }

    let inspector_inner = inspector.inner(inspector_area);

    frame.render_stateful_widget(list, list_area, state.list_state_mut());
    frame.render_widget(inspector, inspector_area);

    if let Some(track) = state.selected_track.as_ref() {
        let track_inspector = TrackInspector::new(Rc::downgrade(track));
        frame.render_stateful_widget(track_inspector, inspector_inner, state);
    } else {
        frame.render_widget(
            Block::bordered()
                .title(" [2] inspector ")
                .border_type(BorderType::Rounded),
            inspector_area,
        );
    }
}

#[derive(Default)]
struct TrackInspector {
    pub track: Weak<RwLock<Track>>,
}

impl TrackInspector {
    fn new(track: Weak<RwLock<Track>>) -> Self {
        Self { track }
    }
}

impl Display for TrackInspector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(lock) = self.track.upgrade() {
            if let Ok(track) = lock.read() {
                write!(f, "{}", track.name)
            } else {
                write!(f, "error")
            }
        } else {
            write!(f, "error")
        }
    }
}

impl<'a> From<&TrackInspector> for Text<'a> {
    fn from(value: &TrackInspector) -> Self {
        Text::from(format!("{value}"))
    }
}

impl StatefulWidget for TrackInspector {
    type State = AppState;
    fn render(
        self,
        area: ratatui::prelude::Rect,
        buf: &mut ratatui::prelude::Buffer,
        state: &mut AppState,
    ) {
        let width = area.width;

        let (name, artist, path, _tags);
        let art = if let Some(lock) = self.track.upgrade() {
            let track = lock
                .read()
                .expect("couldn't read track in track inspector's render method");
            name = track.name.clone();
            artist = track.artist.clone();
            path = track.path.clone();
            _tags = track.tags.clone();
            let art_path = track.album_art.as_ref();

            if let Some(path) = art_path {
                if let Some(asset) = state.images.get_mut(path) {
                    match asset {
                        Asset::Some(_) | Asset::LoadError(_) | Asset::None => Some(asset),
                        Asset::Loading(receiver) => {
                            if let Ok(image) = receiver.try_recv() {
                                *asset = Asset::Some(image);
                            }
                            Some(asset)
                        }
                        Asset::Unloaded => {
                            let (tx, rx) = oneshot::channel();
                            *asset = Asset::Loading(rx);

                            let path = path.clone();
                            let picker = state.picker.clone();
                            let state_tx = state.event_tx.clone();
                            tokio::spawn(async move {
                                let _ = state_tx.send(Event::Redraw).await;
                                let buffer = tokio::fs::read(path)
                                    .await
                                    .expect("couldn't open image at {path}");
                                let image = image::load_from_memory(&buffer)
                                    .expect("couldn't load image from memory");
                                let protocol = tokio::task::spawn_blocking(move || {
                                    picker.new_resize_protocol(image)
                                })
                                .await
                                .expect("join error");
                                let _ = tx.send(protocol);
                                let _ = state_tx.send(Event::Redraw).await;
                            });

                            Some(asset)
                        }
                    }
                } else {
                    state.images.insert(path.clone(), Asset::Unloaded);
                    Some(
                        state
                            .images
                            .get_mut(path)
                            .expect("i'm pretty sure this is impossible"),
                    )
                }
            } else {
                None
            }
        } else {
            name = "".into();
            artist = None;
            path = "".into();
            None
        };

        let title: Vec<String> = textwrap::wrap(name.as_ref(), width as usize)
            .iter()
            .map(|l| l.to_string())
            .collect();
        let artist_wrapped_len = if let Some(artist_ref) = artist.clone() {
            textwrap::wrap(format!("artist: {artist_ref}").as_ref(), width as usize)
                .iter()
                .map(|l| l.to_string())
                .len()
        } else {
            1
        };
        let path_text = format!("path: {path}");
        let path_wrapped = textwrap::wrap(path_text.as_ref(), width as usize);

        use Constraint as c;
        let art_constraint = if let Some(Asset::Some(_)) = art {
            c::Length(u16::min(20, width))
        } else {
            c::Length(1)
        };
        let [title_area, art_area, artist_area, path_area] = Layout::vertical([
            c::Length(title.len() as u16),
            art_constraint,
            c::Length(artist_wrapped_len as u16),
            c::Length(path_wrapped.len() as u16),
        ])
        .areas(area);

        Paragraph::new(name.to_string())
            .wrap(Wrap { trim: false })
            .render(title_area, buf);
        if let Some(artist) = artist.as_ref() {
            let artist = artist.clone();
            Paragraph::new(format!("artist: {artist}"))
                .wrap(Wrap { trim: false })
                .render(artist_area, buf);
        } else {
            Paragraph::new("artist:")
                .wrap(Wrap { trim: false })
                .render(artist_area, buf);
        }
        Paragraph::new(path_text)
            .wrap(Wrap { trim: false })
            .render(path_area, buf);

        if let Some(art) = art {
            match art {
                Asset::Some(art) => StatefulImage::default().render(art_area, buf, art),
                Asset::Loading(_) => Paragraph::new("loading").render(art_area, buf),
                Asset::Unloaded => Paragraph::new("unloaded").render(art_area, buf),
                Asset::LoadError(e) => {
                    Paragraph::new(format!("Couldn't load image: {e}")).render(art_area, buf)
                }
                Asset::None => Paragraph::new("no album art")
                    .fg(Color::Yellow)
                    .render(art_area, buf),
            }
        } else {
            Paragraph::new("no album art")
                .fg(Color::Yellow)
                .render(art_area, buf);
        }
    }
}

pub struct AppState {
    pub library: MusicLibrary,
    list: Vec<TrackInspector>,
    list_state: ListState,
    pub selected_track: Option<Rc<RwLock<Track>>>,
    pub images: HashMap<String, Asset<StatefulProtocol>>,
    pub selected_panel: SelectedPanel,

    pub picker: Picker,

    event_rx: Receiver<Event>,
    event_tx: Sender<Event>,
}

impl Default for AppState {
    fn default() -> Self {
        let (event_tx, event_rx) = channel(16);
        let picker = Picker::from_query_stdio().expect("couldn't query stdio for picker");
        Self {
            library: Default::default(),
            list: Default::default(),
            list_state: Default::default(),
            selected_track: Default::default(),
            images: Default::default(),
            selected_panel: Default::default(),
            picker,
            event_rx,
            event_tx,
        }
    }
}

impl AppState {
    pub fn new() -> Self {
        if let Ok(library) = MusicLibrary::new_from_path("/Users/amit/Desktop/segue-attacca/") {
            Self {
                library,
                ..Default::default()
            }
        } else {
            Self::default()
        }
    }

    pub fn list_state(&self) -> &ListState {
        &self.list_state
    }

    pub fn list_state_mut(&mut self) -> &mut ListState {
        &mut self.list_state
    }
}

#[derive(Default)]
pub enum SelectedPanel {
    #[default]
    TrackList,
    Inspector,
}

pub enum Event {
    KeyPressed(KeyCode),
    Redraw,
}

pub enum KeyCode {
    Char(char),
    Escape,
}

#[derive(Debug, Default, Display)]
pub enum Asset<T> {
    Some(T),
    Loading(oneshot::Receiver<T>),
    #[default]
    Unloaded,
    LoadError(Box<str>),
    None,
}
