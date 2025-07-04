use std::sync::Arc;

use crate::{
    AppState,
    events::{Event, KeyCode},
    track_inspector::TrackInspector,
};

pub fn handle_track_list_events(event: &Event, state: &mut AppState) -> bool {
    match event {
        Event::KeyPressed(KeyCode::Char(c), _) => match c {
            'j' => {
                state.list_state.select_next();
                if let Some(track) = state
                    .library
                    .get_tracks()
                    .get(state.list_state().selected().unwrap_or(0))
                {
                    state.track_inspector = Some(TrackInspector::new(Arc::downgrade(track)));
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
                    state.track_inspector = Some(TrackInspector::new(Arc::downgrade(track)));
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
