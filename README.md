# BashBox (custom edition)

[![CI](https://github.com/hollali/bashbox/actions/workflows/ci.yml/badge.svg)](https://github.com/hollali/bashbox/actions/workflows/ci.yml)

A one-command Bash profile: tuned `.bashrc`, the Starship prompt with a Nord palette, a Fastfetch system-info banner, and zoxide/fzf/Neovim wired in.

Inspired by [ChrisTitusTech/bashbox](https://github.com/ChrisTitusTech/bashbox) (MIT), built from scratch.

## What you get

- A tuned `.bashrc` with generous history handling, aliases (`gs`, `gc`, `killport`, `extract`, ...), and handy functions
- The **Starship** prompt with a Nord-style two-line layout (`starship.toml`)
- **Fastfetch** as a system info banner on every new terminal, with its own config, an `ff` alias, and an opt-out switch (`export BASHBOX_NO_FETCH=1`; it also stays off inside VS Code)
- **Zoxide**, **fzf**, and **Neovim** wired in when available
- **Nerd Fonts** installed so prompt icons render: JetBrainsMono, FiraCode, CascadiaCode, and Hack (edit the `NERD_FONTS` list in `setup.sh` to add or remove fonts)
- `setup.sh` and `uninstall.sh` that back up and restore your previous config

## Toolbox (TUI)

bashbox ships a Rust terminal UI that turns the installer into a small toolbox, linutil-style:

```bash
cargo install --locked --path tui
bashbox
```

Or grab a prebuilt binary from the [releases page](https://github.com/hollali/bashbox/releases).

Four tabs, ~20 tasks driven by the arrow keys, with live output scrolling in the run pane:

- **Applications** — package-manager-aware installers: Docker, Node.js, Git, GitHub CLI, htop, tmux, ripgrep, bat, jq
- **Bash config** — refresh dotfiles, uninstall, toggle the fastfetch banner, test shell startup
- **Themes** — switch the starship palette (Nord, Dracula, Gruvbox, Catppuccin Mocha, Tokyo Night)
- **System** — update system, clean package cache, run doctor, (re)install all Nerd Fonts

Handy keys: `h/l` switch tabs, `j/k` move, `Enter` runs a task, `Esc` cancels a running task or closes the output pane, `q` quits. Scripted tasks ask for confirmation first. Tip: run `sudo -v` before launching so package tasks don't halt on password prompts. On headless systems, `bashbox --list` prints every task without a UI.

The theme switcher rewrites `~/.config/starship.toml` from the embedded template, so custom tweaks to that file are replaced — keep customisations in the repo copy instead.

## Install

```bash
git clone --depth=1 https://github.com/hollali/bashbox.git
cd bashbox
./setup.sh
```

The script detects your package manager (apt, dnf, pacman, or zypper) and installs dependencies one at a time, so a single missing package never aborts the run. It installs Fastfetch (with a PPA fallback on Ubuntu), Starship, and the Nerd Font, then symlinks the configs into place — existing files are renamed with a `.bashbox-backup` suffix first.

Afterwards, set your terminal font to one of the installed Nerd Fonts (e.g. **JetBrainsMono Nerd Font**) and open a new terminal.

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
