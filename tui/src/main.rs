mod state;
mod tasks;
mod themes;

use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use crossterm::cursor::{Hide, Show};
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    MouseButton, MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Clear, List, ListItem, ListState, Paragraph, Wrap},
    DefaultTerminal, Frame,
};

use state::State;
use tasks::{Action, EventMsg, Status, StatusKind, Task, TABS};
use themes::PALETTES;

#[derive(PartialEq)]
enum Phase {
    Idle,
    Running,
    Finished,
}

struct InputState {
    idx: usize,
    label: String,
    buffer: String,
}

#[derive(Clone)]
struct ScreenLayout {
    tabs: Vec<Rect>,
    sections: Rect,
    tasks: Rect,
    details: Option<Rect>,
    status: Rect,
    task_offset: usize,
}

struct App {
    tab: usize,
    sel: usize,
    pending: Option<usize>,
    pending_command: Option<String>,
    auth_failed: bool,
    rx: Option<Receiver<EventMsg>>,
    running_name: String,
    output: Vec<String>,
    done: bool,
    failed: bool,
    child_pid: Option<u32>,
    quitting: bool,
    statuses: Vec<Option<Status>>,
    filter: String,
    searching: bool,
    output_query: String,
    output_searching: bool,
    output_scroll: Option<usize>,
    input: Option<InputState>,
    state: State,
    last_click: Option<(Instant, usize)>,
    toast: Option<(String, Instant)>,
    screen: Option<ScreenLayout>,
}

