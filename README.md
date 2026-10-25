# MyBash (custom edition)

[![CI](https://github.com/hollali/mybash/actions/workflows/ci.yml/badge.svg)](https://github.com/hollali/mybash/actions/workflows/ci.yml)

A one-command Bash profile: tuned `.bashrc`, the Starship prompt with a Nord palette, a Fastfetch system-info banner, and zoxide/fzf/Neovim wired in.

Inspired by [ChrisTitusTech/mybash](https://github.com/ChrisTitusTech/mybash) (MIT), built from scratch.

## What you get

- A tuned `.bashrc` with generous history handling, aliases (`gs`, `gc`, `killport`, `extract`, ...), and handy functions
- The **Starship** prompt with a Nord-style two-line layout (`starship.toml`)
- **Fastfetch** as a system info banner on every new terminal, with its own config, an `ff` alias, and an opt-out switch (`export MYBASH_NO_FETCH=1`; it also stays off inside VS Code)
- **Zoxide**, **fzf**, and **Neovim** wired in when available
- **Nerd Fonts** installed so prompt icons render: JetBrainsMono, FiraCode, CascadiaCode, and Hack (edit the `NERD_FONTS` list in `setup.sh` to add or remove fonts)
- `setup.sh` and `uninstall.sh` that back up and restore your previous config

## Install

```bash
git clone --depth=1 https://github.com/hollali/mybash.git
cd mybash
./setup.sh
```

The script detects your package manager (apt, dnf, pacman, or zypper) and installs dependencies one at a time, so a single missing package never aborts the run. It installs Fastfetch (with a PPA fallback on Ubuntu), Starship, and the Nerd Font, then symlinks the configs into place — existing files are renamed with a `.mybash-backup` suffix first.

Afterwards, set your terminal font to one of the installed Nerd Fonts (e.g. **JetBrainsMono Nerd Font**) and open a new terminal.

## Customising

Edit the files in this repo and re-run `./setup.sh`. The copies live in `~/.local/share/mybash`, and your dotfiles are symlinks to them.

## Uninstall

```bash
./uninstall.sh
```

Restores any backups it made and removes the installed copies. Packages (starship, fastfetch, fzf, ...) are left in place.

## Fastfetch tips

- Run it any time with `ff`
- Turn the banner off for one session with `export MYBASH_NO_FETCH=1`
- Try other logos: `fastfetch --list-logos`, then `fastfetch -l arch`
- Preview the custom config without installing: `fastfetch -c ./fastfetch/config.jsonc`

## CI

GitHub Actions runs on every push and PR:

- **lint** — ShellCheck on all shell files, plus TOML/JSON validation of `starship.toml` and `fastfetch/config.jsonc`
- **smoke-test** — runs `./setup.sh` on a clean runner, boots an interactive Bash to verify the prompt and banner, then `./uninstall.sh` and confirms the original `.bashrc` is restored

## License

MIT. See [LICENSE](LICENSE).
