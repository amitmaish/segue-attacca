mod assets;
mod events;
mod terminal_events;
mod track_inspector;
mod track_list;

use std::{collections::HashMap, ops::Deref, sync::Arc, thread};

use assets::Asset;
use color_eyre::Result;
use events::{Event, KeyCode};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout},
    style::{Color, Style, Stylize},
    widgets::{Block, BorderType, List, ListState},
};
use ratatui_image::{picker::Picker, protocol::StatefulProtocol};
use segue_attacca_lib::music_library::MusicLibrary;
use terminal_events::handle_terminal_events;
use tokio::sync::mpsc::{Receiver, Sender, channel};
use tracing::warn;
use track_inspector::{TrackInspector, handle_inspector_events};
use track_list::handle_track_list_events;

const DEFAULT_COLOR: Color = Color::LightBlue;
const FOCUS_COLOR: Color = Color::LightMagenta;
const SELECT_COLOR: Color = Color::Green;

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
        .map(|track| TrackInspector::new(Arc::downgrade(track)))
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
        .highlight_style(Style::new().fg(SELECT_COLOR))
        .repeat_highlight_symbol(true);
    let mut inspector = Block::bordered()
        .title(" [2] inspector ")
        .border_type(BorderType::Rounded)
        .fg(DEFAULT_COLOR);

    match state.selected_panel {
        SelectedPanel::TrackList => list = list.fg(FOCUS_COLOR),
        SelectedPanel::Inspector => inspector = inspector.fg(FOCUS_COLOR),
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
        let picker = match Picker::from_query_stdio() {
            Ok(picker) => picker,
            Err(e) => {
                warn!("couldn't query stdio for picker: {e}");
                Picker::from_fontsize((7, 14))
            }
        };
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