impl App {
    fn new(tasks: &[Task], state: State) -> Self {
        Self {
            tab: 0,
            sel: 0,
            pending: None,
            pending_command: None,
            auth_failed: false,
            rx: None,
            running_name: String::new(),
            output: Vec::new(),
            done: false,
            failed: false,
            child_pid: None,
            quitting: false,
            statuses: tasks.iter().map(|t| t.status()).collect(),
            filter: String::new(),
            searching: false,
            output_query: String::new(),
            output_searching: false,
            output_scroll: None,
            input: None,
            state,
            last_click: None,
            toast: None,
            screen: None,
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
        let needle = self.filter.trim().to_lowercase();
        let mut targets: Vec<usize> = tasks
            .iter()
            .enumerate()
            .filter(|(_, task)| {
                task.tab == self.tab
                    && (needle.is_empty()
                        || task.name.to_lowercase().contains(&needle)
                        || task.desc.to_lowercase().contains(&needle))
            })
            .map(|(i, _)| i)
            .collect();
        targets.sort_by_key(|&i| !self.state.is_favorite(&tasks[i].name));
        targets
    }

    fn set_tab(&mut self, tab: usize) {
        if tab < TABS.len() {
            self.tab = tab;
            self.sel = 0;
            self.filter.clear();
            self.searching = false;
        }
    }

    fn next_tab(&mut self) {
        self.set_tab((self.tab + 1) % TABS.len());
    }

    fn prev_tab(&mut self) {
        let tab = if self.tab == 0 {
            TABS.len() - 1
        } else {
            self.tab - 1
        };
        self.set_tab(tab);
    }

    fn up(&mut self, len: usize) {
        self.sel = self.sel.saturating_sub(1).min(len.saturating_sub(1));
    }

    fn down(&mut self, len: usize) {
        if len > 0 {
            self.sel = (self.sel + 1).min(len - 1);
        }
    }

    fn refresh_statuses(&mut self, tasks: &[Task]) {
        self.statuses = tasks.iter().map(|t| t.status()).collect();
    }

    fn persist(&self) {
        state::save(&self.state);
    }

    fn set_toast(&mut self, message: impl Into<String>) {
        self.toast = Some((message.into(), Instant::now()));
    }

    fn selected_index(&self, tasks: &[Task]) -> Option<usize> {
        self.targets(tasks).get(self.sel).copied()
    }
}

fn start_with(app: &mut App, tasks: &[Task], idx: usize, command: Option<String>) {
    let action = match command {
        Some(command) => Action::Script(command),
        None => tasks[idx].action.clone(),
    };
    app.rx = Some(tasks::spawn(
        tasks[idx].name.clone(),
        action,
        tasks[idx].timeout,
    ));
    app.running_name = tasks[idx].name.clone();
    app.output.clear();
    app.done = false;
    app.failed = false;
    app.child_pid = None;
    app.pending = None;
    app.pending_command = None;
    app.auth_failed = false;
    app.output_query.clear();
    app.output_searching = false;
    app.output_scroll = None;
    app.state.record_recent(&tasks[idx].name);
    app.persist();
}

fn start(app: &mut App, tasks: &[Task], idx: usize) {
    start_with(app, tasks, idx, None);
}

fn request_task(app: &mut App, terminal: &mut DefaultTerminal, tasks: &[Task], idx: usize) {
    if let Action::Edit(path) = &tasks[idx].action {
        start_editor(app, terminal, tasks, idx, path);
        return;
    }
    if let Some(prompt) = &tasks[idx].prompt {
        app.input = Some(InputState {
            idx,
            label: prompt.clone(),
            buffer: String::new(),
        });
        app.auth_failed = false;
        return;
    }
    if tasks[idx].destructive || script_needs_sudo(tasks, idx) {
        app.pending = Some(idx);
        app.pending_command = None;
        app.auth_failed = false;
    } else {
        start(app, tasks, idx);
    }
}

fn confirm_pending(app: &mut App, terminal: &mut DefaultTerminal, tasks: &[Task], idx: usize) {
    let command = app.pending_command.clone();
    if is_root() || !script_needs_sudo(tasks, idx) || sudo_cached_ok() {
        start_with(app, tasks, idx, command);
    } else {
        app.auth_failed = !authenticate(terminal);
        if !app.auth_failed {
            start_with(app, tasks, idx, command);
        } else {
            app.pending = Some(idx);
        }
    }
}

fn submit_input(app: &mut App, tasks: &[Task]) {
    let Some(input) = app.input.take() else {
        return;
    };
    if let Some(command) = tasks[input.idx].command_with_arg(&input.buffer) {
        if tasks[input.idx].destructive || script_needs_sudo(tasks, input.idx) {
            app.pending = Some(input.idx);
            app.pending_command = Some(command);
            app.auth_failed = false;
        } else {
            start_with(app, tasks, input.idx, Some(command));
        }
    }
}

fn start_editor(
    app: &mut App,
    terminal: &mut DefaultTerminal,
    tasks: &[Task],
    idx: usize,
    path: &Path,
) {
    let editor = editor_name();
    let _ = disable_raw_mode();
    let _ = execute!(
        io::stdout(),
        DisableMouseCapture,
        LeaveAlternateScreen,
        Show
    );
    let status = open_editor(&editor, path).or_else(|_| open_editor("vi", path));
    let _ = enable_raw_mode();
    let _ = execute!(io::stdout(), EnterAlternateScreen, Hide, EnableMouseCapture);
    let _ = terminal.clear();

    app.running_name = tasks[idx].name.clone();
    app.output = vec![
        format!("Editor: {}", editor),
        format!("File: {}", path.display()),
        match &status {
            Ok(s) if s.success() => "Editor closed.".to_string(),
            Ok(s) => format!("Editor exited with status {}.", s),
            Err(e) => format!("Could not start the editor: {}", e),
        },
    ];
    app.done = true;
    app.failed = !status.is_ok_and(|s| s.success());
    app.child_pid = None;
    app.pending = None;
    app.pending_command = None;
    let (_tx, rx) = mpsc::channel();
    app.rx = Some(rx);
    app.state.record_recent(&tasks[idx].name);
    app.persist();
}

fn editor_name() -> String {
    std::env::var("EDITOR")
        .ok()
        .filter(|e| !e.trim().is_empty())
        .or_else(|| {
            std::env::var("VISUAL")
                .ok()
                .filter(|e| !e.trim().is_empty())
        })
        .unwrap_or_else(|| "nano".to_string())
}

fn open_editor(editor: &str, path: &Path) -> io::Result<std::process::ExitStatus> {
    Command::new(editor).arg(path).status()
}

fn cancel_task(app: &mut App) {
    if let Some(pid) = app.child_pid.take() {
        let _ = Command::new("kill")
            .arg("-TERM")
            .arg(pid.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        app.output.push("Task cancelled.".to_string());
    }
    app.done = true;
}

fn close_output(app: &mut App) {
    app.rx = None;
    app.output.clear();
    app.done = false;
    app.failed = false;
    app.child_pid = None;
    app.output_query.clear();
    app.output_searching = false;
    app.output_scroll = None;
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
        .stdin(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn authenticate(terminal: &mut DefaultTerminal) -> bool {
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
    let mut app = App::new(tasks, state::load());

    while !app.quitting {
        terminal.draw(|f| draw(f, &mut app, tasks))?;

        let mut statuses_dirty = false;
        if let Some(rx) = &app.rx {
            loop {
                match rx.try_recv() {
                    Ok(EventMsg::Line(line)) => app.output.push(line),
                    Ok(EventMsg::Done { failed, log }) => {
                        app.done = true;
                        app.failed = failed;
                        if let Some(path) = log {
                            app.output.push(format!("Failure log: {}", path.display()));
                        }
                        statuses_dirty = true;
                    }
                    Ok(EventMsg::Pid(pid)) => app.child_pid = Some(pid),
                    Err(_) => break,
                }
            }
        }
        if statuses_dirty {
            app.refresh_statuses(tasks);
        }

        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    if key.modifiers.contains(KeyModifiers::CONTROL)
                        && key.code == KeyCode::Char('c')
                    {
                        cancel_task(&mut app);
                        app.quitting = true;
                        break;
                    }

                    if app.input.is_some() {
                        match key.code {
                            KeyCode::Enter => submit_input(&mut app, tasks),
                            KeyCode::Esc => app.input = None,
                            KeyCode::Backspace => {
                                if let Some(input) = &mut app.input {
                                    input.buffer.pop();
                                }
                            }
                            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                                if let Some(input) = &mut app.input {
                                    input.buffer.push(c);
                                }
                            }
                            _ => {}
                        }
                        continue;
                    }

                    if app.phase() == Phase::Finished && app.output_searching {
                        match key.code {
                            KeyCode::Enter => app.output_searching = false,
                            KeyCode::Esc => {
                                app.output_query.clear();
                                app.output_searching = false;
                                app.output_scroll = None;
                            }
                            KeyCode::Backspace => {
                                app.output_query.pop();
                                app.output_scroll = None;
                            }
                            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                app.output_query.clear();
                                app.output_scroll = None;
                            }
                            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                                app.output_query.push(c);
                                app.output_scroll = None;
                            }
                            _ => {}
                        }
                        continue;
                    }

                    if app.phase() == Phase::Idle && app.pending.is_none() && app.searching {
                        match key.code {
                            KeyCode::Enter => app.searching = false,
                            KeyCode::Esc => {
                                app.filter.clear();
                                app.searching = false;
                                app.sel = 0;
                            }
                            KeyCode::Backspace => {
                                app.filter.pop();
                                app.sel = 0;
                            }
                            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                app.filter.clear();
                                app.sel = 0;
                            }
                            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                                app.filter.push(c);
                                app.sel = 0;
                            }
                            _ => {}
                        }
                        continue;
                    }

