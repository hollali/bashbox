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

fn palette_block(p: &Palette) -> String {
    render(p)
        .lines()
        .skip_while(|l| !l.trim_start().starts_with("[palettes."))
        .map(|l| l.to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

fn migrate_legacy(text: &str) -> String {
    let mut out = text.to_string();
    for (old, new) in [
        ("nord_blue", "blue"),
        ("nord_cyan", "cyan"),
        ("nord_green", "green"),
        ("nord_yellow", "yellow"),
        ("nord_red", "red"),
        ("nord_purple", "purple"),
    ] {
        out = out.replace(old, new);
    }
    out
}

pub fn switch_theme<F: Fn(String)>(idx: usize, send: F) {
    if idx >= PALETTES.len() {
        send("Unknown palette.".to_string());
        return;
    }
    let p = &PALETTES[idx];
    let home = match std::env::var("HOME") {
        Ok(h) if !h.is_empty() => h,
        _ => {
            send("HOME is not set.".to_string());
            return;
        }
    };
    let cfg = std::path::PathBuf::from(home).join(".config/starship.toml");
    if let Some(dir) = cfg.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let content = std::fs::read_to_string(&cfg).unwrap_or_default();
    if !content.contains("palette = '") {
        std::fs::write(&cfg, render(p)).map_or_else(
            |e| send(format!("Could not write {}: {}", cfg.display(), e)),
            |_| send(format!("Wrote starship.toml with the {} palette.", p.name)),
        );
        return;
    }
    let old = content
        .lines()
        .find_map(|l| {
            let t = l.trim_start();
            t.strip_prefix("palette = '")
                .map(|s| s.split('\'').next().unwrap_or("nord").to_string())
        })
        .unwrap_or_else(|| "nord".to_string());
    let hdr = format!("[palettes.{}]", old);

    let mut out_lines: Vec<String> = Vec::new();
    let mut block_done = false;
    let mut block_insert_at: Option<usize> = None;
    let mut in_block = false;
    for line in content.lines() {
        if line.trim_start().starts_with("palette =") {
            out_lines.push(format!("palette = '{}'", p.name));
            block_insert_at = Some(out_lines.len());
            continue;
        }
        if !block_done && line.trim() == hdr {
            for nl in palette_block(p).lines() {
                out_lines.push(nl.to_string());
            }
            block_done = true;
            in_block = true;
            continue;
        }
        if in_block {
            if line.starts_with('[') {
                in_block = false;
            } else {
                continue;
            }
        }
        out_lines.push(line.to_string());
    }
    if !block_done {
        if let Some(at) = block_insert_at {
            let block = palette_block(p);
            for (offset, nl) in block.lines().enumerate() {
                out_lines.insert(at + offset, nl.to_string());
            }
        }
    }
    let final_text = migrate_legacy(&(out_lines.join("\n") + "\n"));
    std::fs::write(&cfg, final_text).map_or_else(
        |e| send(format!("Could not write {}: {}", cfg.display(), e)),
        |_| {
            send(format!(
                "Switched starship palette to {}. Existing customisations preserved; open a new terminal.",
                p.name
            ))
        },
    );
}
