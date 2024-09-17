use std::thread::sleep;
use std::time::Duration;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, BorderType, Clear, Table, Widget};
use ratatui::{Frame, TerminalOptions, Viewport};
use ratatui::buffer::Buffer;
use ratatui::text::Text;
use tui_textarea::TextArea;

fn main() {
    let mut terminal = ratatui::init_with_options(TerminalOptions {
        viewport: Viewport::Inline(10),
    });
    // for _ in 0..10 {
    //     sleep(Duration::from_millis(100));
    //     terminal
    //         .draw(|frame: &mut Frame| {
    //             draw_picture(frame);
    //         })
    //         .expect("Failed to draw");
    //     terminal.insert_before(1, |buf| {
    //         Text::raw("Next.").render(buf.area, buf);
    //     }).unwrap();
    // }
    terminal.insert_before(10, |buf| {
        print_picture(buf);
    }).unwrap();
    terminal.clear().expect("Failed to clear");
    // ratatui::restore();
}

fn print_picture(buf: &mut Buffer) {
    let rect = buf.area();
    let mut text_area = TextArea::default();
    text_area.set_block(Block::bordered());
    text_area.insert_str("hello");
    let (y, x) = text_area.cursor();
    let mut table = Table::default();
    table = table.block(Block::bordered().border_type(BorderType::Double));

    let area = Rect {
        height: rect.height - 1,
        ..rect.clone()
    };

    let menu_area = Rect {
        x: area.x + x as u16 + 1,
        y: area.y + y as u16 + 2,
        width: 20,
        height: 5,
    };

    text_area.render(area, buf);
    table.render(menu_area, buf);
}

fn draw_picture(frame: &mut Frame) {
    let rect = frame.area();
    let mut text_area = TextArea::default();
    text_area.set_block(Block::bordered());
    text_area.insert_str("hello");
    let (x, y) = text_area.cursor();
    let mut table = Table::default();

    let area = Rect {
        height: rect.height - 1,
        ..rect
    };

    let menu_area = Rect {
        x: area.x + x as u16,
        y: area.y + y as u16,
        ..area
    };

    frame.render_widget(table, menu_area);
    frame.render_widget(&text_area, area);
}
