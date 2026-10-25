mod tasks;
mod themes;

use std::io::{self, Write};
use std::sync::mpsc::Receiver;
use std::time::Duration;

use ratatui::{
    crossterm::cursor::Hide,
    crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Clear, List, ListItem, ListState, Paragraph, Wrap},
    DefaultTerminal, Frame,
};

use tasks::{Action, EventMsg, Task, TABS};

use std::process::Command;

#[derive(PartialEq)]
enum Phase {
    Idle,
    Running,
    Finished,
}

struct App {
    tab: usize,
    sel: usize,
    pending: Option<usize>,
    auth_failed: bool,
    rx: Option<Receiver<EventMsg>>,
    running_name: &'static str,
    output: Vec<String>,
    done: bool,
    child_pid: Option<u32>,
    quitting: bool,
}

impl App {
    fn new() -> Self {
        Self {
            tab: 0,
            sel: 0,
            pending: None,
            auth_failed: false,
            rx: None,
            running_name: "",
            output: Vec::new(),
            done: false,
            child_pid: None,
            quitting: false,
        }
    }

    fn phase(&self) -> Phase {
        if self.rx.is_some() {
            if self.done {
                Phase::Finished
            } else {
                Phase::Running
            }
        } else {
            Phase::Idle
        }
    }

    fn targets(&self, tasks: &[Task]) -> Vec<usize> {
        (0..tasks.len())
            .filter(|&i| tasks[i].tab == self.tab)
            .collect()
    }

    fn next_tab(&mut self) {
        self.tab = (self.tab + 1) % TABS.len();
        self.sel = 0;
    }

    fn prev_tab(&mut self) {
        self.tab = if self.tab == 0 {
            TABS.len() - 1
        } else {
            self.tab - 1
        };
        self.sel = 0;
    }

    fn up(&mut self, len: usize) {
        self.sel = self.sel.saturating_sub(1).min(len.saturating_sub(1));
    }

    fn down(&mut self, len: usize) {
        if len > 0 {
            self.sel = (self.sel + 1).min(len - 1);
        }
    }
}

fn start(app: &mut App, tasks: &[Task], idx: usize) {
    let action = tasks[idx].action.clone();
    app.rx = Some(tasks::spawn(action));
    app.running_name = tasks[idx].name;
    app.output.clear();
    app.done = false;
    app.child_pid = None;
    app.pending = None;
    app.auth_failed = false;
}

fn cancel_task(app: &mut App) {
    if let Some(pid) = app.child_pid.take() {
        let _ = std::process::Command::new("kill")
            .arg("-TERM")
            .arg(pid.to_string())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        app.output.push("Task cancelled.".to_string());
    }
    app.done = true;
}

fn close_output(app: &mut App) {
    app.rx = None;
    app.output.clear();
    app.done = false;
    app.child_pid = None;
}

fn script_needs_sudo(tasks: &[Task], idx: usize) -> bool {
    matches!(&tasks[idx].action, Action::Script(body) if body.contains("$SUDO"))
}

fn is_root() -> bool {
    Command::new("id")
        .arg("-u")
        .output()
        .map(|o| o.stdout.starts_with(b"0\n"))
        .unwrap_or(false)
}

