# BashBox (custom edition)

[![CI](https://github.com/hollali/bashbox/actions/workflows/ci.yml/badge.svg)](https://github.com/hollali/bashbox/actions/workflows/ci.yml)

A one-command Bash profile and a linutil-style toolbox TUI: tuned `.bashrc`, the Starship prompt with swappable palettes, a Fastfetch system-info banner, one-key app installers, a doctor check, and the Nerd Fonts that make it all render. The toolbox ships focused tabs for developers, cloud engineers, security professionals, and DevOps/ICT work, with searchable tasks, install-status badges, mouse support, and your own custom tasks.

Inspired by [ChrisTitusTech/mybash](https://github.com/ChrisTitusTech/mybash) (MIT), built from scratch.

## What you get

- A tuned `.bashrc` with generous history handling, git aliases, smarter `ls`/`ll`/`tree` via eza when available, and helpers like `port`, `serve`, `gclone`, `gitroot`, `path`, `killport`, and `extract`
- An `update()` helper that upgrades packages non-interactively across apt, dnf, pacman, and zypper
- The **Starship** prompt with a Nord-style two-line layout (`starship.toml`); your username is always visible and the hostname appears over SSH
- **Fastfetch** as a system info banner on every new terminal, with its own config, an `ff` alias, and an opt-out switch (`export BASHBOX_NO_FETCH=1`; it also stays off inside VS Code)
- **Zoxide**, **fzf**, and **Neovim** wired in when available
- **Nerd Fonts** installed so prompt icons render: JetBrainsMono, FiraCode, CascadiaCode, and Hack (edit the `NERD_FONTS` list in `setup.sh` to add or remove fonts)
- `setup.sh` and `uninstall.sh` that back up and restore your previous config

## Install & run

The project has two parts — install the shell profile, optionally install the toolbox TUI.

### 1. Shell profile (the install script)

```bash
git clone --depth=1 https://github.com/hollali/bashbox.git
cd bashbox
./setup.sh
```

What happens: the script detects your package manager (apt, dnf, pacman, or zypper), installs all dependencies, Starship, Fastfetch, and the Nerd Fonts, then symlinks the configs into place. Existing files are renamed with a `.bashbox-backup` suffix first, so nothing is lost.

Two steps after installing:

1. Set your terminal font to one of the installed Nerd Fonts (e.g. **JetBrainsMono Nerd Font**) — otherwise prompt icons render as boxes.
2. Open a new terminal. You should see the Fastfetch banner and the two-line Nord prompt.

Verify everything: run `./uninstall.sh` to fully remove (restores backups), or use the toolbox `doctor` task below.

### 2. Toolbox (terminal UI)

**Option A — prebuilt binary** (attached to tagged releases, `x86_64` Linux; publish one first — see below):

```bash
curl -fsSL -o /tmp/bashbox \
  https://github.com/hollali/bashbox/releases/latest/download/bashbox
chmod +x /tmp/bashbox && sudo mv /tmp/bashbox /usr/local/bin/bashbox
```

**Option B — build from source** (needs a Rust toolchain):

```bash
cargo install --locked --path tui
```

**Option C — from a clone** (no install, build and run in place):

```bash
cd tui && cargo run --release
```

Then run:

```bash
bashbox
```

Navigate with `1`-`9` or `h`/`l` to switch sections, `j`/`k` (or arrows) to move, `/` to search, `Enter` to run, `Esc` to cancel/close (or clear a search), and `q` to quit. Press `?` for the in-app help, `p` to pin/unpin a task, `c` to copy the selected command, `r` to re-run the last task, `[`/`]` to resize the sections pane, `y` to copy run output, `w` to save it to the state directory, and `/` in the run pane to filter output. Wide terminals show a details pane with the command preview, status, timeout, and theme swatches. Mouse users can click a section or task, double-click a task to run it, and scroll the task list. First-time users: run the **Run doctor** task in the *System* tab to confirm everything installed correctly. Run `bashbox --list` to see every task without a UI, `bashbox --doctor` to run the checks non-interactively, `bashbox --completions bash` to install shell completions, `bashbox --help` for all flags, and `sudo -v` before launching so package tasks don't halt on password prompts.

What's inside — nine tabs, ~150 built-in tasks with live output in the run pane:

- **Applications** — package-manager-aware installers: Docker, Node.js, Git, GitHub CLI, htop, tmux, ripgrep, bat, jq, Python 3, Go, Neovim, Rust, Docker Compose, kubectl, Helm, Starship, Fastfetch, eza, btop, fd, tree, wget, shellcheck, Zoxide, fzf, trash-cli, and HTTPie, each with an installed/missing badge
- **Bash config** — refresh dotfiles, uninstall, toggle the fastfetch banner, test shell startup, reset toolbox state, and edit `.bashrc`/Starship/Fastfetch/custom-task configs in `$EDITOR`
- **Themes** — switch the starship palette (Nord, Dracula, Gruvbox, Catppuccin Mocha, Tokyo Night, Ayu, Kanagawa, Everblush, Cyberpunk, Material, Sonokai, Solarized, One Half Light, Nordic, Catppuccin Latte, Catppuccin Frappe, Catppuccin Caramel, One Dark, Base16 Default Dark, Base16 Ocean, Adwaita)
- **System** — update system, clean package cache, Docker cleanup, journal vacuuming, font-cache refresh, run doctor, (re)install all Nerd Fonts (with an `n/4 fonts` badge)
- **Diagnostics** — disk, memory, network, ports, services, environment/WSL/container detection, top processes, uptime, CPU, temperatures, battery, public IP, broken symlinks, ShellCheck, git config, fastfetch preview, shell-history search, service journal/status, HTTP headers, update checks, logs, and backups
- **Dev tools** — lazygit, direnv, just, delta, hyperfine, mkcert, uv, pre-commit, actionlint, and glab installers; project test/lint/dependency-audit runners; git repo summary; TODO/FIXME search; large-file and language-version reports
- **Cloud** — AWS CLI, Azure CLI, Google Cloud CLI, Terraform, k9s, and kubectx installers; AWS identity/profiles, Azure account, GCP auth, Kubernetes contexts/cluster info, Terraform workspace/state, and cloud CLI version checks
- **Security** — nmap, tcpdump, whois, DNS tools, netcat, GnuPG, age, pass, Lynis, Trivy, and gitleaks installers; TLS certificate check, password/UUID generation, file hashing, SSH host-key scan, firewall status, sshd audit, last logins, SSH key inventory, and secret scanning
- **DevOps** — Ansible, ansible-lint, Packer, Podman, kind, and minikube installers; Docker containers/images/disk/contexts, Kubernetes pods/events, Helm releases, Ansible config, Compose status, actionlint, recent GitHub Actions runs, and container runtime info

Package updates are non-interactive across apt, dnf, pacman, and zypper; `Update system` no longer runs `autoremove` (use **Clean package cache** when you want that). The theme switcher only replaces the `palette = '...'` line and the `[palettes]` block of `~/.config/starship.toml`; every other section and any custom tweak is left as-is (fallback: if the file has no `palette` line, it's written fresh from the embedded template). If a task exits non-zero, its output is written to `~/.local/state/bashbox/last-task.log` and the run pane shows the path. Favorites, recent runs, and the pane width persist in `~/.local/state/bashbox/state.toml`.

## Customising

Edit the files in this repo and re-run `./setup.sh`. The copies live in `~/.local/share/bashbox`, and your dotfiles are symlinks to them.

## Custom tasks

Add your own commands to the toolbox with `~/.config/bashbox/tasks.toml` (or point `BASHBOX_TASKS` at another file). You can also drop additional `*.toml` files into `~/.config/bashbox/tasks.d/`; they are loaded in sorted order.

```toml
[[tasks]]
name = "Disk usage"
description = "Show free space on mounted filesystems"
command = "df -h"
section = "System"   # Applications, Bash config, Themes, System, Diagnostics,
                     # Dev tools, Cloud, Security, or DevOps
check = "df"         # optional: show installed/missing for this binary

[[tasks]]
name = "Ping host"
script = "ping -c 1 {}"   # {} is replaced with the --arg value / prompt input
prompt = "Host to ping"
timeout = 10              # optional seconds before the task is killed
destructive = true        # optional extra confirmation
tab = 4                   # or use tab = 0..8 instead of section
```

Custom commands run through the same `$SUDO` wrapper as built-ins, so `sudo` works (and is cached up front). Use `check = "fonts"` for the Nerd Fonts badge. Prompt values are shell-quoted before substitution. An invalid file does not stop the toolbox: it appears as **Custom tasks: config problem** under *System*, and the error is shown in the run pane. `bashbox --list` includes custom tasks too.

## Non-interactive use

```bash
bashbox --list                 # numbered text list of every task
bashbox --list --json          # machine-readable list
bashbox --run "Disk usage"     # run one task through the normal task runner
bashbox --run "Ping host" --arg example.com
bashbox --run "Update system" --dry-run
bashbox --recent               # recently run tasks
bashbox --logs                 # last failed task log
bashbox --fetch-tasks URL      # download + validate a task TOML into tasks.d
bashbox --completions bash     # print completion script (bash, zsh, fish)
bashbox --doctor
```

`--run` accepts a task name or its 1-based position. `--dry-run` prints the command without executing it. Download URLs are validated as task TOML before being saved.

Install completions for your shell:

```text
# Bash
bashbox --completions bash | sudo tee /etc/bash_completion.d/bashbox >/dev/null

# Zsh (on your fpath)
bashbox --completions zsh > "${fpath[1]}/_bashbox"

# Fish
bashbox --completions fish > ~/.config/fish/completions/bashbox.fish
```

## Uninstall

```bash
./uninstall.sh
```

Restores any backups it made and removes the installed copies. Packages (starship, fastfetch, fzf, ...) are left in place.

## Fastfetch tips

- Run it any time with `ff`
- Turn the banner off for one session with `export BASHBOX_NO_FETCH=1`
- Try other logos: `fastfetch --list-logos`, then `fastfetch -l arch`
- Preview the custom config without installing: `fastfetch -c ./fastfetch/config.jsonc`

## CI

GitHub Actions runs on every push and PR:

- **lint** — ShellCheck on all shell files, plus TOML/JSON validation of `starship.toml` and `fastfetch/config.jsonc`
- **rust** — cargo fmt/clippy check, unit tests, release build, and `bashbox --list`/`--doctor`/`--help`/`--completions` smoke tests of the TUI
- **smoke-test** — runs `./setup.sh` on a clean runner, boots an interactive Bash to verify the prompt and banner, then `./uninstall.sh` and confirms the original `.bashrc` is restored
- **release** — on tags (`v*`), builds the TUI binary plus a SHA-256 checksum and attaches both to a GitHub release

To publish a release:

```bash
git tag v0.1.0 && git push origin v0.1.0
```

## License

MIT. See [LICENSE](LICENSE).
