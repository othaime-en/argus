# ARGUS - All-Seeing Pipeline Monitor

```
    ╔═══════════════════════════════════════════════════╗
    ║                                                           ║
    ║     █████╗ ██████╗  ██████╗ ██╗   ██╗ ███████╗       ║
    ║    ██╔══██╗██╔══██╗██╔════╝ ██║   ██║ ██╔════╝       ║
    ║    ███████║██████╔╝██║  ███╗██║   ██║ ███████╗       ║
    ║    ██╔══██║██╔══██╗██║   ██║██║   ██║ ╚════██║       ║
    ║    ██║  ██║██║  ██║╚██████╔╝╚██████╔╝ ███████║       ║
    ║    ╚═╝  ╚═╝╚═╝  ╚═╝ ╚═════╝  ╚═════╝  ╚══════╝       ║
    ║                                                           ║
    ║             All-Seeing Pipeline Monitor                   ║
    ╚═══════════════════════════════════════════════════╝
```

**A fast, intuitive TUI dashboard for monitoring GitHub Actions workflows in real-time.**

Named after Argus Panoptes—the all-seeing giant from Greek mythology with a hundred eyes—this tool helps you watch over all your pipelines simultaneously from your terminal.

## Features

- **Real-time Monitoring**: Automatically polls GitHub Actions every 30 seconds
- **Multi-Repository Support**: Monitor workflows across multiple repositories
- **Stage Details**: View individual job status and timing within each workflow
- **Log Viewing**: Access complete job logs without leaving your terminal
- **Status Indicators**: Color-coded status badges for quick visual scanning
- **Keyboard Navigation**: Fully keyboard-driven interface, no mouse required
- **Error Tracking**: Built-in error panel to track API failures and issues

## Installation

### Prerequisites

- Rust 1.70 or higher
- A GitHub personal access token with `repo` and `workflow` scopes

### From Source

```bash
git clone https://github.com/othaime-en/argus.git
cd argus
cargo build --release
./target/release/argus
```

Or install directly:

```bash
cargo install --path .
```

## Quick Start

### 1. Create a GitHub Token

1. Go to GitHub Settings → Developer settings → Personal access tokens
2. Generate a new token with `repo` and `workflow` scopes
3. Copy the token

### 2. Configure ARGUS

Create a configuration file at `~/.config/argus/config.toml`:

```toml
refresh_interval = 30

[ui]
theme = "default"

[[sources]]
name = "my-projects"
type = "github"
token_env = "GITHUB_TOKEN"
owner = "your-github-username"
repos = ["repo1", "repo2", "repo3"]
```

### 3. Set Your Token

```bash
export GITHUB_TOKEN="your_token_here"
```

### 4. Run ARGUS

```bash
argus
```

## Usage

### Keyboard Controls

#### Pipeline List
- `↑`/`k` - Move up
- `↓`/`j` - Move down
- `Enter` - View pipeline details
- `r` - Force refresh all pipelines
- `e` - Toggle error panel
- `q` - Quit

#### Details Panel
- `↑`/`k` - Navigate stages
- `↓`/`j` - Navigate stages
- `l` - Load logs for selected stage
- `←`/`h` - Return to pipeline list

#### Log Viewer
- `↑`/`k` - Scroll up one line
- `↓`/`j` - Scroll down one line
- `PgUp` - Scroll up one page
- `PgDn` - Scroll down one page
- `Home` - Jump to top
- `End` - Jump to bottom
- `Esc` - Close log viewer

## Configuration

ARGUS looks for configuration in these locations (in order):

1. `config/default.toml` (shipped with the application)
2. `~/.config/argus/config.toml` (user configuration)
3. Environment variables with `ARGUS_` prefix

See `config/example.toml` for a complete configuration reference.

### Multiple Sources

You can monitor multiple GitHub organizations or users:

```toml
[[sources]]
name = "work-projects"
type = "github"
token_env = "GITHUB_TOKEN"
owner = "my-company"
repos = ["api", "web", "mobile"]

[[sources]]
name = "personal-projects"
type = "github"
token_env = "GITHUB_TOKEN"
owner = "myusername"
repos = ["side-project"]
```

## Roadmap

- [x] **v0.1.0**: GitHub Actions support with real-time monitoring
- [ ] **v0.2.0**: GitLab CI and Jenkins integration
- [ ] **v0.3.0**: Search, filtering, and notification system
- [ ] **v0.4.0**: Historical data tracking and trend analysis
- [ ] **v1.0.0**: Advanced features and production polish

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

MIT License - see [LICENSE](LICENSE) for details.

---

**"Vigilo, Ergo Sum"** - I watch, therefore I am.