                    match app.phase() {
                        Phase::Idle => {
                            if app.pending.is_some() {
                                match key.code {
                                    KeyCode::Enter | KeyCode::Char('y') => {
                                        if let Some(idx) = app.pending.take() {
                                            confirm_pending(&mut app, terminal, tasks, idx);
                                        }
                                    }
                                    KeyCode::Esc | KeyCode::Char('n') => {
                                        app.pending = None;
                                        app.pending_command = None;
                                        app.auth_failed = false;
                                    }
                                    _ => {}
                                }
                            } else {
                                let targets = app.targets(tasks);
                                match key.code {
                                    KeyCode::Char('q') => app.quitting = true,
                                    KeyCode::Esc => {
                                        if app.filter.is_empty() {
                                            app.quitting = true;
                                        } else {
                                            app.filter.clear();
                                            app.sel = 0;
                                        }
                                    }
                                    KeyCode::Left | KeyCode::Char('h') => app.prev_tab(),
                                    KeyCode::Right | KeyCode::Char('l') => app.next_tab(),
                                    KeyCode::Up | KeyCode::Char('k') => app.up(targets.len()),
                                    KeyCode::Down | KeyCode::Char('j') => app.down(targets.len()),
                                    KeyCode::Char('/') => app.searching = true,
                                    KeyCode::Char('p') => {
                                        if let Some(&idx) = targets.get(app.sel) {
                                            let name = tasks[idx].name.clone();
                                            let pinned = app.state.toggle_favorite(&name);
                                            app.persist();
                                            let new_targets = app.targets(tasks);
                                            app.sel = new_targets
                                                .iter()
                                                .position(|&i| tasks[i].name == name)
                                                .unwrap_or(0);
                                            app.set_toast(if pinned {
                                                format!("Pinned {}", name)
                                            } else {
                                                format!("Unpinned {}", name)
                                            });
                                        }
                                    }
                                    KeyCode::Char('[') => {
                                        app.state.sections_width =
                                            app.state.sections_width.saturating_sub(2).max(14);
                                        app.persist();
                                    }
                                    KeyCode::Char(']') => {
                                        app.state.sections_width =
                                            app.state.sections_width.saturating_add(2).min(40);
                                        app.persist();
                                    }
                                    KeyCode::Char(c @ '1'..='9') => {
                                        let tab = c as usize - '1' as usize;
                                        app.set_tab(tab);
                                    }
                                    KeyCode::Enter => {
                                        if let Some(&idx) = targets.get(app.sel) {
                                            request_task(&mut app, terminal, tasks, idx);
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
                            KeyCode::Char('/') => {
                                app.output_searching = true;
                                app.output_scroll = None;
                            }
                            KeyCode::Char('y') => copy_output(&mut app),
                            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => {
                                close_output(&mut app)
                            }
                            KeyCode::PageUp | KeyCode::Up => output_scroll_by(&mut app, -10),
                            KeyCode::PageDown | KeyCode::Down => output_scroll_by(&mut app, 10),
                            KeyCode::Home => app.output_scroll = Some(0),
                            KeyCode::End => app.output_scroll = None,
                            _ => {}
                        },
                    }
                }
                Event::Mouse(mouse) => {
                    handle_mouse(
                        &mut app,
                        terminal,
                        tasks,
                        mouse.kind,
                        mouse.column,
                        mouse.row,
                    );
                }
                _ => {}
            }
        }
    }

    if app.rx.is_some() {
        cancel_task(&mut app);
    }
    Ok(())
}

