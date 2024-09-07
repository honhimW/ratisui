use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub fn none_match(key_event: &KeyEvent, key_code: KeyCode) -> bool {
    none_modifier(key_event) && key_event.code == key_code
}

pub fn ctrl_match(key_event: &KeyEvent, key_code: KeyCode) -> bool {
    ctrl_modifier(key_event) && key_event.code == key_code
}

pub fn none_modifier(key_event: &KeyEvent) -> bool {
    key_event.modifiers == KeyModifiers::NONE
}

pub fn ctrl_modifier(key_event: &KeyEvent) -> bool {
    key_event.modifiers == KeyModifiers::CONTROL
}

pub fn alt_modifier(key_event: &KeyEvent) -> bool {
    key_event.modifiers == KeyModifiers::ALT
}

pub fn shift_modifier(key_event: &KeyEvent) -> bool {
    key_event.modifiers == KeyModifiers::SHIFT
}
