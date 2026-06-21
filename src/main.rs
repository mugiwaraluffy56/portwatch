mod app;
mod process;
mod ui;

use std::{io, time::Duration};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use app::{App, AppState};
use ui::ui;

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match app.state {
                    AppState::Normal => match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                        KeyCode::Char('j') | KeyCode::Down => app.move_down(),
                        KeyCode::Up | KeyCode::Char('i') => app.move_up(),
                        KeyCode::Char('k') => app.kill_selected_if_safe(),
                        KeyCode::Char('r') => {
                            app.refresh();
                            app.status_msg = "Refreshed".to_string();
                        }
                        KeyCode::Char('/') => app.state = AppState::Search,
                        KeyCode::Char('s') => {
                            app.sort_col = app.sort_col.next();
                            app.apply_filter_sort();
                            app.status_msg = format!("Sorted by {}", app.sort_col.label());
                        }
                        KeyCode::Char('S') => {
                            app.sort_asc = !app.sort_asc;
                            app.apply_filter_sort();
                            app.status_msg = format!(
                                "Sort {}",
                                if app.sort_asc { "ascending" } else { "descending" }
                            );
                        }
                        KeyCode::Char('e') => app.toggle_established(),
                        KeyCode::Char('c') => app.copy_to_clipboard(),
                        KeyCode::Char('f') => app.open_telescope(),
                        _ => {}
                    },

                    AppState::Telescope => match key.code {
                        KeyCode::Esc => {
                            app.state = AppState::Normal;
                            app.tel_filter.clear();
                        }
                        KeyCode::Enter => app.tel_confirm(),
                        KeyCode::Up | KeyCode::Char('i') => app.tel_move_up(),
                        KeyCode::Down | KeyCode::Char('j') => app.tel_move_down(),
                        KeyCode::Backspace => {
                            app.tel_filter.pop();
                            app.tel_apply();
                        }
                        KeyCode::Char(c) => {
                            app.tel_filter.push(c);
                            app.tel_apply();
                        }
                        _ => {}
                    },

                    AppState::Search => match key.code {
                        KeyCode::Esc | KeyCode::Enter => app.state = AppState::Normal,
                        KeyCode::Backspace => {
                            app.filter.pop();
                            app.apply_filter_sort();
                        }
                        KeyCode::Char(c) => {
                            app.filter.push(c);
                            app.apply_filter_sort();
                        }
                        _ => {}
                    },

                    AppState::ConfirmKill => match key.code {
                        KeyCode::Char('y') => app.do_kill(),
                        KeyCode::Char('n') | KeyCode::Esc => {
                            app.state = AppState::Normal;
                            app.status_msg = "Cancelled".to_string();
                        }
                        _ => {}
                    },
                }
            }
        }

        if app.last_refresh.elapsed() > Duration::from_secs(5)
            && matches!(app.state, AppState::Normal | AppState::Telescope)
        {
            app.refresh();
        }
    }
}

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let result = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}