fn handle_mouse(
    app: &mut App,
    terminal: &mut DefaultTerminal,
    tasks: &[Task],
    kind: MouseEventKind,
    column: u16,
    row: u16,
) {
    if app.input.is_some() || app.pending.is_some() {
        return;
    }
    if app.phase() == Phase::Finished {
        match kind {
            MouseEventKind::ScrollDown => output_scroll_by(app, 3),
            MouseEventKind::ScrollUp => output_scroll_by(app, -3),
            _ => {}
        }
        return;
    }
    if app.phase() != Phase::Idle {
        return;
    }
    let (tabs, sections, task_rect, offset) = match &app.screen {
        Some(layout) => (
            layout.tabs.clone(),
            layout.sections,
            layout.tasks,
            layout.task_offset,
        ),
        None => return,
    };
    let targets = app.targets(tasks);
    match kind {
        MouseEventKind::Down(MouseButton::Left) => {
            if let Some(tab) = tabs.iter().position(|r| rect_contains(*r, column, row)) {
                app.set_tab(tab);
                return;
            }
            let section_inner = inner(sections);
            if rect_contains(section_inner, column, row) {
                let idx = (row - section_inner.y) as usize;
                if idx < TABS.len() {
                    app.set_tab(idx);
                }
                return;
            }
            let task_inner = inner(task_rect);
            if rect_contains(task_inner, column, row) {
                app.searching = false;
                let pos = (row - task_inner.y) as usize + offset;
                if pos < targets.len() {
                    let now = Instant::now();
                    let double_click = app.last_click.take().is_some_and(|(at, last)| {
                        last == pos && now.duration_since(at) < Duration::from_millis(400)
                    });
                    if double_click {
                        let idx = targets[pos];
                        request_task(app, terminal, tasks, idx);
                    } else {
                        app.sel = pos;
                        app.last_click = Some((now, pos));
                    }
                }
            }
        }
        MouseEventKind::ScrollDown if rect_contains(inner(task_rect), column, row) => {
            for _ in 0..3 {
                app.down(targets.len());
            }
        }
        MouseEventKind::ScrollUp if rect_contains(inner(task_rect), column, row) => {
            for _ in 0..3 {
                app.up(targets.len());
            }
        }
        _ => {}
    }
}

fn output_scroll_by(app: &mut App, delta: isize) {
    let height = app
        .screen
        .as_ref()
        .map(|s| s.tasks.height.saturating_sub(2) as usize)
        .unwrap_or(10);
    let total = output_lines(app).len();
    let max = total.saturating_sub(height);
    let current = app.output_scroll.unwrap_or(max);
    let next = if delta.is_negative() {
        current.saturating_sub(delta.unsigned_abs())
    } else {
        current.saturating_add(delta as usize)
    };
    app.output_scroll = if next >= max { None } else { Some(next) };
}

fn output_lines(app: &App) -> Vec<&String> {
    let needle = app.output_query.trim().to_lowercase();
    if needle.is_empty() {
        app.output.iter().collect()
    } else {
        app.output
            .iter()
            .filter(|line| line.to_lowercase().contains(&needle))
            .collect()
    }
}

fn copy_output(app: &mut App) {
    if app.output.is_empty() {
        app.set_toast("Nothing to copy.");
        return;
    }
    let text = app.output.join("\n");
    if copy_to_clipboard(&text) {
        app.set_toast("Output copied to the clipboard.");
    } else {
        app.set_toast("Could not copy output.");
    }
}

fn copy_to_clipboard(text: &str) -> bool {
    let candidates: [(&str, &[&str]); 4] = [
        ("wl-copy", &[]),
        ("xclip", &["-selection", "clipboard"]),
        ("xsel", &["--clipboard", "--input"]),
        ("pbcopy", &[]),
    ];
    for (command, args) in candidates {
        let child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        let Ok(mut child) = child else {
            continue;
        };
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
        if child.wait().is_ok_and(|s| s.success()) {
            return true;
        }
    }
    osc52_copy(text)
}

fn osc52_copy(text: &str) -> bool {
    let encoded = base64(text.as_bytes());
    let mut out = io::stdout();
    write!(out, "\x1b]52;c;{}\x07", encoded).is_ok() && out.flush().is_ok()
}

fn base64(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

fn rect_contains(rect: Rect, column: u16, row: u16) -> bool {
    column >= rect.x
        && column < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

fn inner(rect: Rect) -> Rect {
    Rect {
        x: rect.x.saturating_add(1),
        y: rect.y.saturating_add(1),
        width: rect.width.saturating_sub(2),
        height: rect.height.saturating_sub(2),
    }
}

fn visible_offset(selected: usize, len: usize, height: usize) -> usize {
    if height == 0 || len <= height {
        0
    } else {
        selected.saturating_sub(height - 1).min(len - height)
    }
}

fn compute_layout(area: Rect, sections_width: u16) -> ScreenLayout {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(2),
        ])
        .split(area);
    let detail_width = if area.width >= 100 {
        40
    } else if area.width >= 80 {
        32
    } else {
        0
    };
    let mut constraints = vec![Constraint::Length(sections_width), Constraint::Min(0)];
    if detail_width > 0 {
        constraints.push(Constraint::Length(detail_width));
    }
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(outer[1]);
    let details = if detail_width > 0 {
        body.get(2).copied()
    } else {
        None
    };
    let mut tabs = Vec::new();
    let mut x = area.x;
    for name in TABS {
        let width = name.chars().count() as u16 + 2;
        let clipped = width.min(area.right().saturating_sub(x));
        tabs.push(Rect {
            x,
            y: area.y,
            width: clipped,
            height: 1,
        });
        x = x.saturating_add(width).saturating_add(1);
    }
    ScreenLayout {
        tabs,
        sections: body[0],
        tasks: body[1],
        details,
        status: outer[2],
        task_offset: 0,
    }
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

fn draw(f: &mut Frame, app: &mut App, tasks: &[Task]) {
    let area = f.area();
    let mut layout = compute_layout(area, app.state.sections_width);

    draw_header(f, app, area);
    draw_categories(f, app, layout.sections);

    match app.phase() {
        Phase::Idle => {
            let targets = app.targets(tasks);
            let inner_h = layout.tasks.height.saturating_sub(2) as usize;
            let offset = visible_offset(app.sel, targets.len(), inner_h);
            layout.task_offset = offset;
            draw_task_list(f, app, tasks, layout.tasks, offset);
            if let Some(details) = layout.details {
                draw_details(f, app, tasks, details);
            }
        }
        _ => draw_output(f, app, layout.tasks),
    }

    draw_status(f, app, tasks, layout.status);

    if app.input.is_some() {
        draw_input(f, app, area);
    }
    if let Some(idx) = app.pending {
        draw_confirm(f, app, tasks, idx, area);
    }

    app.screen = Some(layout);
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
    f.render_widget(
        Paragraph::new(Line::from(spans)),
        Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1,
        },
    );
}

