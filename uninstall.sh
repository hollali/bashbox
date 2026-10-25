#!/usr/bin/env bash
set -euo pipefail

INSTALL_DIR="$HOME/.local/share/mybash"
FONT_DIR="$HOME/.local/share/fonts/JetBrainsMonoNF"
BACKUP_SUFFIX=".mybash-backup"

restore() {
  local dest="$1"
  if [ -L "$dest" ]; then rm "$dest"; fi
  if [ -e "$dest$BACKUP_SUFFIX" ]; then
    mv "$dest$BACKUP_SUFFIX" "$dest"
    echo "Restored $dest"
  fi
}

restore "$HOME/.bashrc"
restore "$HOME/.config/starship.toml"
restore "$HOME/.config/fastfetch/config.jsonc"

rm -rf "$INSTALL_DIR" "$FONT_DIR"
fc-cache -f || true

echo "Uninstalled. Installed packages (starship, fastfetch, fzf, ...) were left in place."
echo "Open a new terminal to apply the change."
