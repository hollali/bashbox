# ~/.bashrc - BashBox (custom edition)
# shellcheck shell=bash

# Only run in interactive shells
[[ $- != *i* ]] && return

# ---------- History ----------
export HISTSIZE=50000
export HISTFILESIZE=100000
export HISTCONTROL=ignoreboth:erasedups
export HISTTIMEFORMAT='%F %T '
export HISTIGNORE='ls:ll:la:cd:exit:clear:history'
shopt -s histappend
PROMPT_COMMAND="history -a${PROMPT_COMMAND:+; $PROMPT_COMMAND}"

# ---------- Shell behaviour ----------
shopt -s checkwinsize autocd cdspell dirspell globstar nocaseglob
bind "set completion-ignore-case on"
bind "set show-all-if-ambiguous on"

# ---------- Environment ----------
export PATH="$HOME/.local/bin:$PATH"
if command -v nvim >/dev/null 2>&1; then export EDITOR=nvim VISUAL=nvim; fi

if [ -f /usr/share/bash-completion/bash_completion ]; then
  # shellcheck disable=SC1091
  . /usr/share/bash-completion/bash_completion
fi

# ---------- Aliases ----------
if command -v eza >/dev/null 2>&1; then
  alias ls='eza --color=auto --group-directories-first'
  alias ll='eza -lAh --git --group-directories-first'
  alias la='eza -A --group-directories-first'
  alias tree='eza --tree --level=2'
else
  alias ls='ls --color=auto'
  alias ll='ls -lAh'
  alias la='ls -A'
fi
alias grep='grep --color=auto'
alias ..='cd ..'
alias ...='cd ../..'
alias cp='cp -i'
alias mv='mv -i'
alias c='clear'
alias df='df -h'
alias ports='ss -tulpn'
alias reload='source ~/.bashrc'
alias bashrc='${EDITOR:-nano} ~/.bashrc'
alias ff='fastfetch'
alias gs='git status'
alias ga='git add'
alias gaa='git add -A'
alias gc='git commit -m'
alias gp='git push'
alias gl='git log --oneline --graph --decorate -15'
alias gd='git diff'
alias gco='git checkout'
alias gb='git branch'
if command -v nvim >/dev/null 2>&1; then alias vim='nvim'; fi
# Recoverable deletes (needs trash-cli)
if command -v trash-put >/dev/null 2>&1; then alias rm='trash-put'; fi
# Nicer defaults when the optional tools are installed
if command -v bat >/dev/null 2>&1; then
  alias bat='bat --paging=never'
elif command -v batcat >/dev/null 2>&1; then
  alias bat='batcat --paging=never'
fi
if ! command -v fd >/dev/null 2>&1 && command -v fdfind >/dev/null 2>&1; then
  alias fd='fdfind'
fi

# ---------- Functions ----------
mkcd() {
  mkdir -p "$1" && cd "$1" || return
}

backup() { cp -a "$1" "$1.bak.$(date +%F-%H%M)"; }

cpp() { rsync -ah --info=progress2 "$1" "$2"; }

killport() { fuser -k "${1:?Usage: killport <port>}"/tcp; }

myip() { echo "Local:  $(hostname -I | awk '{print $1}')"; echo "Public: $(curl -s ifconfig.me)"; }

weather() { curl -s "wttr.in/${1:-}?format=3"; }

port() {
  if [ -z "${1:-}" ]; then echo "Usage: port <port>"; return 1; fi
  if command -v ss >/dev/null 2>&1; then
    ss -tulpn 2>/dev/null | grep -E "[:.]${1}[[:space:]]" \
      || echo "Nothing is listening on port $1"
  elif command -v lsof >/dev/null 2>&1; then
    lsof -nP -iTCP:"$1" -sTCP:LISTEN 2>/dev/null \
      || echo "Nothing is listening on port $1"
  else
    echo "Install ss or lsof to inspect ports" >&2
    return 1
  fi
}

serve() {
  local port="${1:-8000}"
  if command -v python3 >/dev/null 2>&1; then
    python3 -m http.server "$port"
  elif command -v python >/dev/null 2>&1; then
    python -m SimpleHTTPServer "$port"
  else
    echo "Python is required for serve()" >&2
    return 1
  fi
}

gclone() {
  if [ -z "${1:-}" ]; then echo "Usage: gclone <repo-url>"; return 1; fi
  git clone "$1" && cd "$(basename "${1%.git}")" || return
}

gitroot() { git rev-parse --show-toplevel 2>/dev/null || echo "Not inside a git repository"; }

path() { tr ':' '\n' <<< "$PATH"; }

extract() {
  if [ -z "${1:-}" ]; then echo "Usage: extract <file>"; return 1; fi
  if [ ! -f "$1" ]; then echo "'$1' is not a file"; return 1; fi
  case "$1" in
    *.tar.bz2|*.tbz2) tar xjf "$1" ;;
    *.tar.gz|*.tgz)   tar xzf "$1" ;;
    *.tar.xz|*.txz)   tar xJf "$1" ;;
    *.tar)            tar xf "$1" ;;
    *.bz2)            bunzip2 "$1" ;;
    *.gz)             gunzip "$1" ;;
    *.xz)             unxz "$1" ;;
    *.zip)            unzip "$1" ;;
    *.rar)            unrar x "$1" ;;
    *.7z)             7z x "$1" ;;
    *) echo "Cannot extract '$1'"; return 1 ;;
  esac
}

update() {
  if command -v apt-get >/dev/null 2>&1; then
    sudo env DEBIAN_FRONTEND=noninteractive apt-get update \
      && sudo env DEBIAN_FRONTEND=noninteractive apt-get upgrade -y \
        -o Dpkg::Options::=--force-confdef -o Dpkg::Options::=--force-confold
  elif command -v dnf >/dev/null 2>&1; then
    sudo dnf upgrade --refresh -y
  elif command -v pacman >/dev/null 2>&1; then
    sudo pacman -Syu --noconfirm
  elif command -v zypper >/dev/null 2>&1; then
    sudo zypper --non-interactive refresh \
      && sudo zypper --non-interactive update
  else
    echo "No supported package manager found" >&2
    return 1
  fi
}

# ---------- Tools ----------
if command -v zoxide >/dev/null 2>&1; then eval "$(zoxide init bash)"; fi

if command -v fzf >/dev/null 2>&1; then
  if fzf --bash >/dev/null 2>&1; then
    eval "$(fzf --bash)"
  elif [ -f /usr/share/doc/fzf/examples/key-bindings.bash ]; then
    # shellcheck disable=SC1091
    . /usr/share/doc/fzf/examples/key-bindings.bash
  fi
fi

if command -v starship >/dev/null 2>&1; then eval "$(starship init bash)"; fi

# ---------- Fastfetch banner ----------
# Turn it off with: export BASHBOX_NO_FETCH=1
if command -v fastfetch >/dev/null 2>&1 && [ -z "${BASHBOX_NO_FETCH:-}" ] && [ "${TERM_PROGRAM:-}" != "vscode" ]; then
  fastfetch
fi
