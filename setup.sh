#!/usr/bin/env bash
set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_DIR="$HOME/.local/share/bashbox"
NERD_FONTS=("JetBrainsMono" "FiraCode" "CascadiaCode" "Hack")
BACKUP_SUFFIX=".bashbox-backup"

info() { echo "[*] $*"; }
warn() { echo "[!] $*" >&2; }

# Allow running as root (for example inside a test container)
if [ "$(id -u)" -eq 0 ]; then sudo() { "$@"; }; fi

# --- Detect package manager ---
PM=unknown
for pm in apt dnf pacman zypper; do
  if command -v "$pm" >/dev/null 2>&1; then PM="$pm"; break; fi
done
if [ "$PM" = unknown ]; then warn "No supported package manager found."; exit 1; fi

install_pkg() {
  case "$PM" in
    apt)    sudo apt-get install -y "$@" ;;
    dnf)    sudo dnf install -y "$@" ;;
    pacman) sudo pacman -S --needed --noconfirm "$@" ;;
    zypper) sudo zypper --non-interactive install "$@" ;;
  esac
}

# --- 1. Dependencies ---
info "Installing dependencies..."
if [ "$PM" = apt ]; then sudo apt-get update; fi
for pkg in git curl unzip rsync bash-completion neovim fzf zoxide trash-cli psmisc; do
  install_pkg "$pkg" || warn "Could not install $pkg (skipping)"
done

# --- 2. Fastfetch ---
install_fastfetch() {
  command -v fastfetch >/dev/null 2>&1 && return 0
  info "Installing fastfetch..."
  install_pkg fastfetch && return 0
  if [ "$PM" = apt ]; then
    sudo apt-get install -y software-properties-common
    sudo add-apt-repository -y ppa:zhangsongcui3371/fastfetch
    sudo apt-get update
    install_pkg fastfetch
  else
    return 1
  fi
}
install_fastfetch || warn "Install fastfetch manually: https://github.com/fastfetch-cli/fastfetch/releases"

# --- 3. Starship ---
if ! command -v starship >/dev/null 2>&1; then
  info "Installing Starship..."
  curl -sS https://starship.rs/install.sh | sh -s -- -y
fi

# --- 4. Nerd Fonts ---
# Edit NERD_FONTS above to install more or fewer fonts.
install_font() {
  local font="$1"
  local font_dir="$HOME/.local/share/fonts/${font}NF"
  if [ -d "$font_dir" ]; then return 0; fi
  info "Installing ${font} Nerd Font..."
  mkdir -p "$font_dir"
  curl -fsSL -o "/tmp/${font}.zip" \
    "https://github.com/ryanoasis/nerd-fonts/releases/latest/download/${font}.zip" \
    || { warn "Could not download ${font} Nerd Font (skipping)"; return 1; }
  unzip -oq "/tmp/${font}.zip" -d "$font_dir" || warn "Could not unzip ${font} Nerd Font (skipping)"
  rm -f "/tmp/${font}.zip"
}
for font in "${NERD_FONTS[@]}"; do
  install_font "$font" || true
done
fc-cache -f || true

# --- 5. Copy config into place ---
info "Copying config to $INSTALL_DIR..."
mkdir -p "$INSTALL_DIR/fastfetch"
cp -f "$REPO_DIR/.bashrc" "$REPO_DIR/starship.toml" "$INSTALL_DIR/"
cp -f "$REPO_DIR/fastfetch/config.jsonc" "$INSTALL_DIR/fastfetch/"

# --- 6. Symlink (back up anything that already exists) ---
link() {
  local src="$1" dest="$2" prev
  mkdir -p "$(dirname "$dest")"
  if [ -e "$dest" ] && [ ! -L "$dest" ]; then
    mv "$dest" "$dest$BACKUP_SUFFIX"
    info "Backed up $dest"
  fi
  if [ -L "$dest" ]; then
    prev="$(readlink "$dest" || true)"
    if [ "$prev" != "$src" ]; then
      warn "Replacing existing symlink $dest -> $prev (target not backed up)"
    fi
  fi
  ln -sf "$src" "$dest"
}
link "$INSTALL_DIR/.bashrc"                "$HOME/.bashrc"
link "$INSTALL_DIR/starship.toml"          "$HOME/.config/starship.toml"
link "$INSTALL_DIR/fastfetch/config.jsonc" "$HOME/.config/fastfetch/config.jsonc"

info "Done! Set your terminal font to one of the installed 'Nerd Font' variants (e.g. 'JetBrainsMono Nerd Font'), then open a new terminal."
