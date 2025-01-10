use anyhow::Result;
use crossterm::event::Event as CrosstermEvent;
use crossterm::event::EventStream;
use crossterm::event::MouseButton;
use crossterm::event::MouseEventKind;
use futures::StreamExt;
use tokio::sync::mpsc;
use tokio::time;
use tui_textarea::Input;
use tui_textarea::Key;

use crate::domain::models::Event;
use crate::domain::models::Point;

pub struct EventsService {
    crossterm_events: EventStream,
    events: mpsc::UnboundedReceiver<Event>,
    selection_start: Option<Point>,
}

impl EventsService {
    pub fn new(events: mpsc::UnboundedReceiver<Event>) -> EventsService {
        return EventsService {
            crossterm_events: EventStream::new(),
            events,
            selection_start: None,
        };
    }

    fn handle_crossterm(&mut self, event: CrosstermEvent) -> Option<Event> {
        match event {
            CrosstermEvent::Paste(text) => {
                return Some(Event::KeyboardPaste(text));
            }
            CrosstermEvent::Mouse(mouseevent) => {
                match mouseevent.kind {
                    MouseEventKind::ScrollUp => {
                        return Some(Event::UIScrollUp());
                    }
                    MouseEventKind::ScrollDown => {
                        return Some(Event::UIScrollDown());
                    }
                    MouseEventKind::Down(MouseButton::Left) => {
                        self.selection_start = Some(Point {
                            column: mouseevent.column as usize,
                            row: mouseevent.row as usize,
                        });
                        return None;
                    }
                    MouseEventKind::Drag(MouseButton::Left) => {
                        return self.selection_start.map(|selection_start| {
                            return Some(Event::Highlight(selection_start, Point {
                                column: mouseevent.column as usize,
                                row: mouseevent.row as usize,
                            }));
                        })?;
                    }
                    MouseEventKind::Up(MouseButton::Left) => {
                        return self.selection_start.map(|selection_start| {
                            let selection = Event::Select(selection_start, Point {
                                column: mouseevent.column as usize,
                                row: mouseevent.row as usize,
                            });
                            self.selection_start = None;
                            return Some(selection);
                        })?;
                    }
                    _ => return None,
                }
            }
            CrosstermEvent::Key(keyevent) => {
                match keyevent.into() {
                    Input { key: Key::Down, .. } => {
                        return Some(Event::UIScrollDown());
                    }
                    Input { key: Key::Up, .. } => {
                        return Some(Event::UIScrollUp());
                    }
                    Input {
                        key: Key::MouseScrollDown,
                        ..
                    } => {
                        return Some(Event::UIScrollDown());
                    }
                    Input {
                        key: Key::MouseScrollUp,
                        ..
                    } => {
                        return Some(Event::UIScrollUp());
                    }
                    Input {
                        key: Key::PageDown, ..
                    } => {
                        return Some(Event::UIScrollPageDown());
                    }
                    Input {
                        key: Key::PageUp, ..
                    } => {
                        return Some(Event::UIScrollPageUp());
                    }
                    Input {
                        key: Key::Char('d'),
                        ctrl: true,
                        ..
                    } => {
                        return Some(Event::UIScrollPageDown());
                    }
                    Input {
                        key: Key::Char('u'),
                        ctrl: true,
                        ..
                    } => {
                        return Some(Event::UIScrollPageUp());
                    }
                    Input {
                        key: Key::Char('c'),
                        ctrl: true,
                        ..
                    } => {
                        return Some(Event::KeyboardCTRLC());
                    }
                    Input {
                        key: Key::Char('o'),
                        ctrl: true,
                        ..
                    } => {
                        return Some(Event::KeyboardCTRLO());
                    }
                    Input {
                        key: Key::Char('r'),
                        ctrl: true,
                        ..
                    } => {
                        return Some(Event::KeyboardCTRLR());
                    }
                    Input {
                        key: Key::Enter, ..
                    } => {
                        return Some(Event::KeyboardEnter());
                    }
                    input => {
                        return Some(Event::KeyboardCharInput(input));
                    }
                }
            }
            _ => return None,
        }
    }

    pub async fn next(&mut self) -> Result<Event> {
        loop {
            let evt = tokio::select! {
                event = self.events.recv() => event,
                event = self.crossterm_events.next() => match event {
                    Some(Ok(input)) => self.handle_crossterm(input),
                    Some(Err(_)) => None,
                    None => None
                },
                () = time::sleep(time::Duration::from_millis(500)) => Some(Event::UITick())
            };

            if let Some(event) = evt {
                return Ok(event);
            }
        }
    }
}
