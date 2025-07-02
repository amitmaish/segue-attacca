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
    crossterm::event::{self, Event as CE, KeyModifiers},
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
            let handled = match state.selected_panel {
                SelectedPanel::TrackList => handle_track_list_events(&event, state),
                SelectedPanel::Inspector => handle_inspector_events(&event, state),
            };
            if handled {
                continue;
            }
            match event {
                Event::KeyPressed(KeyCode::Escape, _)
                | Event::KeyPressed(KeyCode::Char('q'), _) => {
                    break Ok(());
                }

                Event::KeyPressed(KeyCode::Char(c), _) => match c {
                    '1' => state.selected_panel = SelectedPanel::TrackList,
                    '2' => state.selected_panel = SelectedPanel::Inspector,

                    _ => continue,
                },
                _ => continue,
            }
        }
    }
}

fn handle_terminal_events(tx: Sender<Event>) -> Result<()> {
    loop {
        let event = event::read()?;
        match event {
            CE::FocusGained => (),
            CE::FocusLost => (),
            CE::Key(key_event) => match key_event.code {
                event::KeyCode::Backspace => {
                    if (tx.blocking_send(Event::KeyPressed(KeyCode::Backspace, Modifiers::NONE)))
                        .is_ok()
                        && tx.blocking_send(Event::Redraw).is_ok()
                    {
                        continue;
                    }
                    break Ok(());
                }
                event::KeyCode::Enter => {
                    if (tx.blocking_send(Event::KeyPressed(KeyCode::Enter, Modifiers::NONE)))
                        .is_ok()
                        && tx.blocking_send(Event::Redraw).is_ok()
                    {
                        continue;
                    }
                    break Ok(());
                }
                event::KeyCode::Left => (),
                event::KeyCode::Right => (),
                event::KeyCode::Up => (),
                event::KeyCode::Down => (),
                event::KeyCode::Home => (),
                event::KeyCode::End => (),
                event::KeyCode::PageUp => (),
                event::KeyCode::PageDown => (),
                event::KeyCode::Tab => {
                    if (tx.blocking_send(Event::KeyPressed(KeyCode::Tab, Modifiers::NONE))).is_ok()
                        && tx.blocking_send(Event::Redraw).is_ok()
                    {
                        continue;
                    }
                    break Ok(());
                }
                event::KeyCode::BackTab => (),
                event::KeyCode::Delete => (),
                event::KeyCode::Insert => (),
                event::KeyCode::F(_) => (),
                event::KeyCode::Char(c) => {
                    let mods = key_event.modifiers;
                    let mods = Modifiers {
                        shift: mods.contains(KeyModifiers::SHIFT),
                        ctrl: mods.contains(KeyModifiers::CONTROL),
                        alt: mods.contains(KeyModifiers::ALT),
                        hyper: mods.contains(KeyModifiers::HYPER),
                    };
                    if (tx.blocking_send(Event::KeyPressed(KeyCode::Char(c), mods))).is_ok()
                        && tx.blocking_send(Event::Redraw).is_ok()
                    {
                        continue;
                    }
                    break Ok(());
                }
                event::KeyCode::Null => (),
                event::KeyCode::Esc => {
                    if (tx.blocking_send(Event::KeyPressed(KeyCode::Escape, Modifiers::NONE)))
                        .is_ok()
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

fn handle_track_list_events(event: &Event, state: &mut AppState) -> bool {
    match event {
        Event::KeyPressed(KeyCode::Char(c), _) => match c {
            'j' => {
                state.list_state.select_next();
                if let Some(track) = state
                    .library
                    .get_tracks()
                    .get(state.list_state().selected().unwrap_or(0))
                {
                    state.track_inspector = Some(TrackInspector::new(Rc::downgrade(track)));
                } else {
                    state.track_inspector = None;
                }
                true
            }
            'k' => {
                state.list_state.select_previous();
                if let Some(track) = state
                    .library
                    .get_tracks()
                    .get(state.list_state().selected().unwrap_or(0))
                {
                    state.track_inspector = Some(TrackInspector::new(Rc::downgrade(track)));
                } else {
                    state.track_inspector = None;
                }
                true
            }
            _ => false,
        },
        _ => false,
    }
}

fn handle_inspector_events(event: &Event, state: &mut AppState) -> bool {
    match event {
        Event::KeyPressed(KeyCode::Tab, modifier) => {
            if let Some(inspector) = state.track_inspector.as_mut() {
                if modifier.shift {
                    inspector.selected_field = inspector.selected_field.prev();
                } else {
                    inspector.selected_field = inspector.selected_field.next();
                }
            }
            true
        }
        Event::KeyPressed(KeyCode::Enter, _) => {
            if let Some(inspector) = state.track_inspector.as_mut() {
                if let Some(value) = inspector.editing_value.as_ref() {
                    if let Some(lock) = inspector.track.upgrade() {
                        if let Ok(mut track) = lock.write() {
                            match inspector.selected_field {
                                TrackInspectorSelectedField::None => (),
                                TrackInspectorSelectedField::Name => {
                                    track.name = value.as_str().into()
                                }
                                TrackInspectorSelectedField::Art => (),
                                TrackInspectorSelectedField::Artist => {
                                    if value.as_str() != "" {
                                        track.artist = Some(value.as_str().into());
                                    } else {
                                        track.artist = None;
                                    }
                                }
                                TrackInspectorSelectedField::Tags => (),
                            }
                        }
                    }
                    inspector.editing_value = None;
                } else {
                    inspector.editing_value = Some(String::new());
                }
            }
            true
        }
        Event::KeyPressed(KeyCode::Char(c), _) => {
            if let Some(inspector) = state.track_inspector.as_mut() {
                if let Some(value) = inspector.editing_value.as_mut() {
                    value.push(*c);
                    return true;
                }
            }
            false
        }
        Event::KeyPressed(KeyCode::Backspace, _) => {
            if let Some(inspector) = state.track_inspector.as_mut() {
                if let Some(value) = inspector.editing_value.as_mut() {
                    let _ = value.pop();
                    return true;
                }
            }
            false
        }
        _ => false,
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
        .highlight_style(Style::new().fg(Color::Green))
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

    if let Some(track_inspector) = state.track_inspector.as_ref() {
        frame.render_stateful_widget(track_inspector.clone(), inspector_inner, state);
    } else {
        frame.render_widget(
            Block::bordered()
                .title(" [2] inspector ")
                .border_type(BorderType::Rounded),
            inspector_area,
        );
    }
}

#[derive(Clone, Default)]
pub struct TrackInspector {
    pub track: Weak<RwLock<Track>>,
    pub selected_field: TrackInspectorSelectedField,

    pub editing_value: Option<String>,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum TrackInspectorSelectedField {
    #[default]
    None,
    Name,
    Art,
    Artist,
    Tags,
}

impl TrackInspectorSelectedField {
    fn next(self) -> Self {
        match self {
            Self::None => Self::Name,
            Self::Name => Self::Art,
            Self::Art => Self::Artist,
            Self::Artist => Self::Tags,
            Self::Tags => Self::None,
        }
    }

    fn prev(self) -> Self {
        match self {
            Self::None => Self::Tags,
            Self::Name => Self::None,
            Self::Art => Self::Name,
            Self::Artist => Self::Art,
            Self::Tags => Self::Artist,
        }
    }
}

impl TrackInspector {
    fn new(track: Weak<RwLock<Track>>) -> Self {
        Self {
            track,
            ..Default::default()
        }
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

        let (name, artist, path, tags);
        let art = if let Some(lock) = self.track.upgrade() {
            let track = lock
                .read()
                .expect("couldn't read track in track inspector's render method");
            name = track.name.clone();
            artist = track.artist.clone();
            path = track.path.clone();
            tags = track.tags.clone();
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
            tags = Vec::new();
            None
        };
        let tags = tags.join(", ");

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
        let tags_text = format!("tags: {tags}");
        let tags_wrapped = textwrap::wrap(&tags_text, width as usize);
        let path_text = format!("path: {path}");
        let path_wrapped = textwrap::wrap(path_text.as_ref(), width as usize);

        use Constraint as c;
        let art_constraint = if let Some(Asset::Some(_)) = art {
            u16::min(20, width)
        } else {
            1
        };
        let [
            title_area,
            art_area,
            artist_area,
            tags_area,
            path_area,
            _,
            edit_area,
        ] = Layout::vertical([
            c::Length(title.len() as u16),
            c::Length(art_constraint),
            c::Length(artist_wrapped_len as u16),
            c::Length(tags_wrapped.len() as u16),
            c::Length(path_wrapped.len() as u16),
            c::Fill(1),
            c::Length(3),
        ])
        .areas(area);

        let mut title = Paragraph::new(name.to_string()).wrap(Wrap { trim: false });
        let mut artist = if let Some(artist) = artist.as_ref() {
            let artist = artist.clone();
            Paragraph::new(format!("artist: {artist}")).wrap(Wrap { trim: false })
        } else {
            Paragraph::new("artist:").wrap(Wrap { trim: false })
        };
        let mut tags = Paragraph::new(tags_text).wrap(Wrap { trim: false });
        let path = Paragraph::new(path_text)
            .wrap(Wrap { trim: false })
            .fg(Color::Gray);

        match self.selected_field {
            TrackInspectorSelectedField::None => (),
            TrackInspectorSelectedField::Name => title = title.fg(Color::Green),
            TrackInspectorSelectedField::Art => (),
            TrackInspectorSelectedField::Artist => artist = artist.fg(Color::Green),
            TrackInspectorSelectedField::Tags => tags = tags.fg(Color::Green),
        }
        if self.selected_field != TrackInspectorSelectedField::None {
            if let Some(value) = self.editing_value {
                Paragraph::new(value)
                    .block(Block::bordered().border_type(BorderType::Rounded))
                    .fg(Color::Green)
                    .render(edit_area, buf);
            } else {
                Paragraph::new("press enter to edit value")
                    .block(Block::bordered().border_type(BorderType::Rounded))
                    .render(edit_area, buf);
            }
        }

        title.render(title_area, buf);
        artist.render(artist_area, buf);
        tags.render(tags_area, buf);
        path.render(path_area, buf);

        if let Some(art) = art {
            let art_area = if self.selected_field == TrackInspectorSelectedField::Art {
                let block = Block::bordered()
                    .border_type(BorderType::Rounded)
                    .fg(Color::Green);
                let new_area = block.inner(art_area);
                block.render(art_area, buf);
                new_area
            } else {
                art_area
            };
            match art {
                Asset::Some(art) => {
                    StatefulImage::default().render(art_area, buf, art);
                }
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
            let mut art = Paragraph::new("no album art").fg(Color::Yellow);
            if self.selected_field == TrackInspectorSelectedField::Art {
                art = art.fg(Color::Green);
            }
            art.render(art_area, buf);
        }
    }
}

pub struct AppState {
    pub library: MusicLibrary,
    list: Vec<TrackInspector>,
    list_state: ListState,
    pub track_inspector: Option<TrackInspector>,
    pub images: HashMap<String, Asset<StatefulProtocol>>,
    pub selected_panel: SelectedPanel,

    pub picker: Picker,

    pub shift: bool,

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
            track_inspector: Default::default(),
            images: Default::default(),
            selected_panel: Default::default(),
            picker,
            shift: Default::default(),
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
    KeyPressed(KeyCode, Modifiers),
    Redraw,
}

pub enum KeyCode {
    Backspace,
    Char(char),
    Enter,
    Escape,
    Tab,
}

pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub hyper: bool,
}

impl Modifiers {
    const NONE: Modifiers = Modifiers {
        shift: false,
        ctrl: false,
        alt: false,
        hyper: false,
    };
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
