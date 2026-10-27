use std::env;
use std::fs;
use std::io::{BufRead, BufReader};
use std::os::unix::fs::symlink as fs_symlink;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::thread;

use serde::Deserialize;

use crate::themes::PALETTES;

pub const TABS: [&str; 9] = [
    "Applications",
    "Bash config",
    "Themes",
    "System",
    "Diagnostics",
    "Dev tools",
    "Cloud",
    "Security",
    "DevOps",
];

pub const TAB_SHORT: [&str; 9] = [
    "Apps", "Bash", "Themes", "System", "Diag", "Dev", "Cloud", "Sec", "Ops",
];

const FONTS: [&str; 4] = ["JetBrainsMono", "FiraCode", "CascadiaCode", "Hack"];

#[derive(Clone, Debug)]
pub enum Action {
    Script(String),
    Theme(usize),
    Builtin(Builtin),
    Message(String),
    Edit(PathBuf),
}

#[derive(Clone, Copy, Debug)]
pub enum Builtin {
    Refresh,
    Uninstall,
    ToggleBanner,
    Doctor,
    LastLog,
    Recent,
    ResetState,
    ClearLog,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Check {
    Bin(String),
    Any(Vec<String>),
    Fonts,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum StatusKind {
    Installed,
    Missing,
    Partial,
}

pub struct Status {
    pub kind: StatusKind,
    pub label: String,
}

#[derive(Debug)]
pub struct Task {
    pub name: String,
    pub desc: String,
    pub tab: usize,
    pub action: Action,
    pub check: Option<Check>,
    pub prompt: Option<String>,
    pub timeout: Option<u64>,
    pub destructive: bool,
    pub custom: bool,
}

impl Task {
    fn new(name: &str, desc: &str, tab: usize, action: Action) -> Self {
        Self {
            name: name.to_string(),
            desc: desc.to_string(),
            tab,
            action,
            check: None,
            prompt: None,
            timeout: None,
            destructive: false,
            custom: false,
        }
    }

    fn with_check(mut self, check: Check) -> Self {
        self.check = Some(check);
        self
    }

    fn with_any_check(mut self, bins: &[&str]) -> Self {
        self.check = Some(Check::Any(bins.iter().map(|bin| bin.to_string()).collect()));
        self
    }

    fn with_prompt(mut self, prompt: &str) -> Self {
        self.prompt = Some(prompt.to_string());
        self
    }

    fn with_timeout(mut self, seconds: u64) -> Self {
        self.timeout = Some(seconds);
        self
    }

    fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }

    pub fn command(&self) -> Option<&str> {
        match &self.action {
            Action::Script(command) => Some(command),
            _ => None,
        }
    }

    pub fn command_with_arg(&self, arg: &str) -> Option<String> {
        let command = self.command()?;
        Some(substitute_arg(command, arg))
    }

    pub fn preview(&self) -> String {
        match &self.action {
            Action::Script(command) => command.clone(),
            Action::Theme(idx) => PALETTES
                .get(*idx)
                .map(|p| format!("Switch starship palette to {}", p.name))
                .unwrap_or_else(|| "Switch theme".to_string()),
            Action::Builtin(Builtin::Refresh) => "Re-link bashbox dotfiles".to_string(),
            Action::Builtin(Builtin::Uninstall) => "Restore backups and remove bashbox".to_string(),
            Action::Builtin(Builtin::ToggleBanner) => {
                "Toggle the fastfetch startup banner".to_string()
            }
            Action::Builtin(Builtin::Doctor) => "Run dependency and config checks".to_string(),
            Action::Builtin(Builtin::LastLog) => "Show the last failed task log".to_string(),
            Action::Builtin(Builtin::Recent) => "Show recently run tasks".to_string(),
            Action::Builtin(Builtin::ResetState) => {
                "Clear pinned tasks, recent runs, and pane width".to_string()
            }
            Action::Builtin(Builtin::ClearLog) => "Delete the last failed task log".to_string(),
            Action::Message(text) => text.clone(),
            Action::Edit(path) => format!("Open {} in $EDITOR", path.display()),
        }
    }

    pub fn status(&self) -> Option<Status> {
        match self.check.as_ref()? {
            Check::Bin(bin) => Some(if which(bin) {
                Status {
                    kind: StatusKind::Installed,
                    label: "installed".to_string(),
                }
            } else {
                Status {
                    kind: StatusKind::Missing,
                    label: "missing".to_string(),
                }
            }),
            Check::Any(bins) => Some(if bins.iter().any(|bin| which(bin)) {
                Status {
                    kind: StatusKind::Installed,
                    label: "installed".to_string(),
                }
            } else {
                Status {
                    kind: StatusKind::Missing,
                    label: "missing".to_string(),
                }
            }),
            Check::Fonts => {
                let count = FONTS.iter().filter(|font| font_dir(font).is_dir()).count();
                let kind = match count {
                    4 => StatusKind::Installed,
                    0 => StatusKind::Missing,
                    _ => StatusKind::Partial,
                };
                Some(Status {
                    kind,
                    label: format!("{}/{} fonts", count, FONTS.len()),
                })
            }
        }
    }
}

pub enum EventMsg {
    Pid(u32),
    Line(String),
    Done { failed: bool, log: Option<PathBuf> },
}

fn home_dir() -> Option<PathBuf> {
    env::var("HOME")
        .ok()
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
}

fn font_dir(font: &str) -> PathBuf {
    home_dir()
        .unwrap_or_default()
        .join(format!(".local/share/fonts/{}NF", font))
}

fn which(bin: &str) -> bool {
    let path = env::var("PATH").unwrap_or_default();
    path.split(':')
        .any(|dir| PathBuf::from(dir).join(bin).is_file())
}

pub fn detect_pm() -> &'static str {
    for pm in ["apt", "dnf", "pacman", "zypper"] {
        if which(pm) {
            return pm;
        }
    }
    "unknown"
}

