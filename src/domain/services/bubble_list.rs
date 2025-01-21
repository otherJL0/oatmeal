use std::collections::BTreeMap;

use ratatui::prelude::Buffer;
use ratatui::prelude::Rect;
use ratatui::style::Color;
use ratatui::text::Line;
use syntect::highlighting::Theme;
use unicode_width::UnicodeWidthStr;

use super::Bubble;
use super::BubbleAlignment;
use crate::domain::models::Author;
use crate::domain::models::Message;
use crate::domain::models::Point;

#[cfg(test)]
#[path = "bubble_list_test.rs"]
mod tests;

struct BubbleCacheEntry<'a> {
    codeblocks_count: usize,
    text_len: usize,
    lines: Vec<Line<'a>>,
}

pub struct BubbleList<'a> {
    cache: BTreeMap<usize, BubbleCacheEntry<'a>>,
    pub line_width: usize,
    pub lines_len: usize,
    pub theme: Theme,
}

fn highlight_line(line: &mut Line, start_point: Option<Point>, end_point: Option<Point>) {
    let mut left_pipe_found: bool = false;
    let mut right_pipe_found: bool = false;
    let mut current_column: usize = 0;
    for span in line.spans.iter_mut() {
        current_column += span.content.width();
        if right_pipe_found {
            break;
        }

        if span.content.contains('│') {
            if left_pipe_found {
                right_pipe_found = true;
            } else {
                left_pipe_found = true;
            }
            continue;
        }
        if span.content.contains('╭')
            || span.content.contains('╮')
            || span.content.contains('╰')
            || span.content.contains('╯')
            || span.content.contains('─')
        {
            continue;
        }

        if left_pipe_found && current_column >= start_point.unwrap_or_default().column {
            span.style = span.style.bg(Color::DarkGray);
        }
        if let Some(end) = end_point
            && current_column >= end.column
        {
            break;
        }
    }
}

impl<'a> BubbleList<'a> {
    pub fn new(theme: Theme) -> BubbleList<'a> {
        return BubbleList {
            cache: BTreeMap::new(),
            line_width: 0,
            lines_len: 0,
            theme,
        };
    }

    pub fn set_messages(&mut self, messages: &[Message], line_width: usize) {
        if self.line_width != line_width {
            self.line_width = line_width;
        }

        let mut total_codeblock_counter = 0;
        self.lines_len = messages
            .iter()
            .enumerate()
            .map(|(idx, message)| {
                if self.cache.contains_key(&idx) {
                    let cache_entry = self.cache.get(&idx).unwrap();
                    if idx < (messages.len() - 1) || message.text.len() == cache_entry.text_len {
                        total_codeblock_counter += cache_entry.codeblocks_count;
                        return cache_entry.lines.len();
                    }
                }

                let mut align = BubbleAlignment::Left;
                if message.author == Author::User {
                    align = BubbleAlignment::Right;
                }

                let bubble_lines = Bubble::new(message, align, line_width, total_codeblock_counter)
                    .as_lines(&self.theme);
                let bubble_line_len = bubble_lines.len();

                let codeblocks_count = message.codeblocks().len();
                total_codeblock_counter += codeblocks_count;

                self.cache.insert(idx, BubbleCacheEntry {
                    codeblocks_count,
                    text_len: message.text.len(),
                    lines: bubble_lines,
                });

                return bubble_line_len;
            })
            .sum();
    }

    pub fn len(&self) -> usize {
        return self.lines_len;
    }

    pub fn render(&self, rect: Rect, buf: &mut Buffer, scroll_index: u16) {
        let mut cache_keys: Vec<usize> = self.cache.keys().copied().collect();
        cache_keys.sort_unstable();

        let mut line_idx = 0;
        let mut should_break = false;

        for cache_key in cache_keys {
            for line in self.cache.get(&cache_key).unwrap().lines.as_slice() {
                if line_idx < scroll_index {
                    line_idx += 1;
                    continue;
                }

                if (line_idx - scroll_index) >= rect.height {
                    should_break = true;
                    break;
                }

                buf.set_line(0, line_idx - scroll_index, line, rect.width);
                line_idx += 1;
            }

            if should_break {
                break;
            }
        }
    }

    pub fn reset_highlight(&mut self) {
        for (_, entry) in self.cache.iter_mut() {
            for line in entry.lines.iter_mut() {
                line.spans.iter_mut().for_each(|span| {
                    // if let Some(Color::DarkGray) = span.style.bg {
                    //     tracing::debug!("highlight span: {:?}", span)
                    // }
                    span.style = span.style.bg(Color::default());
                })
            }
        }
    }

    pub fn yank_selected_lines(&mut self) -> String {
        let mut selected_lines: Vec<String> = Vec::new();
        for (_, entry) in self.cache.iter_mut() {
            for line in entry.lines.iter_mut() {
                let mut current_line = Vec::new();
                line.spans.iter_mut().for_each(|span| {
                    if let Some(Color::DarkGray) = span.style.bg {
                        tracing::debug!("highlight span: {:?}", span);
                        current_line.push(String::from(span.content.clone()));
                    }
                });
                if !current_line.is_empty() {
                    tracing::debug!("{:?}", current_line);
                    selected_lines.push(current_line.join(""));
                }
            }
        }
        return selected_lines.join("\n");
    }

    pub fn highlight_selected_lines(&mut self, start: &Point, end: &Point) {
        let mut current_line = 0;
        for (_, entry) in self.cache.iter_mut() {
            let entry_line_count = entry.lines.len();
            let entry_end = current_line + entry_line_count;

            // Check if this entry contains any of the selected lines
            if current_line <= end.row && entry_end > start.row {
                // Calculate which lines in this entry need highlighting
                let start_row = start.row.saturating_sub(current_line);
                let end_row = end
                    .row
                    .saturating_sub(current_line)
                    .min(entry_line_count - 1);

                // Update the style for the selected lines
                if let Some(line) = entry.lines.get_mut(start_row) {
                    highlight_line(line, Some(*start), None);
                }
                for i in start_row + 1..end_row {
                    if let Some(line) = entry.lines.get_mut(i) {
                        highlight_line(line, None, None);
                    }
                }
                if let Some(line) = entry.lines.get_mut(end_row) {
                    highlight_line(line, None, Some(*end));
                }
            }
            current_line = entry_end;
        }
    }
}