fn draw_categories(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = TABS.iter().map(|t| ListItem::new(t.to_string())).collect();
    let list = List::new(items).block(Block::bordered().title("Sections"));
    let mut state = ListState::default();
    state.select(Some(app.tab));
    f.render_stateful_widget(list, area, &mut state);
}

fn task_item(app: &App, tasks: &[Task], idx: usize) -> ListItem<'static> {
    let mut spans = vec![Span::raw(format!(" {}", tasks[idx].name))];
    if app.state.is_favorite(&tasks[idx].name) {
        spans.push(Span::styled(
            "  [pinned]",
            Style::default().fg(Color::Magenta),
        ));
    }
    if let Some(status) = app.statuses.get(idx).and_then(|s| s.as_ref()) {
        let color = match status.kind {
            StatusKind::Installed => Color::Green,
            StatusKind::Missing => Color::DarkGray,
            StatusKind::Partial => Color::Yellow,
        };
        spans.push(Span::styled(
            format!("  [{}]", status.label),
            Style::default().fg(color),
        ));
    }
    ListItem::new(Line::from(spans))
}

fn draw_task_list(f: &mut Frame, app: &App, tasks: &[Task], area: Rect, offset: usize) {
    let targets = app.targets(tasks);
    let title = if app.filter.is_empty() {
        TABS[app.tab].to_string()
    } else {
        format!("{}  /{}", TABS[app.tab], app.filter)
    };
    if targets.is_empty() {
        let empty = Paragraph::new(" No matching tasks. Press Esc to clear the filter.")
            .block(Block::bordered().title(title))
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(empty, area);
        return;
    }

    let inner_h = area.height.saturating_sub(2) as usize;
    let items: Vec<ListItem> = targets
        .iter()
        .skip(offset)
        .take(inner_h)
        .map(|&idx| task_item(app, tasks, idx))
        .collect();
    let list = List::new(items)
        .block(Block::bordered().title(title))
        .highlight_style(selected_style());
    let visible_sel = app
        .sel
        .saturating_sub(offset)
        .min(list.len().saturating_sub(1));
    let mut state = ListState::default().with_selected(Some(visible_sel));
    f.render_stateful_widget(list, area, &mut state);
}

fn draw_details(f: &mut Frame, app: &App, tasks: &[Task], area: Rect) {
    let Some(idx) = app.selected_index(tasks) else {
        let empty = Paragraph::new(" Select a task to see details.")
            .block(Block::bordered().title("Details"))
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(empty, area);
        return;
    };
    let task = &tasks[idx];
    let mut lines = vec![
        Line::from(task.name.clone()).style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Line::from(format!(
            "{} · {}",
            TABS[task.tab],
            if task.custom { "custom" } else { "built-in" }
        ))
        .style(Style::default().fg(Color::DarkGray)),
    ];
    if app.state.is_favorite(&task.name) {
        lines.push(Line::from("Pinned").style(Style::default().fg(Color::Magenta)));
    }
    if let Some(status) = app.statuses.get(idx).and_then(|s| s.as_ref()) {
        let color = match status.kind {
            StatusKind::Installed => Color::Green,
            StatusKind::Missing => Color::DarkGray,
            StatusKind::Partial => Color::Yellow,
        };
        lines.push(Line::from(status.label.clone()).style(Style::default().fg(color)));
    }
    if task.destructive {
        lines.push(
            Line::from("Destructive: makes system changes").style(Style::default().fg(Color::Red)),
        );
    }
    if let Some(timeout) = task.timeout {
        lines.push(Line::from(format!("Timeout: {}s", timeout)));
    }
    if let Some(prompt) = &task.prompt {
        lines.push(Line::from(format!("Prompt: {}", prompt)));
    }
    lines.push(Line::default());
    lines.push(Line::from(task.desc.clone()));
    lines.push(Line::default());
    match &task.action {
        Action::Theme(idx) => {
            if let Some(palette) = PALETTES.get(*idx) {
                lines.push(Line::from("Palette preview:").style(Style::default().fg(Color::Gray)));
                for (name, hex) in palette.colors {
                    lines.push(Line::from(vec![
                        Span::styled("    ", Style::default().bg(hex_color(hex))),
                        Span::raw(format!(" {} {}", name, hex)),
                    ]));
                }
            }
        }
        Action::Edit(path) => {
            lines.push(Line::from(format!("File: {}", path.display())));
        }
        _ => {
            lines.push(Line::from("Command:").style(Style::default().fg(Color::Gray)));
            lines.push(Line::from(task.preview()));
        }
    }
    let paragraph = Paragraph::new(Text::from(lines))
        .block(Block::bordered().title("Details"))
        .wrap(Wrap { trim: true });
    f.render_widget(paragraph, area);
}

