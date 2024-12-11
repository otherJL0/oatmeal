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

pub struct EventsService {
    crossterm_events: EventStream,
    events: mpsc::UnboundedReceiver<Event>,
    selection_start: Option<u16>,
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
                        self.selection_start = Some(mouseevent.row);
                        return None;
                    }
                    MouseEventKind::Up(MouseButton::Left) => {
                        assert!(self.selection_start.is_some());
                        let selection =
                            Event::Select((self.selection_start.unwrap(), mouseevent.row));
                        self.selection_start = None;
                        return Some(selection);
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
