# portwatch 🦀

A terminal UI for watching what's listening on your machine. See every open port, who owns it, and kill it if you need to. No more `lsof -i` pipelines.

<!-- screenshot -->
<img width="1004" height="722" alt="image" src="https://github.com/user-attachments/assets/f09e3880-98af-463d-ae58-6e954c4cedeb" />
<img width="1004" height="788" alt="image" src="https://github.com/user-attachments/assets/366b7238-8235-4c17-93a0-b6adb324a2bc" />

---

## What it does

- **Live port list** shows every process currently listening or connected, auto-refreshed every 5 seconds
- **Process details** gives you CPU, memory, uptime, and full command line for whatever you've selected
- **Kill by port** lets you navigate to a process, press `k`, confirm, done
- **Search & filter** with `/` to filter inline or `f` for a fuzzy telescope picker
- **Sort** by cycling columns (`s`) and flipping direction (`S`)
- **Toggle connections** with `e` to show established connections alongside listeners
- **Copy to clipboard** with `c` to copy the PID + address of the selected process

Protected system processes (launchd, WindowServer, kernel tasks, etc.) are marked `!` and blocked from killing.

---

## Install

**Prerequisites:** Rust toolchain ([rustup.rs](https://rustup.rs))

```sh
git clone https://github.com/mugiwaraluffy56/portwatch
cd portwatch
cargo install --path .
```

Binary installs as `pwh`.

```sh
pwh
```

---

## Keybindings

| Key       | Action                        |
|-----------|-------------------------------|
| `j` / `↓` | Move down                    |
| `i` / `↑` | Move up                      |
| `k`       | Kill selected process         |
| `y`       | Confirm kill                  |
| `n` / Esc | Cancel                        |
| `/`       | Search / filter               |
| `f`       | Telescope fuzzy picker        |
| `s`       | Cycle sort column             |
| `S`       | Toggle sort direction         |
| `e`       | Toggle established connections|
| `c`       | Copy PID + address            |
| `r`       | Force refresh                 |
| `q`       | Quit                          |

---

## Platform

macOS only. Uses `lsof` and `pbcopy` under the hood.
