#![allow(dead_code)]

use paste::paste;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratisui_macros::characterize;

/// Declare key asserter
/// ```rust
/// key_asserter!(a);
/// ```
/// Macro expansion:
/// ```rust
/// use ratatui::crossterm::event::KeyModifiers;
///
/// pub trait KeyAsserter {
///     fn is_n_a(&self) -> bool;
///     fn is_c_a(&self) -> bool;
///     fn is_s_a(&self) -> bool;
///     fn is_a_a(&self) -> bool;
///     fn is_cs_a(&self) -> bool;
///     fn is_ca_a(&self) -> bool;
///     fn is_sa_a(&self) -> bool;
/// }
/// impl KeyAsserter for KeyEvent {
///     fn is_n_a(&self) -> bool {
///         self.modifiers.is_empty() && self.code == KeyCode::Char('a')
///     }
///     fn is_c_a(&self) -> bool {
///         self.modifiers.contains(KeyModifiers::CONTROL)
///             && self.code == KeyCode::Char('a')
///     }
///     fn is_s_a(&self) -> bool {
///         self.modifiers.contains(KeyModifiers::SHIFT)
///             && self.code == KeyCode::Char('a')
///     }
///     fn is_a_a(&self) -> bool {
///         self.modifiers.contains(KeyModifiers::ALT)
///             && self.code == KeyCode::Char('a')
///     }
///     fn is_cs_a(&self) -> bool {
///         self.modifiers.contains(KeyModifiers::CONTROL)
///             && self.modifiers.contains(KeyModifiers::SHIFT)
///             && self.code == KeyCode::Char('a')
///     }
///     fn is_ca_a(&self) -> bool {
///         self.modifiers.contains(KeyModifiers::SHIFT)
///             && self.modifiers.contains(KeyModifiers::ALT)
///             && self.code == KeyCode::Char('a')
///     }
///     fn is_sa_a(&self) -> bool {
///         self.modifiers.contains(KeyModifiers::SHIFT)
///             && self.modifiers.contains(KeyModifiers::ALT)
///             && self.code == KeyCode::Char('a')
///     }
/// }
//// ```
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
                        self.modifiers.is_empty() && self.code == KeyCode::Char(characterize!($char))
                    }
                    fn [<is_c_$char>](&self) -> bool {
                        self.modifiers.contains(KeyModifiers::CONTROL) && self.code == KeyCode::Char(characterize!($char))
                    }
                    fn [<is_s_$char>](&self) -> bool {
                        self.modifiers.contains(KeyModifiers::SHIFT) && self.code == KeyCode::Char(characterize!($char))
                    }
                    fn [<is_a_$char>](&self) -> bool {
                        self.modifiers.contains(KeyModifiers::ALT) && self.code == KeyCode::Char(characterize!($char))
                    }
                    fn [<is_cs_$char>](&self) -> bool {
                        self.modifiers.contains(KeyModifiers::CONTROL | KeyModifiers::SHIFT) && self.code == KeyCode::Char(characterize!($char))
                    }
                    fn [<is_ca_$char>](&self) -> bool {
                        self.modifiers.contains(KeyModifiers::CONTROL | KeyModifiers::ALT) && self.code == KeyCode::Char(characterize!($char))
                    }
                    fn [<is_sa_$char>](&self) -> bool {
                        self.modifiers.contains(KeyModifiers::SHIFT | KeyModifiers::ALT) && self.code == KeyCode::Char(characterize!($char))
                    }
                }
            )*
        }
    };
}

key_asserter!(
    a, b, c, d, e, f, g, h, i, j,
    k, l, m, n, o, p, q, r, s, t,
    u, v, w, x, y, z,
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9
);

#[cfg(test)]
mod test {
    use ratisui_macros::characterize;
    use super::*;

    #[test]
    fn test_key_asserter() {
        assert_eq!(characterize!(a), 'a');
        let event = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL);
        event.modifiers.contains(KeyModifiers::CONTROL | KeyModifiers::ALT);
        assert!(event.is_c_a());
        let event = KeyEvent::new(KeyCode::Char('0'), KeyModifiers::empty());
        assert!(event.is_n_0());
        let event = KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL | KeyModifiers::SHIFT);
        assert!(event.is_cs_z());
    }

}
