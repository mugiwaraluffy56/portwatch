# portwatch

A terminal UI for watching what's listening on your machine - see every open port, who owns it, and kill it if you need to. No more `lsof -i` pipelines.

<!-- screenshot -->

---

## What it does

- **Live port list** - shows every process currently listening or connected, auto-refreshed every 5 seconds
- **Process details** - CPU, memory, uptime, and full command line for whatever you've selected
- **Kill by port** - navigate to a process, press `k`, confirm, done
- **Search & filter** - `/` to filter inline, `f` for a fuzzy telescope picker
- **Sort** - cycle columns (`s`) and flip direction (`S`)
- **Toggle connections** - `e` to show established connections alongside listeners
- **Copy to clipboard** - `c` copies the PID + address of the selected process

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

macOS only - uses `lsof` and `pbcopy` under the hood.
