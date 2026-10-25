use std::env;
use std::fs;
use std::io::{BufRead, BufReader};
use std::os::unix::fs::symlink as fs_symlink;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::thread;

use crate::themes::PALETTES;

pub const TABS: [&str; 4] = ["Applications", "Bash config", "Themes", "System"];

const FONTS: [&str; 4] = ["JetBrainsMono", "FiraCode", "CascadiaCode", "Hack"];

#[derive(Clone)]
pub enum Action {
    Script(String),
    Theme(usize),
    Builtin(Builtin),
}

#[derive(Clone, Copy)]
pub enum Builtin {
    Refresh,
    Uninstall,
    ToggleBanner,
    Doctor,
}

pub struct Task {
    pub name: &'static str,
    pub desc: &'static str,
    pub tab: usize,
    pub action: Action,
}

pub enum EventMsg {
    Pid(u32),
    Line(String),
    Done,
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

fn pkg(pm: &str, pkgs: &str) -> String {
    match pm {
        "apt" => format!("$SUDO apt-get install -y {}", pkgs),
        "dnf" => format!("$SUDO dnf install -y {}", pkgs),
        "pacman" => format!("$SUDO pacman -S --needed --noconfirm {}", pkgs),
        "zypper" => format!("$SUDO zypper --non-interactive install {}", pkgs),
        _ => "echo 'No supported package manager found'".to_string(),
    }
}

fn update_cmd(pm: &str) -> String {
    match pm {
        "apt" => "$SUDO apt-get update && $SUDO apt-get upgrade -y && $SUDO apt-get autoremove -y"
            .to_string(),
        "dnf" => "$SUDO dnf upgrade -y".to_string(),
        "pacman" => "$SUDO pacman -Syu".to_string(),
        "zypper" => "$SUDO zypper refresh && $SUDO zypper update -y".to_string(),
        _ => "echo 'No supported package manager found'".to_string(),
    }
}

fn cleanup_cmd(pm: &str) -> String {
    match pm {
        "apt" => "$SUDO apt-get autoremove -y && $SUDO apt-get clean".to_string(),
        "dnf" => "$SUDO dnf clean all".to_string(),
        "pacman" => "$SUDO pacman -Sc --noconfirm".to_string(),
        "zypper" => "$SUDO zypper clean --all".to_string(),
        _ => "echo 'No supported package manager found'".to_string(),
    }
}

pub fn build() -> Vec<Task> {
    let pm = detect_pm();
    let mut tasks = vec![
        Task {
            name: "Install Docker",
            desc: "Runs the official Docker convenience script and adds you to the docker group. Log out afterwards for the group to apply.",
            tab: 0,
            action: script("curl -fsSL https://get.docker.com | $SUDO sh && $SUDO usermod -aG docker \"$(id -un)\" && echo 'Log out and back in for the docker group to apply.'"),
        },
        Task {
            name: "Install Node.js",
            desc: "Installs nodejs and npm via your package manager.",
            tab: 0,
            action: script(&pkg(pm, "nodejs npm")),
        },
        Task {
            name: "Install Git",
            desc: "Installs git via your package manager.",
            tab: 0,
            action: script(&pkg(pm, "git")),
        },
        Task {
            name: "Install GitHub CLI",
            desc: "Installs the gh CLI via your package manager.",
            tab: 0,
            action: script(&pkg(pm, "gh")),
        },
        Task {
            name: "Install htop",
            desc: "Installs the htop process viewer.",
            tab: 0,
            action: script(&pkg(pm, "htop")),
        },
        Task {
            name: "Install tmux",
            desc: "Installs the tmux terminal multiplexer.",
            tab: 0,
            action: script(&pkg(pm, "tmux")),
        },
        Task {
            name: "Install ripgrep",
            desc: "Installs rg, a fast code search tool.",
            tab: 0,
            action: script(&pkg(pm, "ripgrep")),
        },
        Task {
            name: "Install bat",
            desc: "Installs bat, a cat clone with syntax highlighting.",
            tab: 0,
            action: script(&pkg(pm, "bat")),
        },
        Task {
            name: "Install jq",
            desc: "Installs jq, a command-line JSON processor.",
            tab: 0,
            action: script(&pkg(pm, "jq")),
        },
        Task {
            name: "Refresh dotfiles",
            desc: "Re-links ~/.bashrc, starship.toml and the fastfetch config from ~/.local/share/bashbox.",
            tab: 1,
            action: Action::Builtin(Builtin::Refresh),
        },
        Task {
            name: "Uninstall bashbox",
            desc: "Restores your .bashbox-backup files and removes bashbox config copies and font installs.",
            tab: 1,
            action: Action::Builtin(Builtin::Uninstall),
        },
        Task {
            name: "Toggle fastfetch banner",
            desc: "Adds or removes BASHBOX_NO_FETCH=1 in ~/.bashrc to turn the startup banner off or on.",
            tab: 1,
            action: Action::Builtin(Builtin::ToggleBanner),
        },
        Task {
            name: "Test shell startup",
            desc: "Opens a throwaway interactive Bash to verify the prompt and banner load correctly.",
            tab: 1,
            action: script("bash -ic 'echo \"Interactive shell loaded.\"'"),
        },
        Task {
            name: "Update system",
            desc: "Upgrades all packages via the detected package manager.",
            tab: 3,
            action: script(&update_cmd(pm)),
        },
        Task {
            name: "Clean package cache",
            desc: "Removes orphaned packages and the download cache.",
            tab: 3,
            action: script(&cleanup_cmd(pm)),
        },
        Task {
            name: "Run doctor",
            desc: "Checks all bashbox dependencies, config links, and fonts, and reports what is missing.",
            tab: 3,
            action: Action::Builtin(Builtin::Doctor),
        },
        Task {
            name: "Install Nerd Fonts",
            desc: "Downloads JetBrainsMono, FiraCode, CascadiaCode and Hack Nerd Fonts to ~/.local/share/fonts.",
            tab: 3,
            action: script(&format!(
                "for font in {}; do d=\"$HOME/.local/share/fonts/${{font}}NF\"; if [ -d \"$d\" ]; then continue; fi; echo \"Installing $font Nerd Font...\"; curl -fsSL -o \"/tmp/${{font}}.zip\" \"https://github.com/ryanoasis/nerd-fonts/releases/latest/download/${{font}}.zip\" && unzip -oq \"/tmp/${{font}}.zip\" -d \"$d\"; rm -f \"/tmp/${{font}}.zip\"; done; if command -v fc-cache >/dev/null; then fc-cache -f; fi",
                FONTS.join(" ")
            )),
        },
    ];
    for (i, p) in PALETTES.iter().enumerate() {
        tasks.push(Task {
            name: p.name,
            desc: p.desc,
            tab: 2,
            action: Action::Theme(i),
        });
    }
    tasks
}

pub fn spawn(action: Action) -> Receiver<EventMsg> {
    let (tx, rx) = channel::<EventMsg>();
    thread::spawn(move || {
        let tx_lines = tx.clone();
        let send = move |line: String| {
            let _ = tx_lines.send(EventMsg::Line(line));
        };
        match action {
            Action::Script(cmds) => {
                let child = Command::new("sh")
                    .arg("-ec")
                    .arg(&cmds)
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .spawn();
                let mut child = match child {
                    Ok(c) => c,
                    Err(e) => {
                        send(format!("Failed to start task: {}", e));
                        let _ = tx.send(EventMsg::Done);
                        return;
                    }
                };
                if child.id() != 0 {
                    let _ = tx.send(EventMsg::Pid(child.id()));
                }
                if let Some(out) = child.stdout.take() {
                    let reader = BufReader::new(out);
                    for line in reader.lines() {
                        match line {
                            Ok(l) => send(l),
                            Err(_) => break,
                        }
                    }
                }
                let code = child.wait().ok().and_then(|s| s.code()).unwrap_or(-1);
                send(format!("Exit status: {}", code));
            }
            Action::Theme(idx) => {
                let tx_theme = tx.clone();
                crate::themes::switch_theme(idx, move |msg| {
                    let _ = tx_theme.send(EventMsg::Line(msg));
                });
            }
            Action::Builtin(b) => run_builtin(b, &send),
        }
        let _ = tx.send(EventMsg::Done);
    });
    rx
}

fn run_builtin<F: Fn(String)>(b: Builtin, send: &F) {
    match b {
        Builtin::Refresh => refresh(send),
        Builtin::Uninstall => uninstall(send),
        Builtin::ToggleBanner => toggle_banner(send),
        Builtin::Doctor => doctor(send),
    }
}

fn home_dir() -> Option<PathBuf> {
    env::var("HOME")
        .ok()
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
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
        let dir = home.join(format!(".local/share/fonts/{}NF", font));
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
    match fs::read_to_string(&rc) {
        Ok(content) => {
            if content.lines().any(|l| l.contains("BASHBOX_NO_FETCH=1")) {
                let out: String = content
                    .lines()
                    .filter(|l| !l.contains("BASHBOX_NO_FETCH=1"))
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

fn doctor<F: Fn(String)>(send: &F) {
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
        let checks: [(&str, PathBuf, bool); 4] = [
            (".bashrc", home.join(".bashrc"), true),
            ("starship.toml", home.join(".config/starship.toml"), true),
            (
                "fastfetch config",
                home.join(".config/fastfetch/config.jsonc"),
                true,
            ),
            (
                "dotfiles copy",
                home.join(".local/share/bashbox/.bashrc"),
                true,
            ),
        ];
        for (label, path, _) in checks {
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
            if home.join(format!(".local/share/fonts/{}NF", font)).is_dir() {
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
