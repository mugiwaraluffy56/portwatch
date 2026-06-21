use std::{
    io,
    process::Command,
    time::{Duration, Instant},
};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Terminal,
};

const PROTECTED_NAMES: &[&str] = &[
    "launchd", "kernel_task", "WindowServer", "loginwindow", "Finder",
    "Dock", "SystemUIServer", "cfprefsd", "bluetoothd", "coreaudiod",
    "diskarbitrationd", "configd", "powerd", "securityd", "trustd",
    "opendirectoryd", "mDNSResponder", "notifyd", "syslogd", "logd",
    "UserEventAgent", "distnoted", "lsd", "coreduetd", "corespotlightd",
    "ControlStrip", "AirPlayUIAgent", "WiFiAgent", "Raycast", "raycast",
];

// pad to `w`, truncate with '>' if longer
fn col(s: &str, w: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() > w {
        let truncated: String = chars[..w.saturating_sub(1)].iter().collect();
        format!("{:<w$}", truncated + ">", w = w)
    } else {
        format!("{:<w$}", s, w = w)
    }
}

fn is_protected(command: &str, user: &str) -> bool {
    if user == "root" || user.starts_with('_') {
        return true;
    }
    PROTECTED_NAMES
        .iter()
        .any(|p| command.to_lowercase().contains(&p.to_lowercase()))
}

#[derive(Debug, Clone)]
struct Process {
    command: String,
    pid: u32,
    user: String,
    proto: String,
    address: String,
    port: u16,
    conn_state: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum SortCol {
    Command,
    Pid,
    Port,
    User,
}

impl SortCol {
    fn label(self) -> &'static str {
        match self {
            SortCol::Command => "CMD",
            SortCol::Pid => "PID",
            SortCol::Port => "PORT",
            SortCol::User => "USER",
        }
    }

    fn next(self) -> Self {
        match self {
            SortCol::Port => SortCol::Command,
            SortCol::Command => SortCol::Pid,
            SortCol::Pid => SortCol::User,
            SortCol::User => SortCol::Port,
        }
    }
}

#[derive(Debug, PartialEq)]
enum AppState {
    Normal,
    Search,
    Telescope,
    ConfirmKill,
}

struct App {
    raw: Vec<Process>,
    processes: Vec<Process>,
    list_state: ListState,
    state: AppState,
    status_msg: String,
    last_refresh: Instant,
    filter: String,
    show_established: bool,
    sort_col: SortCol,
    sort_asc: bool,
    detail_cache: Option<(u32, String, String)>,
    tel_filter: String,
    tel_results: Vec<Process>,
    tel_list: ListState,
}

impl App {
    fn new() -> Self {
        let mut app = Self {
            raw: Vec::new(),
            processes: Vec::new(),
            list_state: ListState::default(),
            state: AppState::Normal,
            status_msg: String::new(),
            last_refresh: Instant::now(),
            filter: String::new(),
            show_established: false,
            sort_col: SortCol::Port,
            sort_asc: true,
            detail_cache: None,
            tel_filter: String::new(),
            tel_results: Vec::new(),
            tel_list: ListState::default(),
        };
        app.refresh();
        app
    }

    fn refresh(&mut self) {
        self.raw = fetch_processes(self.show_established);
        self.last_refresh = Instant::now();
        self.apply_filter_sort();
    }

    fn apply_filter_sort(&mut self) {
        let q = self.filter.to_lowercase();
        let mut filtered: Vec<Process> = self
            .raw
            .iter()
            .filter(|p| {
                q.is_empty()
                    || p.command.to_lowercase().contains(&q)
                    || p.address.contains(&q)
                    || p.user.to_lowercase().contains(&q)
                    || p.pid.to_string().contains(&q)
                    || p.port.to_string().contains(&q)
            })
            .cloned()
            .collect();

        let asc = self.sort_asc;
        filtered.sort_by(|a, b| {
            let ord = match self.sort_col {
                SortCol::Command => a.command.cmp(&b.command),
                SortCol::Pid => a.pid.cmp(&b.pid),
                SortCol::Port => a.port.cmp(&b.port),
                SortCol::User => a.user.cmp(&b.user),
            };
            if asc { ord } else { ord.reverse() }
        });

        self.processes = filtered;
        let len = self.processes.len();
        if len == 0 {
            self.list_state.select(None);
        } else {
            let sel = self.list_state.selected().unwrap_or(0).min(len - 1);
            self.list_state.select(Some(sel));
        }
        self.detail_cache = None;
    }

