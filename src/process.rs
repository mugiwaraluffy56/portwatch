use std::process::Command;

pub const PROTECTED_NAMES: &[&str] = &[
    "launchd", "kernel_task", "WindowServer", "loginwindow", "Finder",
    "Dock", "SystemUIServer", "cfprefsd", "bluetoothd", "coreaudiod",
    "diskarbitrationd", "configd", "powerd", "securityd", "trustd",
    "opendirectoryd", "mDNSResponder", "notifyd", "syslogd", "logd",
    "UserEventAgent", "distnoted", "lsd", "coreduetd", "corespotlightd",
    "ControlStrip", "AirPlayUIAgent", "WiFiAgent", "Raycast", "raycast",
];

#[derive(Debug, Clone)]
pub struct Process {
    pub command: String,
    pub pid: u32,
    pub user: String,
    pub proto: String,
    pub address: String,
    pub port: u16,
    pub conn_state: String,
}

pub fn is_protected(command: &str, user: &str) -> bool {
    if user == "root" || user.starts_with('_') {
        return true;
    }
    PROTECTED_NAMES
        .iter()
        .any(|p| command.to_lowercase().contains(&p.to_lowercase()))
}

pub fn fetch_processes(show_established: bool) -> Vec<Process> {
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