fn sudo_cached_ok() -> bool {
    Command::new("sudo")
        .args(["-n", "true"])
        .stdin(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn authenticate(terminal: &mut DefaultTerminal) -> bool {
    use crossterm::cursor::Show;
    use crossterm::execute;
    use crossterm::terminal::{
        disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
    };
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen, Show);
    let ok = Command::new("sudo")
        .arg("-v")
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    let _ = enable_raw_mode();
    let _ = execute!(io::stdout(), EnterAlternateScreen, Hide);
    let _ = terminal.clear();
    ok
}

pub fn run(terminal: &mut DefaultTerminal, tasks: &[Task]) -> io::Result<()> {
    let mut app = App::new();

    while !app.quitting {
        terminal.draw(|f| draw(f, &app, tasks))?;

        if let Some(rx) = &app.rx {
            loop {
                match rx.try_recv() {
                    Ok(EventMsg::Line(l)) => app.output.push(l),
                    Ok(EventMsg::Done) => app.done = true,
                    Ok(EventMsg::Pid(p)) => app.child_pid = Some(p),
                    Err(_) => break,
                }
            }
        }

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    cancel_task(&mut app);
                    app.quitting = true;
                    break;
                }

                match app.phase() {
                    Phase::Idle => {
                        if app.pending.is_some() {
                            match key.code {
                                KeyCode::Enter | KeyCode::Char('y') => {
                                    if let Some(idx) = app.pending.take() {
                                        if is_root()
                                            || !script_needs_sudo(tasks, idx)
                                            || sudo_cached_ok()
                                        {
                                            start(&mut app, tasks, idx);
                                        } else {
                                            app.auth_failed = !authenticate(terminal);
                                            if !app.auth_failed {
                                                start(&mut app, tasks, idx);
                                            } else {
                                                app.pending = Some(idx);
                                            }
                                        }
                                    }
                                }
                                KeyCode::Esc | KeyCode::Char('n') => {
                                    app.pending = None;
                                    app.auth_failed = false;
                                }
                                _ => {}
                            }
                        } else {
                            let targets = app.targets(tasks);
                            match key.code {
                                KeyCode::Char('q') | KeyCode::Esc => app.quitting = true,
                                KeyCode::Left | KeyCode::Char('h') => app.prev_tab(),
                                KeyCode::Right | KeyCode::Char('l') => app.next_tab(),
                                KeyCode::Up | KeyCode::Char('k') => app.up(targets.len()),
                                KeyCode::Down | KeyCode::Char('j') => app.down(targets.len()),
                                KeyCode::Enter => {
                                    if let Some(&idx) = targets.get(app.sel) {
                                        match tasks[idx].action {
                                            Action::Script(_) => {
                                                app.pending = Some(idx);
                                                app.auth_failed = false;
                                            }
                                            _ => start(&mut app, tasks, idx),
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    Phase::Running => {
                        if let KeyCode::Esc = key.code {
                            cancel_task(&mut app);
                        }
                    }
                    Phase::Finished => match key.code {
                        KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => {
                            close_output(&mut app)
                        }
                        _ => {}
                    },
                }
            }
        }
    }

    if app.rx.is_some() {
        cancel_task(&mut app);
    }
    Ok(())
}

fn header_style(selected: bool) -> Style {
    if selected {
        selected_style()
    } else {
        Style::default().fg(Color::Gray)
    }
}

fn selected_style() -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(Color::Blue)
        .add_modifier(Modifier::BOLD)
}

fn draw(f: &mut Frame, app: &App, tasks: &[Task]) {
    let area = f.area();
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(2),
        ])
        .split(area);

    draw_header(f, app, outer[0]);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(22), Constraint::Min(0)])
        .split(outer[1]);
    draw_categories(f, app, body[0]);

    match app.phase() {
        Phase::Idle => draw_task_list(f, app, tasks, body[1]),
        _ => draw_output(f, app, body[1]),
    }

    draw_status(f, app, tasks, outer[2]);

    if let Some(idx) = app.pending {
        draw_confirm(f, app, tasks, idx, area);
    }
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let mut spans: Vec<Span> = Vec::new();
    for (i, name) in TABS.iter().enumerate() {
        spans.push(Span::styled(
            format!(" {} ", name),
            header_style(i == app.tab),
        ));
        spans.push(Span::raw(" "));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_categories(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = TABS.iter().map(|t| ListItem::new(t.to_string())).collect();
    let list = List::new(items).block(Block::bordered().title("Sections"));
    let mut state = ListState::default();
    state.select(Some(app.tab));
    f.render_stateful_widget(list, area, &mut state);
}

fn draw_task_list(f: &mut Frame, app: &App, tasks: &[Task], area: Rect) {
    let targets = app.targets(tasks);
    let items: Vec<ListItem> = targets
        .iter()
        .map(|&i| ListItem::new(format!(" {}", tasks[i].name)))
        .collect();
    let list = List::new(items)
        .block(Block::bordered().title(TABS[app.tab]))
        .highlight_style(selected_style());
    let mut state = ListState::default();
    state.select(Some(app.sel));
    f.render_stateful_widget(list, area, &mut state);
}

fn draw_output(f: &mut Frame, app: &App, area: Rect) {
    let title = if app.done {
        format!("Done: {} (Esc to close)", app.running_name)
    } else {
        format!("Running: {} (Esc to cancel)", app.running_name)
    };
    let lines: Vec<Line> = app.output.iter().map(|s| Line::from(s.clone())).collect();
    let mut para = Paragraph::new(Text::from(lines))
        .block(Block::bordered().title(title))
        .wrap(Wrap { trim: false });
    let inner_h = area.height.saturating_sub(2) as usize;
    let off = app.output.len().saturating_sub(inner_h);
    if off > 0 {
        para = para.scroll((off.min(u16::MAX as usize) as u16, 0));
    }
    f.render_widget(para, area);
}

fn draw_status(f: &mut Frame, app: &App, tasks: &[Task], area: Rect) {
    let targets = app.targets(tasks);
    let desc = if app.pending.is_some() {
        "Run this task? Enter to confirm, Esc to cancel."
    } else if app.phase() == Phase::Idle {
        targets
            .get(app.sel)
            .map(|&i| tasks[i].desc)
            .unwrap_or_default()
    } else {
        ""
    };
    let hint = if app.phase() == Phase::Idle {
        "arrows navigate · h/l switch tabs · Enter run · q quit · run `sudo -v` first to skip prompts"
    } else {
        "Esc cancel/close"
    };
    let text = vec![
        Line::from(desc.to_string()).style(Style::default().fg(Color::Cyan)),
        Line::from(hint.to_string()).style(Style::default().fg(Color::DarkGray)),
    ];
    f.render_widget(
        Paragraph::new(Text::from(text)).wrap(Wrap { trim: true }),
        area,
    );
}

fn draw_confirm(f: &mut Frame, app: &App, tasks: &[Task], idx: usize, area: Rect) {
    let task = &tasks[idx];
    let block = Block::bordered().title(" Confirm ").title_style(
        Style::default()
            .bg(Color::Red)
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    );
    let rect = centered_rect(area, 54, 8);
    f.render_widget(Clear, rect);
    let mut lines = vec![
        Line::from(task.desc.to_string()).style(Style::default().fg(Color::Cyan)),
        Line::default(),
    ];
    if app.auth_failed {
        lines.push(
            Line::from("sudo authentication failed - press Enter to try again")
                .style(Style::default().fg(Color::Red)),
        );
        lines.push(Line::default());
    }
    lines.push(Line::from(vec![
        Span::styled(" [Enter]", Style::default().fg(Color::Green)),
        Span::raw(" run  "),
        Span::styled("[Esc]", Style::default().fg(Color::Red)),
        Span::raw(" cancel"),
    ]));
    f.render_widget(
        Paragraph::new(Text::from(lines))
            .block(block)
            .wrap(Wrap { trim: true }),
        rect,
    );
}

fn centered_rect(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    }
}

fn print_list(tasks: &[Task]) {
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    for (tab, name) in TABS.iter().enumerate() {
        let _ = writeln!(out, "== {} ==", name);
        for t in tasks.iter().filter(|t| t.tab == tab) {
            let _ = writeln!(out, "  {}\n      {}", t.name, t.desc);
        }
    }
}

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && args[1] == "--list" {
        print_list(&tasks::build());
        return Ok(());
    }
    let tasks = tasks::build();
    let mut terminal = ratatui::init();
    let result = run(&mut terminal, &tasks);
    ratatui::restore();
    result
}