fn hex_color(hex: &str) -> Color {
    let hex = hex.trim_start_matches('#');
    if hex.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&hex[0..2], 16),
            u8::from_str_radix(&hex[2..4], 16),
            u8::from_str_radix(&hex[4..6], 16),
        ) {
            return Color::Rgb(r, g, b);
        }
    }
    Color::Gray
}

fn draw_output(f: &mut Frame, app: &App, area: Rect) {
    let title = if app.done {
        if app.failed {
            format!("Done (failed): {} (Esc to close)", app.running_name)
        } else {
            format!("Done: {} (Esc to close)", app.running_name)
        }
    } else {
        format!("Running: {} (Esc to cancel)", app.running_name)
    };
    let title = if app.output_query.is_empty() {
        title
    } else {
        format!("{}  /{}", title, app.output_query)
    };
    let lines: Vec<&String> = output_lines(app);
    let inner_h = area.height.saturating_sub(2) as usize;
    let max = lines.len().saturating_sub(inner_h);
    let off = app.output_scroll.unwrap_or(max).min(max);
    let visible: Vec<Line> = lines
        .iter()
        .skip(off)
        .take(inner_h)
        .map(|s| Line::from((*s).clone()))
        .collect();
    let para = Paragraph::new(Text::from(visible))
        .block(Block::bordered().title(title))
        .wrap(Wrap { trim: false });
    f.render_widget(para, area);
}

fn draw_status(f: &mut Frame, app: &App, tasks: &[Task], area: Rect) {
    let now = Instant::now();
    if let Some((message, at)) = &app.toast {
        if now.duration_since(*at) < Duration::from_secs(2) {
            f.render_widget(
                Paragraph::new(
                    Line::from(message.clone()).style(Style::default().fg(Color::Green)),
                )
                .wrap(Wrap { trim: true }),
                area,
            );
            return;
        }
    }

    let targets = app.targets(tasks);
    let (desc, hint) = if let Some(input) = &app.input {
        (
            format!("{}: {}", input.label, input.buffer),
            "Enter run · Esc cancel".to_string(),
        )
    } else if app.searching {
        (
            format!("Search: {}", app.filter),
            "type to filter · Enter keep · Esc clear".to_string(),
        )
    } else if app.phase() == Phase::Finished {
        if app.output_searching {
            (
                format!("Search output: {}", app.output_query),
                "type to filter · Enter keep · Esc clear".to_string(),
            )
        } else {
            (
                format!(
                    "{} lines{}",
                    app.output.len(),
                    if app.output_query.is_empty() {
                        String::new()
                    } else {
                        format!(" · filter: {}", app.output_query)
                    }
                ),
                "y copy · / search · arrows scroll · Esc close".to_string(),
            )
        }
    } else if app.pending.is_some() {
        (
            "Run this task? Enter to confirm, Esc to cancel.".to_string(),
            String::new(),
        )
    } else if app.phase() == Phase::Idle {
        (
            targets
                .get(app.sel)
                .map(|&i| tasks[i].desc.clone())
                .unwrap_or_default(),
            "1-5/h/l sections · / search · p pin · [/] width · Enter run · q quit".to_string(),
        )
    } else {
        (String::new(), "Esc cancel/close".to_string())
    };
    let text = vec![
        Line::from(desc).style(Style::default().fg(Color::Cyan)),
        Line::from(hint).style(Style::default().fg(Color::DarkGray)),
    ];
    f.render_widget(
        Paragraph::new(Text::from(text)).wrap(Wrap { trim: true }),
        area,
    );
}

