# MineTUI

A terminal-based Minecraft launcher written in Rust.

## Features

- Create and manage multiple Minecraft instances
- Browse and install modpacks from [Modrinth](https://modrinth.com)
- Fabric mod loader support
- Three authentication methods: Offline, Microsoft, and Ely.by
- Parallel downloads with SHA1 verification
- Fully keyboard-driven TUI

## Installation

**Requirements:** Rust 1.70+, Java (for running Minecraft)

```sh
git clone https://github.com/yourname/mine-tui
cd mine-tui
cargo build --release
./target/release/mine-tui
```

## Usage

| Key | Action |
|-----|--------|
| `Tab` / `Shift+Tab` | Switch between tabs |
| `j` / `k` or `↑` / `↓` | Navigate lists |
| `Enter` | Select / Launch / Confirm |
| `Esc` | Go back / Cancel |
| `q` | Quit |

### Instances tab
| Key | Action |
|-----|--------|
| `n` | New instance |
| `e` | Edit selected instance |
| `d` | Delete selected instance |
| `Enter` | Launch selected instance |

### Modpacks tab
| Key | Action |
|-----|--------|
| `e` or `/` | Focus search bar |
| `Enter` | Search / select modpack / install version |

## Authentication

- **Offline** — enter a username, no account required
- **Microsoft** — device code flow; a URL and code are shown in the Settings status box, open the URL in a browser and enter the code
- **Ely.by** — enter email and password directly

## Data location

Game files, instances, and config are stored in the platform data directory:

| Platform | Path |
|----------|------|
| Linux | `~/.local/share/mine-tui/` |
| macOS | `~/Library/Application Support/mine-tui/` |
| Windows | `%APPDATA%\mine-tui\` |

Instance game logs are written to `<instance>/latest.log`.

## License

MIT
