use color_eyre::Result;
use ratatui::crossterm::event::{self, Event as CE, KeyModifiers};
use tokio::sync::mpsc::Sender;

use crate::events::{Event, KeyCode, Modifiers};

pub fn handle_terminal_events(tx: Sender<Event>) -> Result<()> {
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
            CE::Resize(_, _) => {
                if tx.blocking_send(Event::Redraw).is_ok() {
                    continue;
                }
                break Ok(());
            }
        }
    }
}