fn script(cmds: &str) -> Action {
    Action::Script(format!(
        "SUDO=; [ \"$(id -u)\" -eq 0 ] || SUDO=sudo; ( {} ) 2>&1",
        cmds
    ))
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn substitute_arg(command: &str, arg: &str) -> String {
    command.replace("{}", &shell_quote(arg))
}

fn pkg(pm: &str, pkgs: &str) -> String {
    match pm {
        "apt" => format!("$SUDO apt-get install -y {}", pkgs),
        "dnf" => format!("$SUDO dnf install -y {}", pkgs),
        "pacman" => format!("$SUDO pacman -S --needed --noconfirm {}", pkgs),
        "zypper" => format!("$SUDO zypper --non-interactive install {}", pkgs),
        _ => "echo 'No supported package manager found'".to_string(),
    }
}

fn pkg_alias(pm: &str, apt: &str, dnf: &str, pacman: &str, zypper: &str) -> String {
    let pkgs = match pm {
        "apt" => apt,
        "dnf" => dnf,
        "pacman" => pacman,
        "zypper" => zypper,
        _ => "",
    };
    if pkgs.is_empty() {
        "echo 'No supported package manager found'".to_string()
    } else {
        pkg(pm, pkgs)
    }
}

fn update_cmd(pm: &str) -> String {
    match pm {
        "apt" => "$SUDO env DEBIAN_FRONTEND=noninteractive NEEDRESTART_MODE=a apt-get update && $SUDO env DEBIAN_FRONTEND=noninteractive NEEDRESTART_MODE=a apt-get -y -o Dpkg::Options::=--force-confdef -o Dpkg::Options::=--force-confold upgrade".to_string(),
        "dnf" => "$SUDO dnf -y upgrade".to_string(),
        "pacman" => "$SUDO pacman -Syu --noconfirm".to_string(),
        "zypper" => "$SUDO zypper --non-interactive refresh && $SUDO zypper --non-interactive update".to_string(),
        _ => "echo 'No supported package manager found'".to_string(),
    }
}

fn cleanup_cmd(pm: &str) -> String {
    match pm {
        "apt" => "$SUDO env DEBIAN_FRONTEND=noninteractive NEEDRESTART_MODE=a apt-get -y autoremove && $SUDO apt-get clean".to_string(),
        "dnf" => "$SUDO dnf clean all".to_string(),
        "pacman" => "$SUDO pacman -Sc --noconfirm".to_string(),
        "zypper" => "$SUDO zypper clean --all".to_string(),
        _ => "echo 'No supported package manager found'".to_string(),
    }
}

pub fn build() -> Vec<Task> {
    let pm = detect_pm();
    let home = home_dir().unwrap_or_default();
    let mut tasks = vec![
        Task::new(
            "Install Docker",
            "Runs the official Docker convenience script and adds you to the docker group. Log out afterwards for the group to apply.",
            0,
            script("curl -fsSL https://get.docker.com | $SUDO sh && $SUDO usermod -aG docker \"$(id -un)\" && echo 'Log out and back in for the docker group to apply.'"),
        )
        .with_check(Check::Bin("docker".to_string())),
        Task::new(
            "Install Node.js",
            "Installs nodejs and npm via your package manager.",
            0,
            script(&pkg(pm, "nodejs npm")),
        )
        .with_check(Check::Bin("node".to_string())),
        Task::new(
            "Install Git",
            "Installs git via your package manager.",
            0,
            script(&pkg(pm, "git")),
        )
        .with_check(Check::Bin("git".to_string())),
        Task::new(
            "Install GitHub CLI",
            "Installs the gh CLI via your package manager.",
            0,
            script(&pkg(pm, "gh")),
        )
        .with_check(Check::Bin("gh".to_string())),
        Task::new(
            "Install htop",
            "Installs the htop process viewer.",
            0,
            script(&pkg(pm, "htop")),
        )
        .with_check(Check::Bin("htop".to_string())),
        Task::new(
            "Install tmux",
            "Installs the tmux terminal multiplexer.",
            0,
            script(&pkg(pm, "tmux")),
        )
        .with_check(Check::Bin("tmux".to_string())),
        Task::new(
            "Install ripgrep",
            "Installs rg, a fast code search tool.",
            0,
            script(&pkg(pm, "ripgrep")),
        )
        .with_check(Check::Bin("rg".to_string())),
        Task::new(
            "Install bat",
            "Installs bat, a cat clone with syntax highlighting.",
            0,
            script(&pkg(pm, "bat")),
        )
        .with_any_check(&["bat", "batcat"]),
        Task::new(
            "Install jq",
            "Installs jq, a command-line JSON processor.",
            0,
            script(&pkg(pm, "jq")),
        )
        .with_check(Check::Bin("jq".to_string())),
        Task::new(
            "Install Python 3",
            "Installs Python 3 and pip via your package manager.",
            0,
            script(&pkg_alias(
                pm,
                "python3 python3-pip",
                "python3 python3-pip",
                "python python-pip",
                "python3 python3-pip",
            )),
        )
        .with_check(Check::Bin("python3".to_string())),
        Task::new(
            "Install Go",
            "Installs the Go toolchain via your package manager.",
            0,
            script(&pkg_alias(pm, "golang-go", "golang", "go", "go")),
        )
        .with_check(Check::Bin("go".to_string())),
        Task::new(
            "Install Neovim",
            "Installs the Neovim editor (the bashrc uses it as $EDITOR when present).",
            0,
            script(&pkg(pm, "neovim")),
        )
        .with_check(Check::Bin("nvim".to_string())),
        Task::new(
            "Install Rust",
            "Installs Rust with rustup (user-local, no package manager needed).",
            0,
            script("curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs | sh -s -- -y"),
        )
        .with_check(Check::Bin("rustc".to_string())),
        Task::new(
            "Install Docker Compose",
            "Installs the Docker Compose plugin. Install Docker first.",
            0,
            script(&pkg_alias(
                pm,
                "docker-compose-plugin",
                "docker-compose-plugin",
                "docker-compose",
                "docker-compose",
            )),
        )
        .with_check(Check::Bin("docker-compose".to_string())),
        Task::new(
            "Install kubectl",
            "Installs the Kubernetes CLI via your package manager.",
            0,
            script(&pkg_alias(
                pm,
                "kubectl",
                "kubernetes-client",
                "kubectl",
                "kubectl",
            )),
        )
        .with_check(Check::Bin("kubectl".to_string())),
        Task::new(
            "Install Helm",
            "Installs Helm with the official installer script.",
            0,
            script("curl -fsSL https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3 | bash"),
        )
        .with_check(Check::Bin("helm".to_string())),
        Task::new(
            "Install Starship",
            "Installs the Starship prompt with the official installer. Already-installed shells are left untouched.",
            0,
            script("command -v starship >/dev/null 2>&1 || curl -sS https://starship.rs/install.sh | $SUDO sh -s -- -y"),
        )
        .with_check(Check::Bin("starship".to_string())),
        Task::new(
            "Install Fastfetch",
            "Installs Fastfetch for the startup banner, adding the upstream PPA when apt lacks the package.",
            0,
            script(&if pm == "apt" {
                "$SUDO apt-get install -y fastfetch || { $SUDO apt-get install -y software-properties-common && $SUDO add-apt-repository -y ppa:zhangsongcui3371/fastfetch && $SUDO apt-get update && $SUDO apt-get install -y fastfetch; }".to_string()
            } else {
                pkg_alias(pm, "fastfetch", "fastfetch", "fastfetch", "fastfetch")
            }),
        )
        .with_check(Check::Bin("fastfetch".to_string())),
        Task::new(
            "Install eza",
            "Installs eza, a modern replacement for ls with git and tree support.",
            0,
            script(&pkg(pm, "eza")),
        )
        .with_check(Check::Bin("eza".to_string())),
        Task::new(
            "Install btop",
            "Installs btop, a resource monitor for CPU, memory, disks, and network.",
            0,
            script(&pkg(pm, "btop")),
        )
        .with_check(Check::Bin("btop".to_string())),
        Task::new(
            "Install fd",
            "Installs fd, a fast and user-friendly alternative to find.",
            0,
            script(&pkg_alias(pm, "fd-find", "fd-find", "fd", "fd")),
        )
        .with_any_check(&["fd", "fdfind"]),
        Task::new(
            "Install tree",
            "Installs tree for directory listings.",
            0,
            script(&pkg(pm, "tree")),
        )
        .with_check(Check::Bin("tree".to_string())),
        Task::new(
            "Install wget",
            "Installs wget for non-interactive downloads.",
            0,
            script(&pkg(pm, "wget")),
        )
        .with_check(Check::Bin("wget".to_string())),
        Task::new(
            "Install shellcheck",
            "Installs ShellCheck for linting shell scripts.",
            0,
            script(&pkg(pm, "shellcheck")),
        )
        .with_check(Check::Bin("shellcheck".to_string())),
        Task::new(
            "Install Zoxide",
            "Installs zoxide, a smarter cd command used by the shell profile.",
            0,
            script(&pkg(pm, "zoxide")),
        )
        .with_check(Check::Bin("zoxide".to_string())),
        Task::new(
            "Install fzf",
            "Installs fzf fuzzy finder and its shell integration.",
            0,
            script(&pkg(pm, "fzf")),
        )
        .with_check(Check::Bin("fzf".to_string())),
        Task::new(
            "Install trash-cli",
            "Installs trash-cli so the shell profile can alias rm to a recoverable trash.",
            0,
            script(&pkg(pm, "trash-cli")),
        )
        .with_any_check(&["trash-put"]),
        Task::new(
            "Install HTTPie",
            "Installs HTTPie (http/https) for readable HTTP requests.",
            0,
            script(&pkg(pm, "httpie")),
        )
        .with_any_check(&["http", "httpie"]),
        Task::new(
            "Refresh dotfiles",
            "Re-links ~/.bashrc, starship.toml and the fastfetch config from ~/.local/share/bashbox.",
            1,
            Action::Builtin(Builtin::Refresh),
        ),
        Task::new(
            "Uninstall bashbox",
            "Restores your .bashbox-backup files and removes bashbox config copies and font installs.",
            1,
            Action::Builtin(Builtin::Uninstall),
        )
        .destructive(),
        Task::new(
            "Toggle fastfetch banner",
            "Adds or removes BASHBOX_NO_FETCH=1 in ~/.bashrc to turn the startup banner off or on.",
            1,
            Action::Builtin(Builtin::ToggleBanner),
        ),
        Task::new(
            "Test shell startup",
            "Opens a throwaway interactive Bash to verify the prompt and banner load correctly.",
            1,
            script("bash -ic 'echo \"Interactive shell loaded.\"'"),
        ),
        Task::new(
            "Edit .bashrc",
            "Opens ~/.bashrc in $EDITOR (or nano/vi).",
            1,
            Action::Edit(home.join(".bashrc")),
        ),
        Task::new(
            "Edit starship config",
            "Opens ~/.config/starship.toml in $EDITOR (or nano/vi).",
            1,
            Action::Edit(home.join(".config/starship.toml")),
        ),
        Task::new(
            "Edit fastfetch config",
            "Opens ~/.config/fastfetch/config.jsonc in $EDITOR (or nano/vi).",
            1,
            Action::Edit(home.join(".config/fastfetch/config.jsonc")),
        ),
        Task::new(
            "Edit custom tasks",
            "Opens ~/.config/bashbox/tasks.toml (created if missing) in $EDITOR.",
            1,
            Action::Edit(home.join(".config/bashbox/tasks.toml")),
        ),
        Task::new(
            "Reset toolbox state",
            "Clears pinned tasks, recent runs, and the saved pane width.",
            1,
            Action::Builtin(Builtin::ResetState),
        )
        .destructive(),
        Task::new(
            "Update system",
            "Upgrades all packages non-interactively via the detected package manager.",
            3,
            script(&update_cmd(pm)),
        )
        .destructive(),
        Task::new(
            "Clean package cache",
            "Removes orphaned packages and the download cache.",
            3,
            script(&cleanup_cmd(pm)),
        )
        .destructive(),
        Task::new(
            "Run doctor",
            "Checks all bashbox dependencies, config links, and fonts, and reports what is missing.",
            3,
            Action::Builtin(Builtin::Doctor),
        ),
        Task::new(
            "Install Nerd Fonts",
            "Downloads JetBrainsMono, FiraCode, CascadiaCode and Hack Nerd Fonts to ~/.local/share/fonts.",
            3,
            script(&format!(
                "for font in {}; do d=\"$HOME/.local/share/fonts/${{font}}NF\"; if [ -d \"$d\" ]; then continue; fi; echo \"Installing $font Nerd Font...\"; curl -fsSL -o \"/tmp/${{font}}.zip\" \"https://github.com/ryanoasis/nerd-fonts/releases/latest/download/${{font}}.zip\" && unzip -oq \"/tmp/${{font}}.zip\" -d \"$d\"; rm -f \"/tmp/${{font}}.zip\"; done; if command -v fc-cache >/dev/null; then fc-cache -f; fi",
                FONTS.join(" ")
            )),
        )
        .with_check(Check::Fonts),
        Task::new(
            "Docker cleanup",
            "Removes stopped containers, unused images, networks, and build cache. Volumes are kept.",
            3,
            script("if command -v docker >/dev/null 2>&1; then $SUDO docker system prune -af; else echo 'Docker is not installed.'; fi"),
        )
        .destructive()
        .with_check(Check::Bin("docker".to_string())),
        Task::new(
            "Vacuum journal logs",
            "Trims systemd journal logs to the most recent 200 MB.",
            3,
            script("if command -v journalctl >/dev/null 2>&1; then $SUDO journalctl --vacuum-size=200M; else echo 'systemd journal is not available here.'; fi"),
        )
        .destructive(),
        Task::new(
            "Refresh font cache",
            "Rebuilds the fontconfig cache after installing Nerd Fonts.",
            3,
            script("if command -v fc-cache >/dev/null 2>&1; then fc-cache -f; else echo 'fc-cache is not available.'; fi"),
        ),
        Task::new(
            "Disk usage",
            "Shows free space on mounted filesystems.",
            4,
            script("df -h"),
        ),
        Task::new(
            "Memory usage",
            "Shows RAM and swap usage.",
            4,
            script("free -h 2>/dev/null || vm_stat 2>/dev/null || echo 'No memory tool found'"),
        ),
        Task::new(
            "Network interfaces",
            "Shows interface addresses and link state.",
            4,
            script("ip -brief addr 2>/dev/null || ifconfig 2>/dev/null || echo 'No network tool found'"),
        ),
        Task::new(
            "Listening ports",
            "Shows listening TCP/UDP sockets and their processes.",
            4,
            script("ss -tulpn 2>/dev/null || netstat -tulpn 2>/dev/null || echo 'No socket tool found'"),
        ),
        Task::new(
            "Failed services",
            "Lists failed systemd units.",
            4,
            script("systemctl --failed --no-pager 2>/dev/null || echo 'systemd is not available here'"),
        ),
        Task::new(
            "Environment",
            "Shows distro, kernel, virtualization, WSL, and container detection.",
            4,
            script(
                r#"echo "Host: $(uname -n)"; echo "Kernel: $(uname -sr)"; if [ -r /etc/os-release ]; then . /etc/os-release; echo "Distro: ${PRETTY_NAME:-unknown}"; fi; if grep -qi microsoft /proc/version 2>/dev/null; then echo "WSL: yes"; else echo "WSL: no"; fi; if [ -f /.dockerenv ] || [ -f /run/.containerenv ]; then echo "Container: yes"; else echo "Container: no"; fi; if command -v systemd-detect-virt >/dev/null 2>&1; then echo "Virtualization: $(systemd-detect-virt 2>/dev/null)"; fi"#,
            ),
        ),
        Task::new(
            "Check for updates",
            "Compares the installed version with the latest GitHub release.",
            4,
            script(&format!(
                "latest=$(curl -fsSL --max-time 10 https://api.github.com/repos/hollali/bashbox/releases/latest 2>/dev/null | sed -n 's/.*\"tag_name\"[[:space:]]*:[[:space:]]*\"\\([^\"]*\\)\".*/\\1/p' | head -n1); echo \"Installed: v{}\"; if [ -z \"$latest\" ]; then echo 'Latest: could not check (offline or no releases)'; elif [ \"v{}\" = \"$latest\" ]; then echo \"Latest: $latest (up to date)\"; else echo \"Latest: $latest (update available)\"; fi",
                env!("CARGO_PKG_VERSION"),
                env!("CARGO_PKG_VERSION")
            )),
        ),
        Task::new(
            "View last failure log",
            "Shows the output saved by the most recent failed task.",
            4,
            Action::Builtin(Builtin::LastLog),
        ),
        Task::new(
            "Recent runs",
            "Lists the tasks you ran most recently.",
            4,
            Action::Builtin(Builtin::Recent),
        ),
        Task::new(
            "Show backups",
            "Finds .bashbox-backup files left by setup.sh.",
            4,
            script("find \"$HOME\" -maxdepth 3 -name '*.bashbox-backup' -print 2>/dev/null | sort || true"),
        ),
        Task::new(
            "Diff .bashrc vs backup",
            "Shows what changed in .bashrc compared with the pre-bashbox backup.",
            4,
            script("if [ -e \"$HOME/.bashrc.bashbox-backup\" ]; then diff -u \"$HOME/.bashrc.bashbox-backup\" \"$HOME/.bashrc\" || true; else echo 'No .bashrc backup found.'; fi"),
        ),
        Task::new(
            "Nerd Fonts detected",
            "Lists Nerd Font families installed for the current user.",
            4,
            script("fc-list 2>/dev/null | grep -i 'nerd font' | sort -u | head -40 || echo 'fc-list is not available'"),
        ),
        Task::new(
            "Top processes",
            "Shows the ten processes using the most CPU.",
            4,
            script("ps -eo pid,ppid,comm,%cpu,%mem --sort=-%cpu 2>/dev/null | head -11 || ps aux | head -11"),
        ),
        Task::new(
            "Uptime and load",
            "Shows system uptime and load averages.",
            4,
            script("uptime 2>/dev/null || echo 'uptime is not available'"),
        ),
        Task::new(
            "CPU information",
            "Shows CPU model, cores, and cache details.",
            4,
            script("lscpu 2>/dev/null || sysctl -n machdep.cpu.brand_string 2>/dev/null || echo 'No CPU tool found'"),
        ),
        Task::new(
            "Temperatures",
            "Reads hardware temperatures when sensors or thermal zones are available.",
            4,
            script("if command -v sensors >/dev/null 2>&1; then sensors; elif [ -d /sys/class/thermal ]; then for z in /sys/class/thermal/thermal_zone*; do [ -r \"$z/temp\" ] || continue; printf '%s: %s C\\n' \"$(cat \"$z/type\" 2>/dev/null)\" \"$(( $(cat \"$z/temp\") / 1000 ))\"; done; else echo 'No temperature sensors found.'; fi"),
        ),
        Task::new(
            "Battery status",
            "Shows battery charge, state, and time remaining when a battery is present.",
            4,
            script("if command -v upower >/dev/null 2>&1; then upower -i $(upower -e | grep -m1 BAT) 2>/dev/null | grep -E 'state|percentage|time to'; elif command -v acpi >/dev/null 2>&1; then acpi -b; elif [ -d /sys/class/power_supply ]; then for b in /sys/class/power_supply/BAT*; do [ -r \"$b/capacity\" ] || continue; printf 'Battery: %s%% (%s)\\n' \"$(cat \"$b/capacity\")\" \"$(cat \"$b/status\")\"; done; else echo 'No battery detected.'; fi"),
        ),
        Task::new(
            "Logged-in users",
            "Lists users with active login sessions.",
            4,
            script("who 2>/dev/null || echo 'who is not available'"),
        ),
        Task::new(
            "Public IP",
            "Looks up your public IPv4 address.",
            4,
            script("curl -fsSL --max-time 10 https://ifconfig.me 2>/dev/null || echo 'Could not reach ifconfig.me'"),
        ),
        Task::new(
            "Broken symlinks",
            "Finds dangling symlinks under your home directory.",
            4,
            script("find \"$HOME\" -maxdepth 4 -xtype l -print 2>/dev/null | head -40 || true"),
        ),
        Task::new(
            "ShellCheck .bashrc",
            "Lints ~/.bashrc with ShellCheck when it is installed.",
            4,
            script("if command -v shellcheck >/dev/null 2>&1; then shellcheck \"$HOME/.bashrc\"; else echo 'ShellCheck is not installed (Applications tab).'; fi"),
        ),
        Task::new(
            "Git configuration",
            "Shows the effective git config with the file each value came from.",
            4,
            script("git config --list --show-origin 2>/dev/null || echo 'git is not available'"),
        ),
        Task::new(
            "Preview fastfetch banner",
            "Renders the installed fastfetch config without changing anything.",
            4,
            script("if command -v fastfetch >/dev/null 2>&1; then fastfetch --config \"$HOME/.config/fastfetch/config.jsonc\" --logo none; else echo 'Fastfetch is not installed.'; fi"),
        ),
        Task::new(
            "Clear failure log",
            "Deletes the saved output from the most recent failed task.",
            4,
            Action::Builtin(Builtin::ClearLog),
        )
        .destructive(),
        Task::new(
            "Search shell history",
            "Greps ~/.bash_history for a pattern and shows recent matches.",
            4,
            script(r#"if [ -f "$HOME/.bash_history" ]; then grep -i -- {} "$HOME/.bash_history" | tail -40; else echo 'No ~/.bash_history found.'; fi"#),
        )
        .with_prompt("Pattern to search"),
        Task::new(
            "Most used commands",
            "Shows the commands you run most often from ~/.bash_history.",
            4,
            script(r#"if [ -f "$HOME/.bash_history" ]; then awk '{print $1}' "$HOME/.bash_history" | sort | uniq -c | sort -rn | head -20; else echo 'No ~/.bash_history found.'; fi"#),
        ),
        Task::new(
            "Service status",
            "Shows the status and recent log lines for a systemd service.",
            4,
            script(r#"if command -v systemctl >/dev/null 2>&1; then systemctl status --no-pager {} 2>&1 | head -40; else echo 'systemd is not available here.'; fi"#),
        )
        .with_prompt("Service name"),
        Task::new(
            "Journal for service",
            "Shows the last 50 journal lines for a systemd unit.",
            4,
            script(r#"if command -v journalctl >/dev/null 2>&1; then $SUDO journalctl -u {} -n 50 --no-pager 2>&1; else echo 'systemd journal is not available here.'; fi"#),
        )
        .with_prompt("Service name"),
        Task::new(
            "HTTP headers",
            "Fetches response headers for a URL with curl.",
            4,
            script(r#"if command -v curl >/dev/null 2>&1; then curl -fsSI --max-time 15 {} 2>&1; else echo 'curl is not installed.'; fi"#),
        )
        .with_prompt("URL")
        .with_timeout(20),
        Task::new(
            "Install lazygit",
            "Installs lazygit, a terminal UI for git.",
            5,
            script(&pkg(pm, "lazygit")),
        )
        .with_check(Check::Bin("lazygit".to_string())),
        Task::new(
            "Install direnv",
            "Installs direnv to load per-project environment variables automatically.",
            5,
            script(&pkg(pm, "direnv")),
        )
        .with_check(Check::Bin("direnv".to_string())),
        Task::new(
            "Install just",
            "Installs just, a command runner for project recipes.",
            5,
            script(&pkg(pm, "just")),
        )
        .with_check(Check::Bin("just".to_string())),
        Task::new(
            "Install delta",
            "Installs git-delta for readable side-by-side git diffs.",
            5,
            script(&pkg_alias(pm, "git-delta", "git-delta", "git-delta", "git-delta")),
        )
        .with_any_check(&["delta", "git-delta"]),
        Task::new(
            "Install hyperfine",
            "Installs hyperfine for benchmarking command-line tools.",
            5,
            script(&pkg(pm, "hyperfine")),
        )
        .with_check(Check::Bin("hyperfine".to_string())),
        Task::new(
            "Install mkcert",
            "Installs mkcert for locally trusted development certificates.",
            5,
            script(&pkg(pm, "mkcert")),
        )
        .with_check(Check::Bin("mkcert".to_string())),
        Task::new(
            "Install uv",
            "Installs uv, a fast Python package and project manager.",
            5,
            script("command -v uv >/dev/null 2>&1 || curl -LsSf https://astral.sh/uv/install.sh | sh"),
        )
        .with_check(Check::Bin("uv".to_string())),
        Task::new(
            "Install pre-commit",
            "Installs pre-commit for managing git hooks.",
            5,
            script(&pkg(pm, "pre-commit")),
        )
        .with_check(Check::Bin("pre-commit".to_string())),
        Task::new(
            "Install actionlint",
            "Installs actionlint for validating GitHub Actions workflows.",
            5,
            script(&pkg(pm, "actionlint")),
        )
        .with_check(Check::Bin("actionlint".to_string())),
        Task::new(
            "Install glab",
            "Installs the GitLab CLI.",
            5,
            script(&pkg(pm, "glab")),
        )
        .with_check(Check::Bin("glab".to_string())),
        Task::new(
            "Run project tests",
            "Detects the project in the current directory and runs its tests (Cargo, Go, npm, pytest, or make).",
            5,
            script(r#"echo "Directory: $PWD"; if [ -f Cargo.toml ]; then cargo test; elif [ -f go.mod ]; then go test ./...; elif [ -f package.json ]; then npm test; elif [ -f pyproject.toml ] || [ -f pytest.ini ] || [ -d tests ]; then pytest; elif [ -f Makefile ]; then make test; else echo 'No supported project manifest found.'; fi"#),
        )
        .with_timeout(300),
        Task::new(
            "Lint project",
            "Detects the project in the current directory and runs its linter (clippy, go vet, npm lint, ruff, or make lint).",
            5,
            script(r#"echo "Directory: $PWD"; if [ -f Cargo.toml ]; then cargo fmt --check && cargo clippy --all-targets -- -D warnings; elif [ -f go.mod ]; then go vet ./...; elif [ -f package.json ]; then npm run lint; elif [ -f pyproject.toml ]; then if command -v ruff >/dev/null 2>&1; then ruff check .; elif command -v flake8 >/dev/null 2>&1; then flake8; else echo 'Install ruff or flake8 to lint this project.'; fi; elif [ -f Makefile ]; then make lint; else echo 'No supported project manifest found.'; fi"#),
        )
        .with_timeout(300),
        Task::new(
            "Audit dependencies",
            "Runs the dependency audit for the detected project (npm, cargo-audit, pip-audit, or govulncheck).",
            5,
            script(r#"echo "Directory: $PWD"; if [ -f package.json ]; then npm audit; elif [ -f Cargo.toml ]; then if command -v cargo-audit >/dev/null 2>&1; then cargo audit; else echo 'Install cargo-audit: cargo install cargo-audit'; fi; elif [ -f requirements.txt ] || [ -f pyproject.toml ]; then if command -v pip-audit >/dev/null 2>&1; then pip-audit; else echo 'Install pip-audit: python3 -m pip install pip-audit'; fi; elif [ -f go.mod ]; then if command -v govulncheck >/dev/null 2>&1; then govulncheck ./...; else echo 'Install govulncheck: go install golang.org/x/vuln/cmd/govulncheck@latest'; fi; else echo 'No supported project manifest found.'; fi"#),
        )
        .with_timeout(300),
        Task::new(
            "Git repo summary",
            "Shows the current branch, last commit, remotes, and working-tree status.",
            5,
            script(r#"if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then echo "Repo: $(git rev-parse --show-toplevel)"; echo "Branch: $(git branch --show-current 2>/dev/null)"; echo "Commit: $(git log -1 --pretty='%h %s (%cr)' 2>/dev/null)"; echo "Remote: $(git remote get-url origin 2>/dev/null || echo none)"; echo; git status --short --branch; else echo 'Not inside a git repository.'; fi"#),
        ),
        Task::new(
            "Find TODOs and FIXMEs",
            "Searches the current directory for TODO, FIXME, HACK, and XXX markers.",
            5,
            script(r#"if command -v rg >/dev/null 2>&1; then rg -n --hidden --glob '!.git' 'TODO|FIXME|HACK|XXX' . | head -60; else grep -RInE 'TODO|FIXME|HACK|XXX' --exclude-dir=.git . | head -60; fi"#),
        ),
        Task::new(
            "Large files in repo",
            "Lists the largest tracked files in the current git repository.",
            5,
            script(r#"if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then git ls-files -z | xargs -0 du -h 2>/dev/null | sort -rh | head -20; else echo 'Not inside a git repository.'; fi"#),
        ),
        Task::new(
            "Language and toolchain versions",
            "Summarises the versions of common languages and build tools that are installed.",
            5,
            script(r#"for c in git node npm python3 pip3 go rustc cargo java ruby php dotnet; do if command -v "$c" >/dev/null 2>&1; then printf '%-8s ' "$c"; "$c" --version 2>&1 | head -n1; fi; done"#),
        ),
        Task::new(
            "Recent git branches",
            "Lists local branches ordered by most recent commit.",
            5,
            script(r#"if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then git branch --sort=-committerdate --format='%(committerdate:short) %(refname:short)' | head -20; else echo 'Not inside a git repository.'; fi"#),
        ),
        Task::new(
            "Install AWS CLI",
            "Installs the AWS CLI v2 with the official installer.",
            6,
            script(r#"if command -v aws >/dev/null 2>&1; then echo 'AWS CLI already installed.'; else tmp=$(mktemp -d); case "$(uname -m)" in aarch64|arm64) a=aarch64;; *) a=x86_64;; esac; curl -fsSL "https://awscli.amazonaws.com/awscli-exe-linux-${a}.zip" -o "$tmp/awscliv2.zip" && unzip -q "$tmp/awscliv2.zip" -d "$tmp" && $SUDO "$tmp/aws/install" --update; rm -rf "$tmp"; fi"#),
        )
        .with_check(Check::Bin("aws".to_string())),
        Task::new(
            "Install Azure CLI",
            "Installs the Azure CLI with the official installer on apt and the package manager elsewhere.",
            6,
            script(&if pm == "apt" {
                "command -v az >/dev/null 2>&1 || curl -sL https://aka.ms/InstallAzureCLIDeb | $SUDO bash".to_string()
            } else {
                format!("command -v az >/dev/null 2>&1 || {}", pkg(pm, "azure-cli"))
            }),
        )
        .with_check(Check::Bin("az".to_string())),
        Task::new(
            "Install Google Cloud CLI",
            "Downloads the Google Cloud CLI to ~/.local/share and links gcloud, gsutil, and bq into ~/.local/bin.",
            6,
            script(r#"if command -v gcloud >/dev/null 2>&1; then echo 'Google Cloud CLI already installed.'; else tmp=$(mktemp -d); case "$(uname -m)" in aarch64|arm64) a=arm;; *) a=x86_64;; esac; curl -fsSL "https://dl.google.com/dl/cloudsdk/channels/rapid/downloads/google-cloud-cli-linux-${a}.tar.gz" -o "$tmp/gcloud.tar.gz" && tar -xzf "$tmp/gcloud.tar.gz" -C "$HOME/.local/share" && "$HOME/.local/share/google-cloud-sdk/install.sh" --quiet --path-update false --command-completion false && mkdir -p "$HOME/.local/bin" && for b in gcloud gsutil bq; do ln -sf "$HOME/.local/share/google-cloud-sdk/bin/$b" "$HOME/.local/bin/$b"; done; rm -rf "$tmp"; fi"#),
        )
        .with_check(Check::Bin("gcloud".to_string())),
        Task::new(
            "Install Terraform",
            "Installs the latest Terraform release for this architecture into /usr/local/bin.",
            6,
            script(r#"if command -v terraform >/dev/null 2>&1; then echo 'Terraform already installed.'; else tmp=$(mktemp -d); case "$(uname -m)" in aarch64|arm64) a=arm64;; *) a=amd64;; esac; version=$(curl -fsSL https://checkpoint-api.hashicorp.com/v1/check/terraform | sed -n 's/.*"current_version":"\([^"]*\)".*/\1/p'); [ -n "$version" ] || version=1.9.8; curl -fsSL "https://releases.hashicorp.com/terraform/${version}/terraform_${version}_linux_${a}.zip" -o "$tmp/terraform.zip" && unzip -q "$tmp/terraform.zip" -d "$tmp" && $SUDO install -m 0755 "$tmp/terraform" /usr/local/bin/terraform; rm -rf "$tmp"; fi"#),
        )
        .with_check(Check::Bin("terraform".to_string())),
        Task::new(
            "Install k9s",
            "Installs k9s, a terminal UI for Kubernetes.",
            6,
            script(&pkg(pm, "k9s")),
        )
        .with_check(Check::Bin("k9s".to_string())),
        Task::new(
            "Install kubectx",
            "Installs kubectx and kubens for switching Kubernetes contexts and namespaces.",
            6,
            script(&pkg(pm, "kubectx")),
        )
        .with_any_check(&["kubectx", "kubens"]),
        Task::new(
            "AWS identity",
            "Shows the AWS caller identity for the active credentials.",
            6,
            script(r#"if command -v aws >/dev/null 2>&1; then aws sts get-caller-identity 2>&1; else echo 'AWS CLI is not installed (Cloud tab).'; fi"#),
        ),
        Task::new(
            "AWS profiles and region",
            "Lists configured AWS profiles and the active region.",
            6,
            script(r#"if command -v aws >/dev/null 2>&1; then echo 'Profiles:'; aws configure list-profiles 2>/dev/null; echo; aws configure list 2>&1; else echo 'AWS CLI is not installed (Cloud tab).'; fi"#),
        ),
        Task::new(
            "Azure account",
            "Shows the active Azure account and subscription.",
            6,
            script(r#"if command -v az >/dev/null 2>&1; then az account show --output table 2>&1 || az account list --output table 2>&1; else echo 'Azure CLI is not installed (Cloud tab).'; fi"#),
        ),
        Task::new(
            "Google Cloud auth",
            "Lists authenticated Google Cloud accounts and the active configuration.",
            6,
            script(r#"if command -v gcloud >/dev/null 2>&1; then gcloud auth list --format='table(account,status)' 2>&1; echo; gcloud config list 2>&1; else echo 'Google Cloud CLI is not installed (Cloud tab).'; fi"#),
        ),
        Task::new(
            "Kubernetes contexts",
            "Lists kubeconfig contexts and marks the current one.",
            6,
            script(r#"if command -v kubectl >/dev/null 2>&1; then kubectl config get-contexts 2>&1; else echo 'kubectl is not installed (Applications tab).'; fi"#),
        ),
        Task::new(
            "Kubernetes cluster info",
            "Shows the API server and service addresses of the current cluster.",
            6,
            script(r#"if command -v kubectl >/dev/null 2>&1; then kubectl cluster-info 2>&1; else echo 'kubectl is not installed (Applications tab).'; fi"#),
        )
        .with_timeout(30),
        Task::new(
            "Cloud CLI versions",
            "Prints the versions of installed cloud and infrastructure CLIs.",
            6,
            script(r#"for c in aws az gcloud terraform kubectl helm k9s podman; do if command -v "$c" >/dev/null 2>&1; then printf '%-10s ' "$c"; "$c" --version 2>&1 | head -n1; fi; done"#),
        ),
        Task::new(
            "Terraform workspace and state",
            "Shows the Terraform version, workspace, and resources in the current project.",
            6,
            script(r#"if command -v terraform >/dev/null 2>&1; then terraform version; if [ -d .terraform ] || [ -f .terraform.lock.hcl ]; then echo; echo "Workspace: $(terraform workspace show 2>/dev/null)"; terraform state list 2>&1 | head -50; else echo 'Run this inside an initialised Terraform project to see workspace and state.'; fi; else echo 'Terraform is not installed (Cloud tab).'; fi"#),
        )
        .with_timeout(30),
        Task::new(
            "Install nmap",
            "Installs nmap for network discovery and port scanning.",
            7,
            script(&pkg(pm, "nmap")),
        )
        .with_check(Check::Bin("nmap".to_string())),
        Task::new(
            "Install tcpdump",
            "Installs tcpdump for packet capture analysis.",
            7,
            script(&pkg(pm, "tcpdump")),
        )
        .with_check(Check::Bin("tcpdump".to_string())),
        Task::new(
            "Install whois",
            "Installs the whois client for domain and IP lookups.",
            7,
            script(&pkg(pm, "whois")),
        )
        .with_check(Check::Bin("whois".to_string())),
        Task::new(
            "Install DNS tools",
            "Installs dig/host/nslookup for DNS troubleshooting.",
            7,
            script(&pkg_alias(pm, "dnsutils", "bind-utils", "bind", "bind-utils")),
        )
        .with_any_check(&["dig", "host", "drill", "nslookup"]),
        Task::new(
            "Install netcat",
            "Installs a netcat implementation for quick network tests.",
            7,
            script(&pkg_alias(
                pm,
                "netcat-openbsd",
                "nmap-ncat",
                "openbsd-netcat",
                "netcat-openbsd",
            )),
        )
        .with_any_check(&["nc", "ncat", "netcat"]),
        Task::new(
            "Install GnuPG",
            "Installs GnuPG for signing and encrypting files.",
            7,
            script(&pkg_alias(pm, "gnupg", "gnupg2", "gnupg", "gpg2")),
        )
        .with_check(Check::Bin("gpg".to_string())),
        Task::new(
            "Install age",
            "Installs age, a simple modern file-encryption tool.",
            7,
            script(&pkg(pm, "age")),
        )
        .with_check(Check::Bin("age".to_string())),
        Task::new(
            "Install pass",
            "Installs pass, the standard Unix password manager.",
            7,
            script(&pkg_alias(pm, "pass", "pass", "pass", "password-store")),
        )
        .with_check(Check::Bin("pass".to_string())),
        Task::new(
            "Install Lynis",
            "Installs Lynis for local security auditing and hardening checks.",
            7,
            script(&pkg(pm, "lynis")),
        )
        .with_check(Check::Bin("lynis".to_string())),
        Task::new(
            "Install Trivy",
            "Installs Trivy for container, filesystem, and dependency vulnerability scanning.",
            7,
            script("command -v trivy >/dev/null 2>&1 || curl -sfL https://raw.githubusercontent.com/aquasecurity/trivy/main/contrib/install.sh | $SUDO sh -s -- -b /usr/local/bin"),
        )
        .with_check(Check::Bin("trivy".to_string())),
        Task::new(
            "Install gitleaks",
            "Installs gitleaks from the latest GitHub release for secret scanning.",
            7,
            script(r#"if command -v gitleaks >/dev/null 2>&1; then echo 'gitleaks already installed.'; else tmp=$(mktemp -d); case "$(uname -m)" in x86_64|amd64) a=x64;; aarch64|arm64) a=arm64;; *) echo "Unsupported architecture: $(uname -m)"; exit 1;; esac; url=$(curl -fsSL https://api.github.com/repos/gitleaks/gitleaks/releases/latest | grep -o "https://[^\"]*linux_${a}\.tar\.gz" | head -n1); if [ -z "$url" ]; then echo 'Could not find a gitleaks release asset.'; else curl -fsSL "$url" -o "$tmp/gitleaks.tar.gz" && tar -xzf "$tmp/gitleaks.tar.gz" -C "$tmp" && $SUDO install -m 0755 "$tmp/gitleaks" /usr/local/bin/gitleaks; fi; rm -rf "$tmp"; fi"#),
        )
        .with_check(Check::Bin("gitleaks".to_string())),
        Task::new(
            "TLS certificate check",
            "Connects to a host on port 443 and shows the subject, issuer, validity dates, and SHA-256 fingerprint.",
            7,
            script(r#"if command -v openssl >/dev/null 2>&1; then printf '' | openssl s_client -connect {}:443 -servername {} 2>/dev/null | openssl x509 -noout -subject -issuer -dates -fingerprint -sha256 || echo 'Could not retrieve a certificate.'; else echo 'openssl is not installed.'; fi"#),
        )
        .with_prompt("Hostname (port 443)")
        .with_timeout(20),
        Task::new(
            "Generate strong password",
            "Generates a 24-byte random password using OpenSSL or /dev/urandom.",
            7,
            script(r#"if command -v openssl >/dev/null 2>&1; then openssl rand -base64 24; else head -c 24 /dev/urandom | base64; fi"#),
        ),
        Task::new(
            "Generate UUID",
            "Generates a random UUID using uuidgen or the kernel random source.",
            7,
            script("uuidgen 2>/dev/null || cat /proc/sys/kernel/random/uuid"),
        ),
        Task::new(
            "Hash a file",
            "Prints the SHA-256 checksum of a file.",
            7,
            script(r#"if [ -f {} ]; then sha256sum {}; else echo 'File not found.'; fi"#),
        )
        .with_prompt("File to hash"),
        Task::new(
            "SSH host key scan",
            "Fetches the public host keys for a host with ssh-keyscan.",
            7,
            script(r#"if command -v ssh-keyscan >/dev/null 2>&1; then ssh-keyscan {} 2>/dev/null || echo 'Could not scan the host.'; else echo 'ssh-keyscan is not installed.'; fi"#),
        )
        .with_prompt("Host to scan")
        .with_timeout(20),
        Task::new(
            "Firewall status",
            "Shows the active firewall rules using ufw, firewalld, nftables, or iptables.",
            7,
            script(r#"if command -v ufw >/dev/null 2>&1; then $SUDO ufw status verbose; elif command -v firewall-cmd >/dev/null 2>&1; then $SUDO firewall-cmd --state; $SUDO firewall-cmd --list-all; elif command -v nft >/dev/null 2>&1; then $SUDO nft list ruleset; elif command -v iptables >/dev/null 2>&1; then $SUDO iptables -S; else echo 'No supported firewall tool found.'; fi"#),
        ),
        Task::new(
            "SSH daemon audit",
            "Reviews key sshd_config options and the permissions of your SSH directory.",
            7,
            script(r#"if [ -r /etc/ssh/sshd_config ]; then echo 'sshd_config:'; grep -Ei '^[[:space:]]*(PermitRootLogin|PasswordAuthentication|PermitEmptyPasswords|PubkeyAuthentication|X11Forwarding|AllowTcpForwarding)' /etc/ssh/sshd_config 2>/dev/null || true; else echo 'Cannot read /etc/ssh/sshd_config.'; fi; echo; for p in "$HOME/.ssh" "$HOME/.ssh/authorized_keys"; do if [ -e "$p" ]; then stat -c '%a %n' "$p"; fi; done"#),
        ),
        Task::new(
            "Last logins",
            "Shows recent login sessions and their source addresses.",
            7,
            script("last -n 20 2>/dev/null || echo 'last is not available'"),
        ),
        Task::new(
            "SSH key inventory",
            "Lists your SSH public keys with fingerprints and checks ~/.ssh permissions.",
            7,
            script(r#"if [ -d "$HOME/.ssh" ]; then stat -c '%a %n' "$HOME/.ssh" 2>/dev/null; for k in "$HOME"/.ssh/*.pub; do [ -f "$k" ] || continue; echo; echo "$k"; ssh-keygen -lf "$k" 2>/dev/null; done; else echo 'No ~/.ssh directory found.'; fi"#),
        ),
        Task::new(
            "Secret scan",
            "Runs gitleaks against the current directory to find committed secrets.",
            7,
            script(r#"if command -v gitleaks >/dev/null 2>&1; then echo "Directory: $PWD"; gitleaks detect --no-banner --redact -v; else echo 'gitleaks is not installed (Security tab).'; fi"#),
        )
        .with_timeout(120),
        Task::new(
            "Install Ansible",
            "Installs Ansible for configuration management and automation.",
            8,
            script(&pkg(pm, "ansible")),
        )
        .with_check(Check::Bin("ansible".to_string())),
        Task::new(
            "Install ansible-lint",
            "Installs ansible-lint for validating playbooks and roles.",
            8,
            script(&pkg(pm, "ansible-lint")),
        )
        .with_check(Check::Bin("ansible-lint".to_string())),
        Task::new(
            "Install Packer",
            "Installs HashiCorp Packer for building machine images.",
            8,
            script(&pkg(pm, "packer")),
        )
        .with_check(Check::Bin("packer".to_string())),
        Task::new(
            "Install Podman",
            "Installs Podman for daemonless container workflows.",
            8,
            script(&pkg(pm, "podman")),
        )
        .with_check(Check::Bin("podman".to_string())),
        Task::new(
            "Install kind",
            "Installs kind for running local Kubernetes clusters in Docker.",
            8,
            script(&pkg(pm, "kind")),
        )
        .with_check(Check::Bin("kind".to_string())),
        Task::new(
            "Install minikube",
            "Installs minikube for local Kubernetes development.",
            8,
            script(&pkg(pm, "minikube")),
        )
        .with_check(Check::Bin("minikube".to_string())),
        Task::new(
            "Docker containers",
            "Lists running containers with image, status, and published ports.",
            8,
            script(r#"if command -v docker >/dev/null 2>&1; then docker ps --format 'table {{.Names}}\t{{.Image}}\t{{.Status}}\t{{.Ports}}'; else echo 'Docker is not installed (Applications tab).'; fi"#),
        ),
        Task::new(
            "Docker images",
            "Lists local Docker images with size and age.",
            8,
            script(r#"if command -v docker >/dev/null 2>&1; then docker images --format 'table {{.Repository}}:{{.Tag}}\t{{.Size}}\t{{.CreatedSince}}'; else echo 'Docker is not installed (Applications tab).'; fi"#),
        ),
        Task::new(
            "Docker disk usage",
            "Shows how much disk space images, containers, and volumes use.",
            8,
            script(r#"if command -v docker >/dev/null 2>&1; then docker system df; else echo 'Docker is not installed (Applications tab).'; fi"#),
        ),
        Task::new(
            "Docker contexts",
            "Lists configured Docker contexts and the active one.",
            8,
            script(r#"if command -v docker >/dev/null 2>&1; then docker context ls; else echo 'Docker is not installed (Applications tab).'; fi"#),
        ),
        Task::new(
            "Kubernetes pods",
            "Lists pods across all namespaces with node and IP details.",
            8,
            script(r#"if command -v kubectl >/dev/null 2>&1; then kubectl get pods -A -o wide 2>&1; else echo 'kubectl is not installed (Applications tab).'; fi"#),
        )
        .with_timeout(30),
        Task::new(
            "Kubernetes events",
            "Shows the most recent cluster events.",
            8,
            script(r#"if command -v kubectl >/dev/null 2>&1; then kubectl get events -A --sort-by=.lastTimestamp 2>/dev/null | tail -30; else echo 'kubectl is not installed (Applications tab).'; fi"#),
        )
        .with_timeout(30),
        Task::new(
            "Helm releases",
            "Lists Helm releases across all namespaces.",
            8,
            script(r#"if command -v helm >/dev/null 2>&1; then helm list -A 2>&1; else echo 'Helm is not installed (Applications tab).'; fi"#),
        )
        .with_timeout(30),
        Task::new(
            "Ansible configuration",
            "Shows the Ansible version and any non-default configuration.",
            8,
            script(r#"if command -v ansible >/dev/null 2>&1; then ansible --version; echo; ansible-config dump --only-changed 2>/dev/null | head -50; else echo 'Ansible is not installed (DevOps tab).'; fi"#),
        ),
        Task::new(
            "Compose project status",
            "Shows the status of services in the current Docker Compose project.",
            8,
            script(r#"if docker compose version >/dev/null 2>&1; then docker compose ps; elif command -v docker-compose >/dev/null 2>&1; then docker-compose ps; elif command -v podman-compose >/dev/null 2>&1; then podman-compose ps; else echo 'No Compose implementation found.'; fi"#),
        )
        .with_timeout(30),
        Task::new(
            "Lint GitHub Actions",
            "Runs actionlint against the workflows in the current repository.",
            8,
            script(r#"if command -v actionlint >/dev/null 2>&1; then echo "Directory: $PWD"; actionlint; else echo 'actionlint is not installed (Dev tools tab).'; fi"#),
        ),
        Task::new(
            "Recent GitHub Actions runs",
            "Lists recent workflow runs for the current repository with the GitHub CLI.",
            8,
            script(r#"if command -v gh >/dev/null 2>&1; then gh run list --limit 10 2>&1; else echo 'GitHub CLI is not installed (Applications tab).'; fi"#),
        )
        .with_timeout(30),
        Task::new(
            "Container runtime info",
            "Shows a short summary of the active Docker or Podman runtime.",
            8,
            script(r#"if command -v docker >/dev/null 2>&1; then docker info --format 'Server: {{.ServerVersion}}\nStorage: {{.Driver}}\nContainers: {{.Containers}} (running {{.ContainersRunning}})\nImages: {{.Images}}' 2>&1; elif command -v podman >/dev/null 2>&1; then podman info --format '{{.Host.Hostname}} {{.Version.Version}}' 2>&1; else echo 'No Docker or Podman runtime found.'; fi"#),
        ),
    ];
    for (i, p) in PALETTES.iter().enumerate() {
        tasks.push(Task::new(p.name, p.desc, 2, Action::Theme(i)));
    }

    let (custom, warning) = load_custom_tasks();
    tasks.extend(custom);
    if let Some(warning) = warning {
        tasks.push(Task::new(
            "Custom tasks: config problem",
            "The custom task file could not be loaded. Run this task to see the error.",
            3,
            Action::Message(warning),
        ));
    }
    tasks
}

#[derive(Deserialize)]
struct CustomFile {
    #[serde(default)]
    tasks: Vec<CustomEntry>,
}

#[derive(Deserialize)]
struct CustomEntry {
    name: String,
    #[serde(default, alias = "description")]
    desc: String,
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    script: Option<String>,
    #[serde(default)]
    section: Option<String>,
    #[serde(default)]
    tab: Option<usize>,
    #[serde(default)]
    check: Option<String>,
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default)]
    timeout: Option<u64>,
    #[serde(default, alias = "confirm")]
    destructive: bool,
}

fn tab_index(section: &str) -> Option<usize> {
    match section.trim().to_lowercase().as_str() {
        "application" | "applications" | "apps" | "app" => Some(0),
        "bash" | "bash config" | "config" | "dotfiles" => Some(1),
        "theme" | "themes" => Some(2),
        "system" | "sys" => Some(3),
        "diagnostics" | "diagnostic" | "diag" | "system info" => Some(4),
        "dev" | "dev tools" | "development" | "developer" => Some(5),
        "cloud" | "cloud engineering" | "cloud engineer" => Some(6),
        "security" | "sec" | "cyber" | "cybersecurity" => Some(7),
        "devops" | "dev ops" | "ops" => Some(8),
        _ => None,
    }
}

fn parse_custom_tasks(text: &str) -> Result<Vec<Task>, String> {
    let file: CustomFile = toml::from_str(text).map_err(|e| format!("invalid TOML: {}", e))?;
    let mut tasks = Vec::new();
    for (i, entry) in file.tasks.into_iter().enumerate() {
        let command = entry
            .command
            .or(entry.script)
            .filter(|c| !c.trim().is_empty())
            .ok_or_else(|| format!("task #{} ('{}') has no command", i + 1, entry.name))?;
        let tab = match (entry.tab, entry.section.as_deref()) {
            (Some(tab), _) => {
                if tab >= TABS.len() {
                    return Err(format!(
                        "task #{} ('{}') uses tab {}, but only 0-{} exist",
                        i + 1,
                        entry.name,
                        tab,
                        TABS.len() - 1
                    ));
                }
                tab
            }
            (None, Some(section)) => tab_index(section).ok_or_else(|| {
                format!(
                    "task #{} ('{}') has unknown section '{}' (use {})",
                    i + 1,
                    entry.name,
                    section,
                    TABS.join(", ")
                )
            })?,
            (None, None) => 3,
        };
        let check = match entry.check.as_deref() {
            Some("fonts") => Some(Check::Fonts),
            Some(bin) if !bin.trim().is_empty() => Some(Check::Bin(bin.trim().to_string())),
            _ => None,
        };
        let prompt = entry
            .prompt
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty());
        if prompt.is_some() && !command.contains("{}") {
            return Err(format!(
                "task #{} ('{}') has a prompt but no '{{}}' placeholder in its command",
                i + 1,
                entry.name
            ));
        }
        let timeout = match entry.timeout {
            Some(0) => {
                return Err(format!(
                    "task #{} ('{}') has timeout = 0, which is invalid",
                    i + 1,
                    entry.name
                ))
            }
            other => other,
        };
        let desc = if entry.desc.trim().is_empty() {
            "Custom task from BashBox.".to_string()
        } else {
            entry.desc
        };
        tasks.push(Task {
            name: entry.name,
            desc,
            tab,
            action: script(&command),
            check,
            prompt,
            timeout,
            destructive: entry.destructive,
            custom: true,
        });
    }
    Ok(tasks)
}

pub fn custom_tasks_dir() -> Option<PathBuf> {
    let base = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| home_dir().map(|h| h.join(".config")))?;
    Some(base.join("bashbox").join("tasks.d"))
}

fn custom_config_paths() -> Vec<PathBuf> {
    if let Some(path) = env::var_os("BASHBOX_TASKS") {
        return vec![PathBuf::from(path)];
    }
    let mut paths = Vec::new();
    if let Some(base) = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| home_dir().map(|h| h.join(".config")))
    {
        let main = base.join("bashbox").join("tasks.toml");
        if main.is_file() {
            paths.push(main);
        }
        let dir = base.join("bashbox").join("tasks.d");
        if let Ok(entries) = fs::read_dir(dir) {
            let mut extra: Vec<PathBuf> = entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "toml"))
                .collect();
            extra.sort();
            paths.extend(extra);
        }
    }
    paths
}

fn load_custom_tasks() -> (Vec<Task>, Option<String>) {
    let mut tasks = Vec::new();
    let mut warnings = Vec::new();
    for path in custom_config_paths() {
        match fs::read_to_string(&path) {
            Ok(text) => match parse_custom_tasks(&text) {
                Ok(mut parsed) => tasks.append(&mut parsed),
                Err(e) => warnings.push(format!("Could not load {}: {}", path.display(), e)),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => warnings.push(format!("Could not read {}: {}", path.display(), e)),
        }
    }
    let warning = if warnings.is_empty() {
        None
    } else {
        Some(warnings.join("\n\n"))
    };
    (tasks, warning)
}

pub fn fetch_tasks(url: &str) -> Result<PathBuf, String> {
    let output = Command::new("curl")
        .args(["-fsSL", "--max-time", "20", url])
        .output()
        .map_err(|e| format!("could not run curl: {}", e))?;
    if !output.status.success() {
        return Err(format!(
            "download failed (exit {}): {}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let text = String::from_utf8(output.stdout)
        .map_err(|_| "downloaded file is not valid UTF-8".to_string())?;
    parse_custom_tasks(&text)?;

    let name = url
        .split('?')
        .next()
        .and_then(|u| u.rsplit('/').next())
        .filter(|n| !n.is_empty() && n.ends_with(".toml"))
        .unwrap_or("remote.toml")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        .collect::<String>();
    let name = if name.ends_with(".toml") {
        name
    } else {
        "remote.toml".to_string()
    };
    let dir = custom_tasks_dir().ok_or_else(|| "HOME is not set".to_string())?;
    fs::create_dir_all(&dir).map_err(|e| format!("could not create {}: {}", dir.display(), e))?;
    let path = dir.join(name);
    fs::write(&path, text).map_err(|e| format!("could not write {}: {}", path.display(), e))?;
    Ok(path)
}

pub fn last_log_path() -> Option<PathBuf> {
    failure_log_path()
}

fn failure_log_path() -> Option<PathBuf> {
    let base = env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| home_dir().map(|h| h.join(".local/state")))?;
    Some(base.join("bashbox").join("last-task.log"))
}

fn write_failure_log(name: &str, lines: &[String], code: i32) -> Option<PathBuf> {
    let path = failure_log_path()?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).ok()?;
    }
    let mut content = format!("# BashBox task: {}\n# exit status: {}\n\n", name, code);
    for line in lines {
        content.push_str(line);
        content.push('\n');
    }
    fs::write(&path, content).ok()?;
    Some(path)
}

pub fn spawn(name: String, action: Action, timeout: Option<u64>) -> Receiver<EventMsg> {
    let (tx, rx) = channel::<EventMsg>();
    thread::spawn(move || {
        let tx_lines = tx.clone();
        let send = move |line: String| {
            let _ = tx_lines.send(EventMsg::Line(line));
        };
        match action {
            Action::Script(cmds) => {
                let mut command = match timeout {
                    Some(secs) => {
                        let mut command = Command::new("timeout");
                        command.arg(secs.to_string()).arg("sh");
                        command
                    }
                    None => Command::new("sh"),
                };
                let child = command
                    .arg("-ec")
                    .arg(&cmds)
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .spawn();
                let mut child = match child {
                    Ok(c) => c,
                    Err(e) => {
                        let line = format!("Failed to start task: {}", e);
                        send(line.clone());
                        let log = write_failure_log(&name, &[line], -1);
                        let _ = tx.send(EventMsg::Done { failed: true, log });
                        return;
                    }
                };
                if child.id() != 0 {
                    let _ = tx.send(EventMsg::Pid(child.id()));
                }
                let mut lines = Vec::new();
                if let Some(out) = child.stdout.take() {
                    let reader = BufReader::new(out);
                    for line in reader.lines() {
                        match line {
                            Ok(l) => {
                                lines.push(l.clone());
                                send(l);
                            }
                            Err(_) => break,
                        }
                    }
                }
                let code = child.wait().ok().and_then(|s| s.code()).unwrap_or(-1);
                let failed = code != 0;
                let log = if failed {
                    write_failure_log(&name, &lines, code)
                } else {
                    None
                };
                send(format!("Exit status: {}", code));
                let _ = tx.send(EventMsg::Done { failed, log });
            }
            Action::Theme(idx) => {
                let tx_theme = tx.clone();
                crate::themes::switch_theme(idx, move |msg| {
                    let _ = tx_theme.send(EventMsg::Line(msg));
                });
                let _ = tx.send(EventMsg::Done {
                    failed: false,
                    log: None,
                });
            }
            Action::Builtin(b) => {
                run_builtin(b, &send);
                let _ = tx.send(EventMsg::Done {
                    failed: false,
                    log: None,
                });
            }
            Action::Message(text) => {
                for line in text.lines() {
                    send(line.to_string());
                }
                let _ = tx.send(EventMsg::Done {
                    failed: false,
                    log: None,
                });
            }
            Action::Edit(path) => {
                send(format!(
                    "Cannot edit {} in a background task.",
                    path.display()
                ));
                let _ = tx.send(EventMsg::Done {
                    failed: true,
                    log: None,
                });
            }
        }
    });
    rx
}

fn run_builtin<F: Fn(String)>(b: Builtin, send: &F) {
    match b {
        Builtin::Refresh => refresh(send),
        Builtin::Uninstall => uninstall(send),
        Builtin::ToggleBanner => toggle_banner(send),
        Builtin::Doctor => doctor(send),
        Builtin::LastLog => last_log(send),
        Builtin::Recent => recent(send),
        Builtin::ResetState => reset_state(send),
        Builtin::ClearLog => clear_log(send),
    }
}

fn reset_state<F: Fn(String)>(send: &F) {
    crate::state::save(&crate::state::State::default());
    send("Toolbox state reset: pins, recent runs, and pane width cleared.".to_string());
    send("Reopen the toolbox to see the defaults.".to_string());
}

fn clear_log<F: Fn(String)>(send: &F) {
    let Some(path) = failure_log_path() else {
        send("HOME is not set.".to_string());
        return;
    };
    match fs::remove_file(&path) {
        Ok(()) => send(format!("Removed {}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            send("No failed task log to remove.".to_string());
        }
        Err(e) => send(format!("Could not remove {}: {}", path.display(), e)),
    }
}

fn last_log<F: Fn(String)>(send: &F) {
    let Some(path) = failure_log_path() else {
        send("HOME is not set.".to_string());
        return;
    };
    match fs::read_to_string(&path) {
        Ok(text) => {
            send(path.display().to_string());
            send(String::new());
            for line in text.lines() {
                send(line.to_string());
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            send("No failed task has been recorded yet.".to_string());
        }
        Err(e) => send(format!("Could not read {}: {}", path.display(), e)),
    }
}

fn recent<F: Fn(String)>(send: &F) {
    let state = crate::state::load();
    if state.recent.is_empty() {
        send("No tasks have been run yet.".to_string());
        return;
    }
    send("Recently run tasks:".to_string());
    for (i, name) in state.recent.iter().enumerate() {
        send(format!("  {}. {}", i + 1, name));
    }
}

fn backup_path(dest: &Path) -> PathBuf {
    let name = dest.file_name().and_then(|n| n.to_str()).unwrap_or("");
    dest.with_file_name(format!("{}.bashbox-backup", name))
}

fn link(src: &Path, dest: &Path, send: &impl Fn(String)) {
    if let Some(parent) = dest.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(md) = fs::symlink_metadata(dest) {
        if !md.is_symlink() {
            let bak = backup_path(dest);
            if fs::rename(dest, &bak).is_ok() {
                send(format!("Backed up {}", dest.display()));
            }
        }
    }
    let _ = fs::remove_file(dest);
    match fs_symlink(src, dest) {
        Ok(()) => send(format!("Linked {} -> {}", dest.display(), src.display())),
        Err(e) => send(format!("Failed to link {}: {}", dest.display(), e)),
    }
}

fn refresh<F: Fn(String)>(send: &F) {
    let home = match home_dir() {
        Some(h) => h,
        None => {
            send("HOME is not set.".to_string());
            return;
        }
    };
    let install = home.join(".local/share/bashbox");
    if !install.join(".bashrc").exists() {
        send(
            "bashbox is not installed yet (~/.local/share/bashbox missing). Run setup.sh first."
                .to_string(),
        );
        return;
    }
    send("Refreshing dotfiles...".to_string());
    link(&install.join(".bashrc"), &home.join(".bashrc"), send);
    link(
        &install.join("starship.toml"),
        &home.join(".config/starship.toml"),
        send,
    );
    link(
        &install.join("fastfetch/config.jsonc"),
        &home.join(".config/fastfetch/config.jsonc"),
        send,
    );
    send("Dotfiles refreshed. Open a new terminal.".to_string());
}

fn restore(dest: &Path, send: &impl Fn(String)) {
    let is_link = fs::symlink_metadata(dest)
        .map(|md| md.is_symlink())
        .unwrap_or(false);
    if is_link {
        let _ = fs::remove_file(dest);
        send(format!("Removed link {}", dest.display()));
        let bak = backup_path(dest);
        if bak.exists() {
            let _ = fs::rename(&bak, dest);
            send(format!("Restored {}", dest.display()));
        } else {
            send(format!("No backup found for {}", dest.display()));
        }
    } else if dest.exists() {
        send(format!(
            "{} is not managed by bashbox, skipped",
            dest.display()
        ));
    }
}

fn uninstall<F: Fn(String)>(send: &F) {
    let home = match home_dir() {
        Some(h) => h,
        None => {
            send("HOME is not set.".to_string());
            return;
        }
    };
    restore(&home.join(".bashrc"), send);
    restore(&home.join(".config/starship.toml"), send);
    restore(&home.join(".config/fastfetch/config.jsonc"), send);
    let install = home.join(".local/share/bashbox");
    if fs::remove_dir_all(&install).is_ok() {
        send(format!("Removed {}", install.display()));
    }
    for font in FONTS {
        let dir = font_dir(font);
        if fs::remove_dir_all(&dir).is_ok() {
            send(format!("Removed {}", dir.display()));
        }
    }
    send("Uninstalled. Installed packages were left in place. Open a new terminal.".to_string());
}

fn toggle_banner<F: Fn(String)>(send: &F) {
    let home = match home_dir() {
        Some(h) => h,
        None => {
            send("HOME is not set.".to_string());
            return;
        }
    };
    let rc = home.join(".bashrc");
    let marker = |l: &str| l.trim_start().starts_with("export") && l.contains("BASHBOX_NO_FETCH=1");
    match fs::read_to_string(&rc) {
        Ok(content) => {
            if content.lines().any(marker) {
                let out: String = content
                    .lines()
                    .filter(|l| !marker(l))
                    .collect::<Vec<_>>()
                    .join("\n")
                    + "\n";
                let _ = fs::write(&rc, out);
                send(
                    "Fastfetch banner enabled (removed BASHBOX_NO_FETCH from .bashrc).".to_string(),
                );
            } else {
                let out =
                    content.trim_end_matches('\n').to_string() + "\nexport BASHBOX_NO_FETCH=1\n";
                let _ = fs::write(&rc, out);
                send(
                    "Fastfetch banner disabled (BASHBOX_NO_FETCH=1 added to .bashrc).".to_string(),
                );
            }
        }
        Err(e) => send(format!("Could not read .bashrc: {}", e)),
    }
}

pub fn doctor<F: Fn(String)>(send: &F) {
    send("bashbox doctor".to_string());
    send(String::new());
    let tools = [
        ("prompt", "starship"),
        ("banner", "fastfetch"),
        ("fuzzy finder", "fzf"),
        ("smart cd", "zoxide"),
        ("editor", "nvim"),
        ("safe delete", "trash-put"),
        ("git", "git"),
        ("network", "curl"),
        ("archives", "unzip"),
        ("copies", "rsync"),
    ];
    for (label, bin) in tools {
        if which(bin) {
            send(format!("[ok]   {:<12} {}", label, bin));
        } else {
            send(format!("[miss] {:<12} {} (run setup.sh)", label, bin));
        }
    }
    send(String::new());
    if let Some(home) = home_dir() {
        let checks: [(&str, PathBuf); 4] = [
            (".bashrc", home.join(".bashrc")),
            ("starship.toml", home.join(".config/starship.toml")),
            (
                "fastfetch config",
                home.join(".config/fastfetch/config.jsonc"),
            ),
            ("dotfiles copy", home.join(".local/share/bashbox/.bashrc")),
        ];
        for (label, path) in checks {
            match fs::symlink_metadata(&path) {
                Ok(md) if md.is_symlink() => send(format!("[ok]   {} linked", label)),
                Ok(_) => send(format!(
                    "[warn] {} exists but is not a bashbox symlink",
                    label
                )),
                Err(_) => send(format!("[miss] {} not found", label)),
            }
        }
        let mut installed = 0;
        for font in FONTS {
            if font_dir(font).is_dir() {
                installed += 1;
            }
        }
        send(format!(
            "[info] Nerd Fonts installed: {}/{} (JetBrainsMono, FiraCode, CascadiaCode, Hack)",
            installed,
            FONTS.len()
        ));
    }
    send(String::new());
    send("Tip: run `sudo -v` before launching the toolbox to skip password prompts.".to_string());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_custom_tasks() {
        let tasks = parse_custom_tasks(
            r#"
[[tasks]]
name = "Disk usage"
description = "Show disk usage"
command = "df -h"
section = "System"
check = "df"

[[tasks]]
name = "Theme preview"
script = "echo hi"
tab = 2
"#,
        )
        .expect("valid config");

        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].name, "Disk usage");
        assert_eq!(tasks[0].desc, "Show disk usage");
        assert_eq!(tasks[0].tab, 3);
        assert_eq!(tasks[0].check, Some(Check::Bin("df".to_string())));
        assert_eq!(tasks[1].tab, 2);
        assert_eq!(tasks[1].desc, "Custom task from BashBox.");
    }

    #[test]
    fn rejects_task_without_command() {
        let err = parse_custom_tasks(
            r#"
[[tasks]]
name = "Broken"
"#,
        )
        .expect_err("missing command should fail");
        assert!(err.contains("no command"));
    }

    #[test]
    fn rejects_unknown_section() {
        let err = parse_custom_tasks(
            r#"
[[tasks]]
name = "Broken"
command = "true"
section = "Nope"
"#,
        )
        .expect_err("unknown section should fail");
        assert!(err.contains("unknown section"));
    }

    #[test]
    fn maps_sections() {
        assert_eq!(tab_index("Apps"), Some(0));
        assert_eq!(tab_index("bash config"), Some(1));
        assert_eq!(tab_index("Themes"), Some(2));
        assert_eq!(tab_index("system"), Some(3));
        assert_eq!(tab_index("Diagnostics"), Some(4));
        assert_eq!(tab_index("dev tools"), Some(5));
        assert_eq!(tab_index("cloud"), Some(6));
        assert_eq!(tab_index("cybersecurity"), Some(7));
        assert_eq!(tab_index("devops"), Some(8));
        assert_eq!(tab_index("nope"), None);
    }

    #[test]
    fn parses_prompt_timeout_and_destructive() {
        let tasks = parse_custom_tasks(
            r#"
[[tasks]]
name = "Ping host"
command = "ping -c 1 {}"
prompt = "Host to ping"
timeout = 10
destructive = true
section = "diagnostics"
"#,
        )
        .expect("valid config");

        assert_eq!(tasks[0].tab, 4);
        assert_eq!(tasks[0].prompt.as_deref(), Some("Host to ping"));
        assert_eq!(tasks[0].timeout, Some(10));
        assert!(tasks[0].destructive);
        assert!(tasks[0].custom);
        assert_eq!(
            tasks[0].command_with_arg("example.com").as_deref(),
            Some("SUDO=; [ \"$(id -u)\" -eq 0 ] || SUDO=sudo; ( ping -c 1 'example.com' ) 2>&1")
        );
    }

    #[test]
    fn quotes_prompt_values() {
        assert_eq!(shell_quote("hello world"), "'hello world'");
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
    }

    #[test]
    fn rejects_prompt_without_placeholder() {
        let err = parse_custom_tasks(
            r#"
[[tasks]]
name = "Broken"
command = "true"
prompt = "Value"
"#,
        )
        .expect_err("prompt without placeholder should fail");
        assert!(err.contains("placeholder"));
    }

    #[test]
    fn rejects_zero_timeout() {
        let err = parse_custom_tasks(
            r#"
[[tasks]]
name = "Broken"
command = "true"
timeout = 0
"#,
        )
        .expect_err("zero timeout should fail");
        assert!(err.contains("timeout = 0"));
    }

    #[test]
    fn builtin_tasks_are_unique_and_well_formed() {
        let tasks = build();
        let builtins: Vec<&Task> = tasks.iter().filter(|task| !task.custom).collect();
        let mut names: Vec<&str> = builtins.iter().map(|task| task.name.as_str()).collect();
        names.sort_unstable();
        let unique = names.len();
        names.dedup();
        assert_eq!(names.len(), unique, "task names must be unique");
        for task in &builtins {
            assert!(!task.name.trim().is_empty());
            assert!(!task.desc.trim().is_empty());
            assert!(task.tab < TABS.len(), "{} has an invalid tab", task.name);
        }
        for (tab, name) in TABS.iter().enumerate() {
            assert!(
                builtins.iter().any(|task| task.tab == tab),
                "{} tab has no tasks",
                name
            );
        }
    }

    #[test]
    fn short_tabs_match_full_tabs() {
        assert_eq!(TAB_SHORT.len(), TABS.len());
        assert!(TAB_SHORT.iter().all(|name| !name.is_empty()));
        assert!(TAB_SHORT
            .iter()
            .zip(TABS)
            .all(|(short, full)| short.len() <= full.len()));
    }

    #[test]
    fn installer_tasks_have_status_checks() {
        for task in build() {
            if task.name.starts_with("Install ") {
                assert!(task.check.is_some(), "{} has no status check", task.name);
            }
        }
    }

    #[test]
    fn any_check_accepts_known_binaries() {
        let task = Task::new("test", "test", 0, Action::Message(String::new()))
            .with_any_check(&["definitely-not-a-real-binary-xyz", "sh"]);
        let status = task.status().expect("check status");
        assert_eq!(status.kind, StatusKind::Installed);
    }

    #[test]
    fn any_check_reports_missing() {
        let task = Task::new("test", "test", 0, Action::Message(String::new()))
            .with_any_check(&["definitely-not-a-real-binary-xyz"]);
        let status = task.status().expect("check status");
        assert_eq!(status.kind, StatusKind::Missing);
    }
}
