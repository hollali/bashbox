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
    Palette {
        name: "ayu",
        desc: "Rewrites starship.toml with the Ayu palette.",
        colors: [
            ("blue", "#04A5E8"),
            ("cyan", "#9CDCFE"),
            ("green", "#98C379"),
            ("yellow", "#E5C07B"),
            ("red", "#E8B4BC"),
            ("purple", "#C4A7E7"),
        ],
    },
    Palette {
        name: "kanagawa",
        desc: "Rewrites starship.toml with the Kanagawa palette.",
        colors: [
            ("blue", "#71A6CB"),
            ("cyan", "#83CAF2"),
            ("green", "#C8CBD0"),
            ("yellow", "#D2A960"),
            ("red", "#FF5D62"),
            ("purple", "#BC83E3"),
        ],
    },
    Palette {
        name: "everblush",
        desc: "Rewrites starship.toml with the Everblush palette.",
        colors: [
            ("blue", "#7DAEA3"),
            ("cyan", "#C5E1E7"),
            ("green", "#A9C6EA"),
            ("yellow", "#F6C17E"),
            ("red", "#E6989E"),
            ("purple", "#B092B7"),
        ],
    },
    Palette {
        name: "cyberpunk",
        desc: "Rewrites starship.toml with the Cyberpunk palette.",
        colors: [
            ("blue", "#54A5F5"),
            ("cyan", "#54E5F5"),
            ("green", "#78C850"),
            ("yellow", "#F6D66A"),
            ("red", "#FF7B7B"),
            ("purple", "#AA5CAA"),
        ],
    },
    Palette {
        name: "material",
        desc: "Rewrites starship.toml with the Material palette.",
        colors: [
            ("blue", "#67C23A"),
            ("cyan", "#00BCD4"),
            ("green", "#74DD3B"),
            ("yellow", "#FDD835"),
            ("red", "#FF5252"),
            ("purple", "#AB47BC"),
        ],
    },
    Palette {
        name: "sonokai",
        desc: "Rewrites starship.toml with the Sonokai palette.",
        colors: [
            ("blue", "#02837A"),
            ("cyan", "#06B6D4"),
            ("green", "#93C763"),
            ("yellow", "#F5C676"),
            ("red", "#C0392B"),
            ("purple", "#B48EAD"),
        ],
    },
    Palette {
        name: "solarized",
        desc: "Rewrites starship.toml with the Solarized Light palette.",
        colors: [
            ("blue", "#268BD2"),
            ("cyan", "#2AA198"),
            ("green", "#859900"),
            ("yellow", "#B58900"),
            ("red", "#DC322F"),
            ("purple", "#6C71C4"),
        ],
    },
    Palette {
        name: "onehalf",
        desc: "Rewrites starship.toml with the One Half Light palette.",
        colors: [
            ("blue", "#0F3F87"),
            ("cyan", "#08A8BD"),
            ("green", "#48A9A7"),
            ("yellow", "#A5C261"),
            ("red", "#CB4B4B"),
            ("purple", "#8481B8"),
        ],
    },
    Palette {
        name: "nordic",
        desc: "Rewrites starship.toml with the Nordic palette.",
        colors: [
            ("blue", "#6BA6CD"),
            ("cyan", "#78D1DE"),
            ("green", "#A7C7E7"),
            ("yellow", "#E4C873"),
            ("red", "#E06C75"),
            ("purple", "#C5C5C5"),
        ],
    },
    Palette {
        name: "catppuccin-latte",
        desc: "Rewrites starship.toml with the Catppuccin Latte palette.",
        colors: [
            ("blue", "#8CA9CC"),
            ("cyan", "#8FA1B3"),
            ("green", "#8FBCBB"),
            ("yellow", "#D2B48C"),
            ("red", "#DDAE9F"),
            ("purple", "#C6A0F6"),
        ],
    },
    Palette {
        name: "catppuccin-frappe",
        desc: "Rewrites starship.toml with the Catppuccin Frappe palette.",
        colors: [
            ("blue", "#88A0BC"),
            ("cyan", "#8AA9BD"),
            ("green", "#A3BE8C"),
            ("yellow", "#E5C890"),
            ("red", "#ED823F"),
            ("purple", "#B48EAD"),
        ],
    },
    Palette {
        name: "catppuccin-caramel",
        desc: "Rewrites starship.toml with the Catppuccin Caramel palette.",
        colors: [
            ("blue", "#638C98"),
            ("cyan", "#6BA0A9"),
            ("green", "#A3BE8C"),
            ("yellow", "#E5C890"),
            ("red", "#BF616A"),
            ("purple", "#B48EAD"),
        ],
    },
    Palette {
        name: "one-dark",
        desc: "Rewrites starship.toml with the One Dark palette.",
        colors: [
            ("blue", "#61AFEF"),
            ("cyan", "#56B6C0"),
            ("green", "#98C379"),
            ("yellow", "#E5C07B"),
            ("red", "#E06C75"),
            ("purple", "#C678DD"),
        ],
    },
    Palette {
        name: "base16-default-dark",
        desc: "Rewrites starship.toml with the Base16 Default Dark palette.",
        colors: [
            ("blue", "#3b4ba7"),
            ("cyan", "#33b5e5"),
            ("green", "#008000"),
            ("yellow", "#b58900"),
            ("red", "#dc322f"),
            ("purple", "#6c71c4"),
        ],
    },
    Palette {
        name: "base16-ocean",
        desc: "Rewrites starship.toml with the Base16 Ocean palette.",
        colors: [
            ("blue", "#006181"),
            ("cyan", "#009999"),
            ("green", "#006b3c"),
            ("yellow", "#b58900"),
            ("red", "#cb4b4b"),
            ("purple", "#6c71c4"),
        ],
    },
    Palette {
        name: "adwaita",
        desc: "Rewrites starship.toml with the Adwaita palette.",
        colors: [
            ("blue", "#4C78A8"),
            ("cyan", "#6BA9B6"),
            ("green", "#55A868"),
            ("yellow", "#C0995D"),
            ("red", "#B07040"),
            ("purple", "#845EC2"),
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
    let mut block = format!("[palettes.{}]\n", p.name);
    for (key, hex) in p.colors {
        block.push_str(&format!("{} = '{}'\n", key, hex));
    }
    block.trim_end().to_string()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_block_contains_only_the_palette() {
        let block = palette_block(&PALETTES[1]);
        assert!(block.starts_with("[palettes.dracula]"));
        assert!(!block.contains("[username]"));
        assert_eq!(block.lines().count(), 7);
    }

    #[test]
    fn template_has_one_copy_of_each_section() {
        let text = render(&PALETTES[0]);
        assert_eq!(text.matches("[username]").count(), 1);
        assert_eq!(text.matches("[character]").count(), 1);
        assert_eq!(text.matches("[palettes.nord]").count(), 1);
    }
}