    fn selected(&self) -> Option<&Process> {
        self.list_state.selected().and_then(|i| self.processes.get(i))
    }

    fn do_kill(&mut self) {
        if let Some(p) = self.selected() {
            let pid = p.pid.to_string();
            let name = p.command.clone();
            match Command::new("kill").arg("-9").arg(&pid).output() {
                Ok(o) if o.status.success() => {
                    self.status_msg = format!("Killed {} ({})", name, pid);
                }
                Ok(o) => {
                    self.status_msg = format!(
                        "Kill failed: {}",
                        String::from_utf8_lossy(&o.stderr).trim()
                    );
                }
                Err(e) => self.status_msg = format!("Error: {e}"),
            }
        }
        self.state = AppState::Normal;
        self.refresh();
    }

    fn move_up(&mut self) {
        let len = self.processes.len();
        if len == 0 { return; }
        let i = self.list_state.selected().map(|i| i.saturating_sub(1)).unwrap_or(0);
        self.list_state.select(Some(i));
        self.detail_cache = None;
    }

    fn move_down(&mut self) {
        let len = self.processes.len();
        if len == 0 { return; }
        let i = self.list_state.selected().map(|i| (i + 1).min(len - 1)).unwrap_or(0);
        self.list_state.select(Some(i));
        self.detail_cache = None;
    }

    fn open_telescope(&mut self) {
        self.tel_filter.clear();
        self.tel_results = self.raw.clone();
        self.tel_list = ListState::default();
        if !self.tel_results.is_empty() {
            self.tel_list.select(Some(0));
        }
        self.state = AppState::Telescope;
    }

    fn tel_apply(&mut self) {
        let q = self.tel_filter.to_lowercase();
        self.tel_results = self
            .raw
            .iter()
            .filter(|p| {
                q.is_empty()
                    || p.command.to_lowercase().contains(&q)
                    || p.address.contains(&q)
                    || p.user.to_lowercase().contains(&q)
                    || p.pid.to_string().contains(&q)
                    || p.port.to_string().contains(&q)
            })
            .cloned()
            .collect();
        let len = self.tel_results.len();
        if len == 0 {
            self.tel_list.select(None);
        } else {
            let sel = self.tel_list.selected().unwrap_or(0).min(len - 1);
            self.tel_list.select(Some(sel));
        }
    }

    fn tel_move_up(&mut self) {
        let len = self.tel_results.len();
        if len == 0 { return; }
        let i = self.tel_list.selected().map(|i| i.saturating_sub(1)).unwrap_or(0);
        self.tel_list.select(Some(i));
    }

    fn tel_move_down(&mut self) {
        let len = self.tel_results.len();
        if len == 0 { return; }
        let i = self.tel_list.selected().map(|i| (i + 1).min(len - 1)).unwrap_or(0);
        self.tel_list.select(Some(i));
    }

    // confirm selection: jump main list to the chosen process
    fn tel_confirm(&mut self) {
        if let Some(chosen) = self.tel_list.selected().and_then(|i| self.tel_results.get(i)) {
            let pid = chosen.pid;
            if let Some(idx) = self.processes.iter().position(|p| p.pid == pid) {
                self.list_state.select(Some(idx));
                self.detail_cache = None;
            }
        }
        self.state = AppState::Normal;
        self.tel_filter.clear();
    }

    fn toggle_established(&mut self) {
        self.show_established = !self.show_established;
        self.refresh();
        self.status_msg = if self.show_established {
            "Showing LISTEN + ESTABLISHED".to_string()
        } else {
            "Showing LISTEN only".to_string()
        };
    }

    fn copy_to_clipboard(&mut self) {
        if let Some(p) = self.selected() {
            let text = format!("{} {}", p.pid, p.address);
            let _ = Command::new("sh")
                .arg("-c")
                .arg(format!("echo -n '{text}' | pbcopy"))
                .output();
            self.status_msg = format!("Copied: {text}");
        }
    }

    fn get_details(&mut self) -> (String, String) {
        let pid = match self.selected().map(|p| p.pid) {
            Some(p) => p,
            None => return (String::new(), String::new()),
        };

        if let Some((cached_pid, ref cmd, ref stats)) = self.detail_cache {
            if cached_pid == pid {
                return (cmd.clone(), stats.clone());
            }
        }

        let pid_str = pid.to_string();

        let cmdline = Command::new("ps")
            .args(["-p", &pid_str, "-o", "args="])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();

        let stats = Command::new("ps")
            .args(["-p", &pid_str, "-o", "%cpu=,%mem=,etime="])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();

        let parts: Vec<&str> = stats.split_whitespace().collect();
        let formatted = match parts.as_slice() {
            [cpu, mem, elapsed, ..] => format!("cpu {cpu}%  mem {mem}%  up {elapsed}"),
            _ => stats.clone(),
        };

        self.detail_cache = Some((pid, cmdline.clone(), formatted.clone()));
        (cmdline, formatted)
    }
}