fn draw_input(f: &mut Frame, app: &App, area: Rect) {
    let Some(input) = &app.input else {
        return;
    };
    let rect = centered_rect(area, 64, 8);
    f.render_widget(Clear, rect);
    let lines = vec![
        Line::from(input.label.clone()).style(Style::default().fg(Color::Cyan)),
        Line::default(),
        Line::from(format!("> {}_", input.buffer)),
        Line::default(),
        Line::from(vec![
            Span::styled(" [Enter]", Style::default().fg(Color::Green)),
            Span::raw(" run  "),
            Span::styled("[Esc]", Style::default().fg(Color::Red)),
            Span::raw(" cancel"),
        ]),
    ];
    f.render_widget(
        Paragraph::new(Text::from(lines))
            .block(Block::bordered().title(" Input "))
            .wrap(Wrap { trim: true }),
        rect,
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
    let rect = centered_rect(area, 54, 9);
    f.render_widget(Clear, rect);
    let mut lines = vec![
        Line::from(task.desc.clone()).style(Style::default().fg(Color::Cyan)),
        Line::default(),
    ];
    if task.destructive {
        lines.push(
            Line::from("This task makes system changes.").style(Style::default().fg(Color::Red)),
        );
        lines.push(Line::default());
    }
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

fn print_list(tasks: &[Task], json: bool) {
    if json {
        let items: Vec<serde_json::Value> = tasks
            .iter()
            .enumerate()
            .map(|(index, task)| {
                serde_json::json!({
                    "index": index + 1,
                    "name": task.name,
                    "description": task.desc,
                    "section": TABS[task.tab],
                    "tab": task.tab,
                    "status": task.status().map(|s| s.label),
                    "custom": task.custom,
                    "destructive": task.destructive,
                    "prompt": task.prompt,
                    "timeout": task.timeout,
                    "command": task.command(),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&items).unwrap());
        return;
    }
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    for (tab, name) in TABS.iter().enumerate() {
        let _ = writeln!(out, "== {} ==", name);
        for (index, t) in tasks.iter().enumerate().filter(|(_, t)| t.tab == tab) {
            let status = t
                .status()
                .map(|s| format!(" [{}]", s.label))
                .unwrap_or_default();
            let pin = if t.custom { " (custom)" } else { "" };
            let _ = writeln!(
                out,
                "  {:>2}. {}{}{}\n      {}",
                index + 1,
                t.name,
                status,
                pin,
                t.desc
            );
        }
    }
}

fn print_help() {
    println!("BashBox toolbox {}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("Usage: bashbox [OPTION]");
    println!();
    println!("Options:");
    println!("  --list                 List every task and exit");
    println!("  --json                 With --list, emit JSON");
    println!("  --run <name|number>    Run one task and exit");
    println!("  --arg <value>          Value for a task's prompt placeholder");
    println!("  --dry-run              With --run, print what would run");
    println!("  --yes                  Accepted for scripting; CLI runs are explicit");
    println!("  --recent               Show recently run tasks");
    println!("  --logs                 Show the last failed task log");
    println!("  --doctor               Check dependencies, links, and fonts, then exit");
    println!("  --fetch-tasks <url>    Download and validate a task TOML into tasks.d");
    println!("  -V, --version          Print the version and exit");
    println!("  -h, --help             Show this help");
    println!();
    println!("Keys:");
    println!("  1-5          Jump to a section");
    println!("  h/l, arrows  Switch sections");
    println!("  j/k, arrows  Move between tasks");
    println!("  /            Search tasks (Enter keeps, Esc clears)");
    println!("  p            Pin/unpin the selected task");
    println!("  [ / ]        Shrink/grow the sections pane");
    println!("  Enter        Run the selected task");
    println!("  Esc          Cancel/close, or clear a filter");
    println!("  y            Copy run output (clipboard/OSC52)");
    println!("  q            Quit");
    println!("  Mouse        Click a section/task, double-click to run, wheel to scroll");
    println!();
    println!("Custom tasks: ~/.config/bashbox/tasks.toml");
    println!("              plus any ~/.config/bashbox/tasks.d/*.toml");
    println!("              (override with BASHBOX_TASKS)");
    println!("State and failed-task logs live in ~/.local/state/bashbox/");
}

#[derive(Default)]
struct Options {
    list: bool,
    json: bool,
    run: Option<String>,
    arg: Option<String>,
    dry_run: bool,
    recent: bool,
    logs: bool,
    doctor: bool,
    fetch_tasks: Option<String>,
    help: bool,
    version: bool,
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--list" => options.list = true,
            "--json" => options.json = true,
            "--dry-run" => options.dry_run = true,
            "--yes" => {}
            "--recent" => options.recent = true,
            "--logs" => options.logs = true,
            "--doctor" => options.doctor = true,
            "--run" => {
                i += 1;
                options.run = Some(
                    args.get(i)
                        .ok_or_else(|| "--run needs a task name or number".to_string())?
                        .clone(),
                );
            }
            "--arg" => {
                i += 1;
                options.arg = Some(
                    args.get(i)
                        .ok_or_else(|| "--arg needs a value".to_string())?
                        .clone(),
                );
            }
            "--fetch-tasks" => {
                i += 1;
                options.fetch_tasks = Some(
                    args.get(i)
                        .ok_or_else(|| "--fetch-tasks needs a URL".to_string())?
                        .clone(),
                );
            }
            "-h" | "--help" => options.help = true,
            "-V" | "--version" => options.version = true,
            other if other.starts_with('-') => {
                return Err(format!("unknown option '{}'", other));
            }
            _ => return Err(format!("unexpected argument '{}'", args[i])),
        }
        i += 1;
    }
    Ok(options)
}

fn find_task(tasks: &[Task], query: &str) -> Option<usize> {
    if let Some(index) = tasks
        .iter()
        .position(|task| task.name.eq_ignore_ascii_case(query))
    {
        return Some(index);
    }
    let number: usize = query.parse().ok()?;
    if number > 0 && number <= tasks.len() {
        Some(number - 1)
    } else {
        None
    }
}

fn run_task_cli(tasks: &[Task], name: &str, arg: Option<&str>, dry_run: bool) -> i32 {
    let Some(idx) = find_task(tasks, name) else {
        eprintln!("bashbox: no task named '{}' (try --list)", name);
        return 2;
    };
    let task = &tasks[idx];
    if dry_run {
        match (&task.action, arg) {
            (Action::Script(_), Some(arg)) => {
                println!(
                    "{}",
                    task.command_with_arg(arg).unwrap_or_else(|| task.preview())
                );
            }
            _ => println!("{}", task.preview()),
        }
        return 0;
    }
    if let Action::Edit(path) = &task.action {
        return match open_editor(&editor_name(), path).or_else(|_| open_editor("vi", path)) {
            Ok(status) if status.success() => 0,
            Ok(status) => status.code().unwrap_or(1),
            Err(e) => {
                eprintln!("bashbox: could not open editor: {}", e);
                1
            }
        };
    }

    let mut action = task.action.clone();
    if task.prompt.is_some() {
        let Some(arg) = arg else {
            eprintln!(
                "bashbox: task '{}' needs --arg <value> ({})",
                task.name,
                task.prompt.as_deref().unwrap_or("input")
            );
            return 2;
        };
        let Some(command) = task.command_with_arg(arg) else {
            eprintln!("bashbox: task '{}' has no command to run", task.name);
            return 2;
        };
        action = Action::Script(command);
    }

    if matches!(action, Action::Script(_))
        && script_needs_sudo(tasks, idx)
        && !is_root()
        && !sudo_cached_ok()
        && !Command::new("sudo")
            .arg("-v")
            .status()
            .is_ok_and(|s| s.success())
    {
        eprintln!("bashbox: sudo authentication failed");
        return 1;
    }

    let mut state = state::load();
    state.record_recent(&task.name);
    state::save(&state);

    let rx = tasks::spawn(task.name.clone(), action, task.timeout);
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    let mut failed = false;
    for msg in rx {
        match msg {
            EventMsg::Line(line) => {
                let _ = writeln!(out, "{}", line);
            }
            EventMsg::Done { failed: f, log } => {
                failed = f;
                if let Some(path) = log {
                    let _ = writeln!(out, "Failure log: {}", path.display());
                }
                break;
            }
            EventMsg::Pid(_) => {}
        }
    }
    let _ = out.flush();
    if failed {
        1
    } else {
        0
    }
}

fn print_recent() {
    let state = state::load();
    if state.recent.is_empty() {
        println!("No tasks have been run yet.");
        return;
    }
    for (i, name) in state.recent.iter().enumerate() {
        println!("{}. {}", i + 1, name);
    }
}

fn print_logs() {
    let Some(path) = tasks::last_log_path() else {
        println!("HOME is not set.");
        return;
    };
    match std::fs::read_to_string(&path) {
        Ok(text) => {
            println!("{}", path.display());
            println!();
            print!("{}", text);
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            println!("No failed task has been recorded yet.");
        }
        Err(e) => println!("Could not read {}: {}", path.display(), e),
    }
}

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let options = match parse_args(&args) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("bashbox: {}", message);
            eprintln!("Try 'bashbox --help'.");
            std::process::exit(2);
        }
    };

    if options.help {
        print_help();
        return Ok(());
    }
    if options.version {
        println!("bashbox {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if let Some(url) = &options.fetch_tasks {
        match tasks::fetch_tasks(url) {
            Ok(path) => println!("Saved tasks to {}", path.display()),
            Err(e) => {
                eprintln!("bashbox: {}", e);
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    if options.recent {
        print_recent();
        return Ok(());
    }
    if options.logs {
        print_logs();
        return Ok(());
    }
    if options.doctor {
        tasks::doctor(&|line| println!("{}", line));
        return Ok(());
    }

    let tasks = tasks::build();
    if let Some(run) = &options.run {
        std::process::exit(run_task_cli(
            &tasks,
            run,
            options.arg.as_deref(),
            options.dry_run,
        ));
    }
    if options.list {
        print_list(&tasks, options.json);
        return Ok(());
    }

    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        eprintln!("bashbox: the toolbox needs an interactive terminal.");
        eprintln!("Use --list or --doctor for non-interactive output.");
        std::process::exit(1);
    }

    let mut terminal = ratatui::init();
    let _ = execute!(io::stdout(), EnableMouseCapture);
    let result = run(&mut terminal, &tasks);
    let _ = execute!(io::stdout(), DisableMouseCapture);
    ratatui::restore();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_encodes() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"hello"), "aGVsbG8=");
    }

    #[test]
    fn parses_options() {
        let args = vec![
            "bashbox".to_string(),
            "--run".to_string(),
            "Disk usage".to_string(),
            "--arg".to_string(),
            "x".to_string(),
            "--dry-run".to_string(),
        ];
        let options = parse_args(&args).expect("valid args");
        assert_eq!(options.run.as_deref(), Some("Disk usage"));
        assert_eq!(options.arg.as_deref(), Some("x"));
        assert!(options.dry_run);
    }
}
