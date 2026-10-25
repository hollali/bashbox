pub struct Palette {
    pub name: &'static str,
    pub desc: &'static str,
    pub colors: [(&'static str, &'static str); 6],
}

pub const PALETTES: &[Palette] = &[
    Palette {
        name: "nord",
        desc: "Rewrites starship.toml with the Nord palette (default).",
        colors: [
            ("blue", "#81A1C1"),
            ("cyan", "#88C0D0"),
            ("green", "#A3BE8C"),
            ("yellow", "#EBCB8B"),
            ("red", "#BF616A"),
            ("purple", "#B48EAD"),
        ],
    },
    Palette {
        name: "dracula",
        desc: "Rewrites starship.toml with the Dracula palette.",
        colors: [
            ("blue", "#6272A4"),
            ("cyan", "#8BE9FD"),
            ("green", "#50FA7B"),
            ("yellow", "#F1FA8C"),
            ("red", "#FF5555"),
            ("purple", "#BD93F9"),
        ],
    },
    Palette {
        name: "gruvbox",
        desc: "Rewrites starship.toml with the Gruvbox palette.",
        colors: [
            ("blue", "#83A598"),
            ("cyan", "#8EC07C"),
            ("green", "#B8BB26"),
            ("yellow", "#FABD2F"),
            ("red", "#FB4934"),
            ("purple", "#D3869B"),
        ],
    },
    Palette {
        name: "catppuccin",
        desc: "Rewrites starship.toml with the Catppuccin Mocha palette.",
        colors: [
            ("blue", "#89B4FA"),
            ("cyan", "#94E2D5"),
            ("green", "#A6E3A1"),
            ("yellow", "#F9E2AF"),
            ("red", "#F38BA8"),
            ("purple", "#CBA6F7"),
        ],
    },
    Palette {
        name: "tokyonight",
        desc: "Rewrites starship.toml with the Tokyo Night palette.",
        colors: [
            ("blue", "#7AA2F7"),
            ("cyan", "#7DCFFF"),
            ("green", "#9ECE6A"),
            ("yellow", "#E0AF68"),
            ("red", "#F7768E"),
            ("purple", "#BB9AF7"),
        ],
    },
];

const TEMPLATE: &str = r#"add_newline = true
command_timeout = 1000
palette = 'nord'

format = """
[╭─](bold blue)$username$hostname$directory$git_branch$git_status$python$nodejs$php$java$docker_context$cmd_duration
[╰─](bold blue)$character"""

{{PALETTE_BLOCK}}

[username]
show_always = false
style_user = 'bold purple'
format = '[$user]($style) '

[hostname]
ssh_only = true
style = 'bold purple'
format = '[@$hostname]($style) '

[directory]
style = 'bold cyan'
truncation_length = 3
truncation_symbol = '…/'
format = '[$path]($style)[$read_only]($read_only_style) '

[git_branch]
style = 'bold purple'
format = '[$symbol$branch]($style) '

[git_status]
style = 'yellow'
format = '([\[$all_status$ahead_behind\]]($style) )'

[python]
style = 'yellow'
format = '[$symbol$version]($style) '

[nodejs]
style = 'green'
format = '[$symbol$version]($style) '

[php]
style = 'purple'
format = '[$symbol$version]($style) '

[java]
style = 'red'
format = '[$symbol$version]($style) '

[docker_context]
style = 'blue'
format = '[$symbol$context]($style) '

[cmd_duration]
min_time = 2000
style = 'yellow'
format = '[took $duration]($style) '

[character]
success_symbol = '[❯](bold green)'
error_symbol = '[❯](bold red)'
"#;

fn render(p: &Palette) -> String {
    let mut block = format!("[palettes.{}]\n", p.name);
    for (key, hex) in p.colors {
        block.push_str(&format!("{} = '{}'\n", key, hex));
    }
    TEMPLATE
        .replace("{{PALETTE_BLOCK}}", &block)
        .replace("palette = 'nord'", &format!("palette = '{}'", p.name))
}

pub fn switch_theme<F: Fn(String)>(idx: usize, send: F) {
    if idx >= PALETTES.len() {
        send("Unknown palette.".to_string());
        return;
    }
    let p = &PALETTES[idx];
    let home = std::env::var("HOME").unwrap_or_default();
    if home.is_empty() {
        send("HOME is not set.".to_string());
        return;
    }
    let cfg = std::path::PathBuf::from(home).join(".config/starship.toml");
    if let Some(dir) = cfg.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match std::fs::write(&cfg, render(p)) {
        Ok(()) => send(format!(
            "Switched starship palette to {}. Open a new terminal.",
            p.name
        )),
        Err(e) => send(format!("Could not write {}: {}", cfg.display(), e)),
    }
}
