#!/usr/bin/env bash
set -euo pipefail

INSTALL_DIR="$HOME/.local/share/bashbox"
NERD_FONTS=("JetBrainsMono" "FiraCode" "CascadiaCode" "Hack")
BACKUP_SUFFIX=".bashbox-backup"

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

rm -rf "$INSTALL_DIR"

for font in "${NERD_FONTS[@]}"; do
  rm -rf "$HOME/.local/share/fonts/${font}NF"
done
fc-cache -f || true

echo "Uninstalled. Installed packages (starship, fastfetch, fzf, ...) were left in place."
echo "Open a new terminal to apply the change."
