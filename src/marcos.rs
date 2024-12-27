#![allow(dead_code)]

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use paste::paste;

macro_rules! key_asserter {
    ( $( $char:tt ),* ) => {

        pub trait KeyAsserter {
            $(
                paste! {
                    fn [<is_n_$char>](&self) -> bool;
                    fn [<is_c_$char>](&self) -> bool;
                    fn [<is_s_$char>](&self) -> bool;
                    fn [<is_a_$char>](&self) -> bool;
                    fn [<is_cs_$char>](&self) -> bool;
                    fn [<is_ca_$char>](&self) -> bool;
                    fn [<is_sa_$char>](&self) -> bool;
                }
            )*
        }

        impl KeyAsserter for KeyEvent {
            $(
                paste! {
                    fn [<is_n_$char>](&self) -> bool {
                        self.modifiers.is_empty() && self.code == KeyCode::Char(stringify!($char).chars().next().unwrap())
                    }
                    fn [<is_c_$char>](&self) -> bool {
                        self.modifiers.contains(KeyModifiers::CONTROL) && self.code == KeyCode::Char(stringify!($char).chars().next().unwrap())
                    }
                    fn [<is_s_$char>](&self) -> bool {
                        self.modifiers.contains(KeyModifiers::SHIFT) && self.code == KeyCode::Char(stringify!($char).chars().next().unwrap())
                    }
                    fn [<is_a_$char>](&self) -> bool {
                        self.modifiers.contains(KeyModifiers::ALT) && self.code == KeyCode::Char(stringify!($char).chars().next().unwrap())
                    }
                    fn [<is_cs_$char>](&self) -> bool {
                        self.modifiers.contains(KeyModifiers::CONTROL) && self.modifiers.contains(KeyModifiers::SHIFT) && self.code == KeyCode::Char(stringify!($char).chars().next().unwrap())
                    }
                    fn [<is_ca_$char>](&self) -> bool {
                        self.modifiers.contains(KeyModifiers::SHIFT) && self.modifiers.contains(KeyModifiers::ALT) && self.code == KeyCode::Char(stringify!($char).chars().next().unwrap())
                    }
                    fn [<is_sa_$char>](&self) -> bool {
                        self.modifiers.contains(KeyModifiers::SHIFT) && self.modifiers.contains(KeyModifiers::ALT) && self.code == KeyCode::Char(stringify!($char).chars().next().unwrap())
                    }
                }
            )*
        }
    };
}
key_asserter!(a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p, q, r, s, t, u, v, w, x, y, z, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9);

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_key_asserter() {
        let event = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL);
        assert!(event.is_c_a());

        let event = KeyEvent::new(KeyCode::Char('0'), KeyModifiers::empty());
        assert!(event.is_n_0());
    }
}
