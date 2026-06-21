use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::app::{App, AppState, SortCol};
use crate::process::is_protected;

pub fn col(s: &str, w: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() > w {
        let truncated: String = chars[..w.saturating_sub(1)].iter().collect();
        format!("{:<w$}", truncated + ">", w = w)
    } else {
        format!("{:<w$}", s, w = w)
    }
}

pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let pop = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(pop[1])[1]
}

fn col_style(col: SortCol, active: SortCol, asc: bool) -> (Style, &'static str) {
    if col == active {
        (
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            if asc { "▲" } else { "▼" },
        )
    } else {
        (Style::default().fg(Color::DarkGray), "")
    }
}

pub fn ui(f: &mut Frame, app: &mut App) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // column header
            Constraint::Min(3),    // process list
            Constraint::Length(4), // detail panel
            Constraint::Length(3), // status / search
            Constraint::Length(1), // help bar
        ])
        .split(area);

    // ── column header ───────────────────────────────────────────
    let (cmd_style, cmd_arr) = col_style(SortCol::Command, app.sort_col, app.sort_asc);
    let (pid_style, pid_arr) = col_style(SortCol::Pid, app.sort_col, app.sort_asc);
    let (usr_style, usr_arr) = col_style(SortCol::User, app.sort_col, app.sort_asc);
    let (prt_style, prt_arr) = col_style(SortCol::Port, app.sort_col, app.sort_asc);

    let header = Line::from(vec![
        Span::raw("     "),
        Span::styled(format!("{:<16}", format!("COMMAND{cmd_arr}")), cmd_style),
        Span::styled(format!("{:<7}", format!("PID{pid_arr}")), pid_style),
        Span::styled(format!("{:<20}", format!("USER{usr_arr}")), usr_style),
        Span::styled(format!("{:<6}", "PROTO"), Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{:<7}", format!("PORT{prt_arr}")), prt_style),
        Span::styled("ADDRESS", Style::default().fg(Color::DarkGray)),
    ]);
    f.render_widget(Paragraph::new(header), chunks[0]);

    // ── process list ─────────────────────────────────────────────
    let established_tag = if app.show_established { "+ESTAB" } else { "" };
    let filter_tag = if app.filter.is_empty() {
        String::new()
    } else {
        format!("  /{}/ ", app.filter)
    };
    let title = format!(
        " portwatch — {} processes{}{} ",
        app.processes.len(),
        established_tag,
        filter_tag,
    );

    let items: Vec<ListItem> = app
        .processes
        .iter()
        .map(|p| {
            let locked = is_protected(&p.command, &p.user);
            let fg = |c: Color| {
                if locked {
                    Style::default().fg(Color::DarkGray)
                } else {
                    Style::default().fg(c)
                }
            };
            let lock = if locked { "! " } else { "  " };
            let state_color = match p.conn_state.as_str() {
                "ESTAB" => Color::Blue,
                _ => Color::DarkGray,
            };
            Line::from(vec![
                Span::styled(lock.to_string(), Style::default().fg(Color::Red)),
                Span::styled(col(&p.command, 16), fg(Color::Cyan)),
                Span::styled(format!("{:<7}", p.pid), fg(Color::Yellow)),
                Span::styled(col(&p.user, 20), fg(Color::Green)),
                Span::styled(format!("{:<6}", p.proto), fg(Color::Magenta)),
                Span::styled(format!("{:<7}", p.port), fg(Color::White)),
                Span::styled(p.address.clone(), fg(Color::White)),
                Span::styled(
                    format!(" {}", p.conn_state),
                    Style::default().fg(state_color),
                ),
            ])
        })
        .map(ListItem::new)
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD))
        .highlight_symbol("> ");

    let mut main_list_state = if app.state == AppState::Telescope {
        ListState::default()
    } else {
        app.list_state.clone()
    };
    f.render_stateful_widget(list, chunks[1], &mut main_list_state);

    // ── detail panel ─────────────────────────────────────────────
    let (cmdline, stats) = app.get_details();
    let detail_lines = vec![
        Line::from(vec![
            Span::styled("cmd  ", Style::default().fg(Color::DarkGray)),
            Span::styled(cmdline, Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("stat ", Style::default().fg(Color::DarkGray)),
            Span::styled(stats, Style::default().fg(Color::Cyan)),
        ]),
    ];
    let detail = Paragraph::new(detail_lines)
        .block(Block::default().borders(Borders::ALL).title(" details "));
    f.render_widget(detail, chunks[2]);

    // ── status / search bar ───────────────────────────────────────
    let elapsed = app.last_refresh.elapsed().as_secs();
    let refresh_in = 5u64.saturating_sub(elapsed);

    let (bar_title, bar_text, bar_style) = match &app.state {
        AppState::Telescope => (
            " status ",
            app.status_msg.clone(),
            Style::default().fg(Color::Green),
        ),
        AppState::Search => (
            " search ",
            format!("{}_", app.filter),
            Style::default().fg(Color::White),
        ),
        AppState::ConfirmKill => (
            " confirm ",
            if let Some(p) = app.selected() {
                format!("Kill {} ({})? y=yes  n/Esc=cancel", p.command, p.pid)
            } else {
                String::new()
            },
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        AppState::Normal => (
            " status ",
            app.status_msg.clone(),
            Style::default().fg(Color::Green),
        ),
    };

    let status_text = format!("{bar_text} [refresh in {refresh_in}s]");
    let status = Paragraph::new(status_text)
        .style(bar_style)
        .block(Block::default().borders(Borders::ALL).title(bar_title));
    f.render_widget(status, chunks[3]);

    // ── help bar ──────────────────────────────────────────────────
    let help_spans: Vec<Span> = vec![
        ("↑↓/ji", " nav"),
        ("/", " search"),
        ("k→y", " kill"),
        ("s/S", " sort"),
        ("e", " estab"),
        ("c", " copy"),
        ("f", " telescope"),
        ("r", " refresh"),
        ("q", " quit"),
    ]
    .into_iter()
    .flat_map(|(key, desc)| {
        vec![
            Span::styled(key, Style::default().fg(Color::Yellow)),
            Span::styled(desc, Style::default().fg(Color::DarkGray)),
            Span::raw("  "),
        ]
    })
    .collect();

    f.render_widget(Paragraph::new(Line::from(help_spans)), chunks[4]);

    // ── telescope overlay ─────────────────────────────────────────
    if app.state == AppState::Telescope {
        let buf = f.buffer_mut();
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                buf[(x, y)].set_style(
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::DIM),
                );
            }
        }
        let popup = centered_rect(70, 75, area);
        f.render_widget(Clear, popup);

        let pop_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(popup);

        let query_text = format!(" > {}_", app.tel_filter);
        let query = Paragraph::new(query_text)
            .style(Style::default().fg(Color::White))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan))
                    .title(format!(
                        " telescope — {} matches ",
                        app.tel_results.len()
                    )),
            );
        f.render_widget(query, pop_chunks[0]);

        let tel_items: Vec<ListItem> = app
            .tel_results
            .iter()
            .map(|p| {
                let locked = is_protected(&p.command, &p.user);
                let fg = |c: Color| {
                    if locked {
                        Style::default().fg(Color::DarkGray)
                    } else {
                        Style::default().fg(c)
                    }
                };
                ListItem::new(Line::from(vec![
                    Span::styled(col(&p.command, 16), fg(Color::Cyan)),
                    Span::styled(format!("{:<7}", p.pid), fg(Color::Yellow)),
                    Span::styled(format!("{:<7}", p.port), fg(Color::White)),
                    Span::styled(col(&p.user, 16), fg(Color::Green)),
                    Span::styled(p.address.clone(), fg(Color::DarkGray)),
                ]))
            })
            .collect();

        let tel_list = List::new(tel_items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan))
                    .title(" results "),
            )
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD))
            .highlight_symbol("> ");

        f.render_stateful_widget(tel_list, pop_chunks[1], &mut app.tel_list);
    }
}
