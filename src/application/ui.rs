use std::io;

use anyhow::Error;
use anyhow::Result;
use crossterm::cursor;
use crossterm::event::DisableBracketedPaste;
use crossterm::event::DisableMouseCapture;
use crossterm::event::EnableBracketedPaste;
use crossterm::event::EnableMouseCapture;
use crossterm::terminal::EnterAlternateScreen;
use crossterm::terminal::LeaveAlternateScreen;
use crossterm::terminal::disable_raw_mode;
use crossterm::terminal::enable_raw_mode;
use crossterm::terminal::is_raw_mode_enabled;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Scrollbar;
use ratatui::widgets::ScrollbarOrientation;
use tokio::sync::mpsc;

use crate::configuration::Config;
use crate::configuration::ConfigKey;
use crate::domain::models::AcceptType;
use crate::domain::models::Action;
use crate::domain::models::Author;
use crate::domain::models::BackendName;
use crate::domain::models::BackendPrompt;
use crate::domain::models::EditorName;
use crate::domain::models::Event;
use crate::domain::models::Loading;
use crate::domain::models::Message;
use crate::domain::models::Point;
use crate::domain::models::SlashCommand;
use crate::domain::models::TextArea;
use crate::domain::services::AppState;
use crate::domain::services::AppStateProps;
use crate::domain::services::Bubble;
use crate::domain::services::Sessions;
use crate::domain::services::events::EventsService;
use crate::infrastructure::backends::BackendManager;
use crate::infrastructure::editors::EditorManager;

/// Verifies that the current window size is large enough to handle the bare
/// minimum width that includes the model name, username, bubbles, and padding.
fn is_line_width_sufficient(line_width: u16) -> bool {
    let author_lengths = vec![Author::User, Author::Oatmeal, Author::Model]
        .into_iter()
        .map(|e| return e.to_string().len())
        .max()
        .unwrap();

    let bubble_style = Bubble::style_config();
    let min_width =
        (author_lengths + bubble_style.bubble_padding + bubble_style.border_elements_length) as i32;
    let trimmed_line_width =
        ((f32::from(line_width) * (1.0 - bubble_style.outer_padding_percentage)).ceil()) as i32;

    return trimmed_line_width >= min_width;
}

/// Ensure that the start and end points do not include the input text area
fn validate_selected_region<B: Backend>(
    app_state: &AppState,
    start_point: Point,
    end_point: Point,
    terminal: &mut Terminal<B>,
    textarea: &tui_textarea::TextArea,
) -> Result<(Point, Point)> {
    let position = app_state.scroll.position;
    let start = start_point.min(end_point).shift_row(position);
    let end = start_point.max(end_point).shift_row(position);

    // Clicks in the bottom text box should be ignored
    let bottom_edge = position + terminal.size()?.height as usize - textarea.lines().len() - 3;
    if start.row < bottom_edge {
        return Ok((start, end));
    }
    return Err(Error::msg("None"));
}

