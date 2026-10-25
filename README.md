# BashBox (custom edition)

[![CI](https://github.com/hollali/bashbox/actions/workflows/ci.yml/badge.svg)](https://github.com/hollali/bashbox/actions/workflows/ci.yml)

A one-command Bash profile and a linutil-style toolbox TUI: tuned `.bashrc`, the Starship prompt with swappable palettes, a Fastfetch system-info banner, one-key app installers, a doctor check, and the Nerd Fonts that make it all render.

Inspired by [ChrisTitusTech/mybash](https://github.com/ChrisTitusTech/mybash) (MIT), built from scratch.

## What you get

- A tuned `.bashrc` with generous history handling, aliases (`gs`, `gc`, `killport`, `extract`, ...), and handy functions
- The **Starship** prompt with a Nord-style two-line layout (`starship.toml`)
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

**Option A — prebuilt binary** (attached to tagged releases, `x86_64` Linux):

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

Use `h/l` to switch tabs, `j/k` to move, `Enter` to run a task, `Esc` to cancel or close, `q` to quit. First-time users: run the **Run doctor** task in the *System* tab to confirm everything installed correctly. Run `bashbox --list` to see every task without a UI, and `sudo -v` before launching so package tasks don't halt on password prompts.

What's inside — four tabs, ~20 tasks with live output in the run pane:

- **Applications** — package-manager-aware installers: Docker, Node.js, Git, GitHub CLI, htop, tmux, ripgrep, bat, jq
- **Bash config** — refresh dotfiles, uninstall, toggle the fastfetch banner, test shell startup
- **Themes** — switch the starship palette (Nord, Dracula, Gruvbox, Catppuccin Mocha, Tokyo Night)
- **System** — update system, clean package cache, run doctor, (re)install all Nerd Fonts

The theme switcher rewrites `~/.config/starship.toml` from the embedded template, so custom tweaks to that file are replaced — keep customisations in the repo copy instead.

## Customising

Edit the files in this repo and re-run `./setup.sh`. The copies live in `~/.local/share/bashbox`, and your dotfiles are symlinks to them.

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
- **rust** — cargo fmt/clippy check, release build, and `bashbox --list` smoke test of the TUI
- **smoke-test** — runs `./setup.sh` on a clean runner, boots an interactive Bash to verify the prompt and banner, then `./uninstall.sh` and confirms the original `.bashrc` is restored
- **release** — on tags (`v*`), builds the TUI binary and attaches it to a GitHub release

## License

MIT. See [LICENSE](LICENSE).
