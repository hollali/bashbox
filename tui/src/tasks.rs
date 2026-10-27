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

pub const TABS: [&str; 5] = [
    "Applications",
    "Bash config",
    "Themes",
    "System",
    "Diagnostics",
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
}

#[derive(Clone, PartialEq, Debug)]
pub enum Check {
    Bin(String),
    Fonts,
}

#[derive(Clone, Copy, PartialEq)]
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
        .with_check(Check::Bin("bat".to_string())),
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
                    "task #{} ('{}') has unknown section '{}' (use Applications, Bash config, Themes, System, or Diagnostics)",
                    i + 1,
                    entry.name,
                    section
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
}
