use std::{
    process::Command,
    time::Instant,
};

use ratatui::widgets::ListState;

use crate::process::{Process, fetch_processes, is_protected};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SortCol {
    Command,
    Pid,
    Port,
    User,
}

impl SortCol {
    pub fn label(self) -> &'static str {
        match self {
            SortCol::Command => "CMD",
            SortCol::Pid => "PID",
            SortCol::Port => "PORT",
            SortCol::User => "USER",
        }
    }

    pub fn next(self) -> Self {
        match self {
            SortCol::Port => SortCol::Command,
            SortCol::Command => SortCol::Pid,
            SortCol::Pid => SortCol::User,
            SortCol::User => SortCol::Port,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum AppState {
    Normal,
    Search,
    Telescope,
    ConfirmKill,
}

pub struct App {
    pub raw: Vec<Process>,
    pub processes: Vec<Process>,
    pub list_state: ListState,
    pub state: AppState,
    pub status_msg: String,
    pub last_refresh: Instant,
    pub filter: String,
    pub show_established: bool,
    pub sort_col: SortCol,
    pub sort_asc: bool,
    pub detail_cache: Option<(u32, String, String)>,
    pub tel_filter: String,
    pub tel_results: Vec<Process>,
    pub tel_list: ListState,
}

impl App {
    pub fn new() -> Self {
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

    pub fn refresh(&mut self) {
        self.raw = fetch_processes(self.show_established);
        self.last_refresh = Instant::now();
        self.apply_filter_sort();
    }

    pub fn apply_filter_sort(&mut self) {
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

    pub fn selected(&self) -> Option<&Process> {
        self.list_state.selected().and_then(|i| self.processes.get(i))
    }

    pub fn do_kill(&mut self) {
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

    pub fn move_up(&mut self) {
        let len = self.processes.len();
        if len == 0 { return; }
        let i = self.list_state.selected().map(|i| i.saturating_sub(1)).unwrap_or(0);
        self.list_state.select(Some(i));
        self.detail_cache = None;
    }

    pub fn move_down(&mut self) {
        let len = self.processes.len();
        if len == 0 { return; }
        let i = self.list_state.selected().map(|i| (i + 1).min(len - 1)).unwrap_or(0);
        self.list_state.select(Some(i));
        self.detail_cache = None;
    }

    pub fn open_telescope(&mut self) {
        self.tel_filter.clear();
        self.tel_results = self.raw.clone();
        self.tel_list = ListState::default();
        if !self.tel_results.is_empty() {
            self.tel_list.select(Some(0));
        }
        self.state = AppState::Telescope;
    }

    pub fn tel_apply(&mut self) {
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

    pub fn tel_move_up(&mut self) {
        let len = self.tel_results.len();
        if len == 0 { return; }
        let i = self.tel_list.selected().map(|i| i.saturating_sub(1)).unwrap_or(0);
        self.tel_list.select(Some(i));
    }

    pub fn tel_move_down(&mut self) {
        let len = self.tel_results.len();
        if len == 0 { return; }
        let i = self.tel_list.selected().map(|i| (i + 1).min(len - 1)).unwrap_or(0);
        self.tel_list.select(Some(i));
    }

    pub fn tel_confirm(&mut self) {
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

    pub fn toggle_established(&mut self) {
        self.show_established = !self.show_established;
        self.refresh();
        self.status_msg = if self.show_established {
            "Showing LISTEN + ESTABLISHED".to_string()
        } else {
            "Showing LISTEN only".to_string()
        };
    }

    pub fn copy_to_clipboard(&mut self) {
        if let Some(p) = self.selected() {
            let text = format!("{} {}", p.pid, p.address);
            let _ = Command::new("sh")
                .arg("-c")
                .arg(format!("echo -n '{text}' | pbcopy"))
                .output();
            self.status_msg = format!("Copied: {text}");
        }
    }

    pub fn get_details(&mut self) -> (String, String) {
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

    pub fn kill_selected_if_safe(&mut self) {
        if let Some(p) = self.selected() {
            if is_protected(&p.command.clone(), &p.user.clone()) {
                self.status_msg = format!("Protected: {} cannot be killed", p.command);
            } else {
                self.state = AppState::ConfirmKill;
                self.status_msg = String::new();
            }
        }
    }
}