async fn start_loop<B: Backend>(
    terminal: &mut Terminal<B>,
    app_state_props: AppStateProps,
    tx: mpsc::UnboundedSender<Action>,
    rx: mpsc::UnboundedReceiver<Event>,
) -> Result<()> {
    let mut events = EventsService::new(rx);
    let mut textarea = TextArea::default();
    let mut app_state = AppState::new(app_state_props).await?;
    let loading = Loading::default();

    #[cfg(feature = "dev")]
    {
        let test_str = "Write a function in Java that prints from 0 to 10. Describe the example before and after.";
        textarea.insert_str(test_str);
    }

    loop {
        terminal.draw(|frame| {
            if !is_line_width_sufficient(frame.area().width) {
                frame.render_widget(
                    Paragraph::new("I'm too small, make me bigger!").alignment(Alignment::Left),
                    frame.area(),
                );
                return;
            }

            let textarea_len = (textarea.lines().len() + 3).try_into().unwrap();
            let layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints(vec![Constraint::Min(1), Constraint::Max(textarea_len)])
                .split(frame.area());

            if layout[0].width as usize != app_state.last_known_width
                || layout[0].height as usize != app_state.last_known_height
            {
                app_state.set_rect(layout[0]);
            }

            app_state.bubble_list.render(
                layout[0],
                frame.buffer_mut(),
                app_state.scroll.position.try_into().unwrap(),
            );

            frame.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight),
                layout[0].inner(Margin {
                    vertical: 1,
                    horizontal: 0,
                }),
                &mut app_state.scroll.scrollbar_state,
            );

            if app_state.waiting_for_backend {
                loading.render(frame, layout[1]);
            } else {
                // Frame::render_widget(&textarea,);
                frame.render_widget(&textarea, layout[1]);
                // frame.render_widget(textarea.widget(), layout[1]);
            }
        })?;

        macro_rules! send_user_message {
            ( $input_str:expr ) => {
                let input_str = $input_str;

                let msg = Message::new(Author::User, &input_str);
                textarea = TextArea::default();
                app_state.add_message(msg);

                let (should_break, should_continue) =
                    app_state.handle_slash_commands(input_str, &tx)?;

                if should_break {
                    break;
                }
                if should_continue {
                    continue;
                }

                app_state.waiting_for_backend = true;
                let mut prompt =
                    BackendPrompt::new(input_str.to_string(), app_state.backend_context.clone());

                if app_state.backend_context.is_empty() && SlashCommand::parse(&input_str).is_none()
                {
                    prompt.append_chat_context(&app_state.editor_context);
                }

                tx.send(Action::BackendRequest(prompt))?;
                app_state.save_session().await?;
            };
        }

        match events.next().await? {
            Event::BackendMessage(msg) => {
                app_state.add_message(msg);
                app_state.waiting_for_backend = false;
            }
            Event::BackendPromptResponse(msg) => {
                app_state.handle_backend_response(msg.clone());
                if msg.done {
                    app_state.save_session().await?;
                }
            }
            Event::KeyboardCharInput(input) => {
                if app_state.waiting_for_backend {
                    continue;
                }

                // Windows submits a null event right after CTRL+C. Ignore it.
                if input.key != tui_textarea::Key::Null {
                    app_state.exit_warning = false;
                }

                let max_width = terminal.size()?.width as usize;
                let line_width = textarea.lines().last().unwrap().len();
                if line_width + 5 >= max_width {
                    textarea.move_cursor(tui_textarea::CursorMove::WordBack);
                    textarea.insert_newline();
                    textarea.move_cursor(tui_textarea::CursorMove::WordForward);
                }
                textarea.input(input);
            }
            Event::KeyboardCTRLC() => {
                if app_state.waiting_for_backend {
                    app_state.waiting_for_backend = false;
                    tx.send(Action::BackendAbort())?;
                } else if !app_state.exit_warning {
                    app_state.add_message(Message::new(
                        Author::Oatmeal,
                        "If you wish to quit, hit CTRL+C one more time, or use /quit",
                    ));
                    app_state.exit_warning = true;
                } else {
                    break;
                }
            }
            Event::KeyboardCTRLO() => {
                if app_state.waiting_for_backend {
                    continue;
                }
                app_state.exit_warning = false;
                textarea.insert_newline();
            }
            Event::KeyboardCTRLR() => {
                let last_message = app_state
                    .messages
                    .iter()
                    .filter(|msg| {
                        return msg.author == Author::User
                            && SlashCommand::parse(&msg.text).is_none();
                    })
                    .last();
                if let Some(message) = last_message.cloned() {
                    send_user_message!(&message.text);
                }
            }
            Event::KeyboardEnter() => {
                if app_state.waiting_for_backend {
                    continue;
                }
                let input_str = &textarea.lines().join("\n");
                if input_str.is_empty() {
                    continue;
                }
                send_user_message!(input_str);
            }
            Event::KeyboardPaste(text) => {
                if app_state.waiting_for_backend {
                    continue;
                }
                app_state.exit_warning = false;
                textarea.set_yank_text(text.replace('\r', "\n"));
                textarea.paste();
            }
            Event::UITick() => {
                continue;
            }
            Event::UIScrollDown() => {
                app_state.scroll.down();
            }
            Event::UIScrollUp() => {
                app_state.scroll.up();
            }
            Event::UIScrollPageDown() => {
                app_state.scroll.down_page();
            }
            Event::UIScrollPageUp() => {
                app_state.scroll.up_page();
            }
            Event::Highlight(downclick_point, releaseclick_point) => {
                if let Ok((start, end)) = validate_selected_region(
                    &app_state,
                    downclick_point,
                    releaseclick_point,
                    terminal,
                    &textarea,
                ) {
                    app_state.bubble_list.reset_highlight();
                    app_state.bubble_list.highlight_selected_lines(&start, &end);
                }
            }
            Event::Select(downclick_point, releaseclick_point) => {
                if let Ok((start, end)) = validate_selected_region(
                    &app_state,
                    downclick_point,
                    releaseclick_point,
                    terminal,
                    &textarea,
                ) {
                    let selected_text: String =
                        app_state.bubble_list.yank_selected_lines(&start, &end);

                    tx.send(Action::AcceptCodeBlock(
                        app_state.editor_context.clone(),
                        selected_text,
                        AcceptType::Replace,
                    ))?;
                    app_state.bubble_list.reset_highlight();
                }
            }
        }
    }

    return Ok(());
}

pub fn destruct_terminal_for_panic() {
    if let Ok(enabled) = is_raw_mode_enabled() {
        if enabled {
            let _ = disable_raw_mode();
            let _ = crossterm::execute!(
                io::stdout(),
                LeaveAlternateScreen,
                DisableMouseCapture,
                DisableBracketedPaste
            );
            let _ = crossterm::execute!(io::stdout(), cursor::Show);
        }
    }
}

pub async fn start(
    tx: mpsc::UnboundedSender<Action>,
    rx: mpsc::UnboundedReceiver<Event>,
) -> Result<()> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    enable_raw_mode()?;
    crossterm::execute!(
        stdout,
        EnterAlternateScreen,
        EnableMouseCapture,
        EnableBracketedPaste
    )?;
    let term_backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(term_backend)?;
    let editor_name = EditorName::parse(Config::get(ConfigKey::Editor)).unwrap();
    let mut session_id = None;
    if !Config::get(ConfigKey::SessionID).is_empty() {
        session_id = Some(Config::get(ConfigKey::SessionID));
    }

    let backend =
        BackendManager::get(BackendName::parse(Config::get(ConfigKey::Backend)).unwrap())?;
    let editor = EditorManager::get(EditorName::parse(Config::get(ConfigKey::Editor)).unwrap())?;
    let app_state_pros = AppStateProps {
        backend,
        editor,
        model_name: Config::get(ConfigKey::Model),
        theme_name: Config::get(ConfigKey::Theme),
        theme_file: Config::get(ConfigKey::ThemeFile),
        session_id,
        sessions_service: Sessions::default(),
    };

    start_loop(&mut terminal, app_state_pros, tx, rx).await?;
    let editor = EditorManager::get(editor_name)?;
    if editor.health_check().await.is_ok() {
        editor.clear_context().await?;
    }

    disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        DisableBracketedPaste
    )?;
    terminal.show_cursor()?;

    return Ok(());
}