fn fetch_processes(show_established: bool) -> Vec<Process> {
    let out = Command::new("lsof")
        .args(["-i", "-P", "-n"])
        .output()
        .ok();

    let output = match out {
        Some(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        None => return Vec::new(),
    };

    output
        .lines()
        .filter(|line| {
            line.contains("LISTEN")
                || (show_established && line.contains("ESTABLISHED"))
        })
        .filter_map(parse_lsof_line)
        .collect()
}

fn parse_lsof_line(line: &str) -> Option<Process> {
    let cols: Vec<&str> = line.split_whitespace().collect();
    if cols.len() < 9 { return None; }

    let command = cols[0].to_string();
    let pid: u32 = cols[1].parse().ok()?;
    let user = cols[2].to_string();
    let name = cols[8];

    let conn_state = if name.contains("(LISTEN)") {
        "LISTEN"
    } else if name.contains("(ESTABLISHED)") {
        "ESTAB"
    } else {
        ""
    }
    .to_string();

    let proto = if name.starts_with('[') || cols.get(7).map(|s| s.contains('6')).unwrap_or(false) {
        "TCP6"
    } else {
        "TCP"
    }
    .to_string();

    let address = name
        .replace("(LISTEN)", "")
        .replace("(ESTABLISHED)", "")
        .trim()
        .to_string();

    let port: u16 = address
        .rsplit(':')
        .next()
        .and_then(|p| p.split("->").next())
        .and_then(|p| p.parse().ok())
        .unwrap_or(0);

    Some(Process { command, pid, user, proto, address, port, conn_state })
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press { continue; }

                match app.state {
                    AppState::Normal => match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                        KeyCode::Char('j') | KeyCode::Down => app.move_down(),
                        KeyCode::Up | KeyCode::Char('i') => app.move_up(),
                        KeyCode::Char('k') => {
                            if let Some(p) = app.selected() {
                                if is_protected(&p.command.clone(), &p.user.clone()) {
                                    app.status_msg =
                                        format!("Protected: {} cannot be killed", p.command);
                                } else {
                                    app.state = AppState::ConfirmKill;
                                    app.status_msg = String::new();
                                }
                            }
                        }
                        KeyCode::Char('r') => {
                            app.refresh();
                            app.status_msg = "Refreshed".to_string();
                        }
                        KeyCode::Char('/') => {
                            app.state = AppState::Search;
                        }
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
                        KeyCode::Esc | KeyCode::Enter => {
                            app.state = AppState::Normal;
                        }
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

fn ui(f: &mut ratatui::Frame, app: &mut App) {
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
    let sort_arrow = if app.sort_asc { "▲" } else { "▼" };
    let sort_lbl = app.sort_col.label();

    fn col_style(col: SortCol, active: SortCol, asc: bool) -> (Style, &'static str) {
        if col == active {
            (Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
             if asc { "▲" } else { "▼" })
        } else {
            (Style::default().fg(Color::DarkGray), "")
        }
    }

    let _ = (sort_arrow, sort_lbl); // used below via col_style

    let (cmd_style, cmd_arr) = col_style(SortCol::Command, app.sort_col, app.sort_asc);
    let (pid_style, pid_arr) = col_style(SortCol::Pid, app.sort_col, app.sort_asc);
    let (usr_style, usr_arr) = col_style(SortCol::User, app.sort_col, app.sort_asc);
    let (prt_style, prt_arr) = col_style(SortCol::Port, app.sort_col, app.sort_asc);

    // 1 (border) + 2 (highlight symbol) + 2 (lock span) = 5 col offset before COMMAND
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

    let elapsed = app.last_refresh.elapsed().as_secs();
    let refresh_in = 5u64.saturating_sub(elapsed);
    let bar_suffix = format!(" [refresh in {refresh_in}s]");
    let status_text = format!("{bar_text}{bar_suffix}");

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
        // dim all rendered cells before drawing popup on top
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
                    if locked { Style::default().fg(Color::DarkGray) } else { Style::default().fg(c) }
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

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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
