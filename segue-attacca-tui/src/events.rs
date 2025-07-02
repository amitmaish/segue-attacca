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
    pub const NONE: Modifiers = Modifiers {
        shift: false,
        ctrl: false,
        alt: false,
        hyper: false,
    };
}
