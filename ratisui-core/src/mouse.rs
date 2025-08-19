use ratatui::crossterm::event::MouseEventKind::*;
use ratatui::crossterm::event::{MouseButton, MouseEvent};
use MouseButton::{Left, Middle, Right};

#[allow(unused)]
pub trait MouseEventHelper {
    fn is_left_down(&self) -> bool;
    fn is_left_up(&self) -> bool;
    fn is_right_down(&self) -> bool;
    fn is_right_up(&self) -> bool;
    fn is_middle_down(&self) -> bool;
    fn is_middle_up(&self) -> bool;
    fn is_left_drag(&self) -> bool;
    fn is_right_drag(&self) -> bool;
    fn is_middle_drag(&self) -> bool;
    fn is_moved(&self) -> bool;
    fn is_scroll_down(&self) -> bool;
    fn is_scroll_up(&self) -> bool;
    fn is_scroll_left(&self) -> bool;
    fn is_scroll_right(&self) -> bool;
}

impl MouseEventHelper for MouseEvent {
    fn is_left_down(&self) -> bool {
        matches!(self.kind, Down(Left))
    }

    fn is_left_up(&self) -> bool {
        matches!(self.kind, Up(Left))
    }

    fn is_right_down(&self) -> bool {
        matches!(self.kind, Down(Right))
    }

    fn is_right_up(&self) -> bool {
        matches!(self.kind, Up(Right))
    }

    fn is_middle_down(&self) -> bool {
        matches!(self.kind, Down(Middle))
    }

    fn is_middle_up(&self) -> bool {
        matches!(self.kind, Up(Middle))
    }

    fn is_left_drag(&self) -> bool {
        matches!(self.kind, Drag(Left))
    }

    fn is_right_drag(&self) -> bool {
        matches!(self.kind, Drag(Right))
    }

    fn is_middle_drag(&self) -> bool {
        matches!(self.kind, Drag(Middle))
    }

    fn is_moved(&self) -> bool {
        matches!(self.kind, Moved)
    }

    fn is_scroll_down(&self) -> bool {
        matches!(self.kind, ScrollDown)
    }

    fn is_scroll_up(&self) -> bool {
        matches!(self.kind, ScrollUp)
    }

    fn is_scroll_left(&self) -> bool {
        matches!(self.kind, ScrollLeft)
    }

    fn is_scroll_right(&self) -> bool {
        matches!(self.kind, ScrollRight)
    }
}

