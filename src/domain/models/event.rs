use std::cmp::Ordering;

use tui_textarea::Input;

use super::BackendResponse;
use super::Message;

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct Point {
    pub column: usize,
    pub row: usize,
}

impl Point {
    pub fn shift_row(&self, shift: usize) -> Point {
        return Point {
            column: self.column,
            row: self.row + shift,
        };
    }
}

impl Ord for Point {
    fn cmp(&self, other: &Self) -> Ordering {
        return match self.row.cmp(&other.row) {
            Ordering::Equal => self.column.cmp(&other.column),
            row_cmp => row_cmp,
        };
    }
}

impl PartialOrd for Point {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        return Some(self.cmp(other));
    }
}

pub enum Event {
    BackendMessage(Message),
    BackendPromptResponse(BackendResponse),
    KeyboardCharInput(Input),
    KeyboardCTRLC(),
    KeyboardCTRLO(),
    KeyboardCTRLR(),
    KeyboardEnter(),
    KeyboardPaste(String),
    UITick(),
    UIScrollDown(),
    UIScrollUp(),
    UIScrollPageDown(),
    UIScrollPageUp(),
    Select(Point, Point),
    Highlight(Point, Point),
}
