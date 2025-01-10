use std::collections::BTreeMap;

use ratatui::prelude::Buffer;
use ratatui::prelude::Rect;
use ratatui::style::Color;
use ratatui::text::Line;
use syntect::highlighting::Theme;

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

    pub fn clear_selection(&mut self) {
        for (_, entry) in self.cache.iter_mut() {
            for line in entry.lines.iter_mut() {
                line.spans.iter_mut().for_each(|span| {
                    span.style = span.style.bg(Color::default());
                })
            }
        }
    }

    pub fn update_selected_lines(&mut self, start: &Point, end: &Point) {
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

                // // Update the style for the selected lines
                for i in start_row..=end_row {
                    if let Some(line) = entry.lines.get_mut(i) {
                        line.spans.iter_mut().for_each(|span| {
                            let trimmed = span.content.trim();
                            if !(trimmed.is_empty()
                                || trimmed.starts_with('│')
                                || trimmed.ends_with('│'))
                            {
                                span.style = span.style.bg(Color::DarkGray);
                            }
                        });
                    }
                }
            }
            current_line = entry_end;
        }
    }

    pub fn yank_selected_lines(&self, start: &Point, end: &Point) -> String {
        let mut current_line = 0;
        let mut selected_lines = Vec::with_capacity(1 + end.row - start.row);
        for (_, entry) in self.cache.iter() {
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

                for i in start_row..=end_row {
                    if let Some(line) = entry.lines.get(i) {
                        selected_lines.push(
                            line.spans
                                .iter()
                                .map(|span| {
                                    return if span.content.contains('│')
                                        || span.content.clone().trim().is_empty()
                                        || span.content.contains('╰')
                                        || span.content.contains('╯')
                                        || span.content.contains('╭')
                                        || span.content.contains('╮')
                                    {
                                        String::default()
                                    } else {
                                        String::from(span.content.clone())
                                    };
                                })
                                .collect::<String>(),
                        );
                    }
                }
                return selected_lines.join("\n");
            }
            current_line = entry_end;
        }
        unreachable!("");
    }
}
