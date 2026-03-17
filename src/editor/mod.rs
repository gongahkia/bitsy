// main editor coordination -- thin coordinator struct

mod action;
mod command_exec;
mod history;
mod input;
mod motion;
mod operator;
mod render;
mod surround;

use crossterm::event::{Event, KeyEvent};
use notify::{RecommendedWatcher, Watcher};
use pulldown_cmark::{html, Options, Parser};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tiny_http::{Response, Server};

use crate::buffer::Buffer;
use crate::command_bar::CommandBar;
use crate::config::Config;
use crate::cursor::Cursor;
use crate::error::{Error, Result};
use crate::fuzzy_finder::FuzzyFinder;
use crate::keymap::Action;
use crate::mode::Mode;
use crate::register::RegisterManager;
use crate::screen::Screen;
use crate::selection::Selection;
use crate::statusline::StatusLine;
use crate::syntax::SyntaxHighlighter;
use crate::terminal::Terminal;
use crate::theme::Theme;
use crate::undo::UndoManager;
use crate::event::{EditorApi, EditorEvent, EventBus, Plugin};
use crate::lsp::LspClient;
use crate::window::Window;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PendingOperator {
    None,
    Delete,
    Change,
    Yank,
    MakeLowercase,
    MakeUppercase,
    ToggleCase,
    Indent,
    Dedent,
    AutoIndent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MarkAction {
    Set,
    Jump,
    JumpExact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TextObjectModifier {
    Around,
    Inner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FindDirection {
    Forward,
    Backward,
    Till,
    TillBack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CaseChange {
    Lower,
    Upper,
    Toggle,
}

pub struct Editor {
    terminal: Terminal,
    screen: Screen,
    theme: Theme,
    buffers: Vec<Buffer>,
    pub(crate) undo_manager: UndoManager,
    mode: Mode,
    statusline: StatusLine,
    command_bar: CommandBar,
    command_buffer: String,
    message: Option<String>,
    should_quit: bool,
    registers: RegisterManager,
    pending_operator: PendingOperator,
    config: Config,
    selection: Option<Selection>,
    last_find: Option<(char, FindDirection)>,
    pending_key: Option<char>,
    count: usize,
    last_change: Option<(Action, usize)>,
    search_buffer: String,
    search_pattern: Option<String>,
    search_forward: bool,
    substitute_preview_pattern: Option<String>,
    substitute_preview_range: Option<(usize, usize)>,
    visual_cmd_range: Option<(usize, usize)>,
    pending_text_object: Option<TextObjectModifier>,
    pending_register: Option<char>,
    waiting_for_register: bool,
    global_marks: HashMap<char, (usize, usize)>,
    change_list: Vec<(usize, usize)>,
    change_index: usize,
    waiting_for_mark: Option<MarkAction>,
    command_history: Vec<String>,
    history_index: Option<usize>,
    completion_candidates: Vec<String>,
    completion_index: Option<usize>,
    jump_list: Vec<(usize, usize)>,
    jump_index: usize,
    recording_register: Option<char>,
    macro_buffer: Vec<KeyEvent>,
    last_macro_register: Option<char>,
    windows: Vec<Window>,
    active_window: usize,
    needs_render: bool,
    showing_landing_page: bool,
    viewing_help: bool,
    help_return_buffer: Option<Buffer>,
    help_return_cursor: Option<Cursor>,
    was_showing_landing_page: bool,
    markdown_preview_server: Option<JoinHandle<()>>,
    markdown_preview_url: Option<String>,
    markdown_preview_shutdown: Option<Arc<AtomicBool>>,
    zen_mode: bool,
    file_watcher: Option<RecommendedWatcher>,
    file_events: Option<Receiver<notify::Result<notify::Event>>>,
    file_changed_externally: bool,
    fuzzy_finder: Option<FuzzyFinder>,
    syntax: SyntaxHighlighter,
    layout: crate::window::Layout,
    event_bus: EventBus,
    plugins: Vec<Box<dyn Plugin>>,
    leader_pending: bool,           // waiting for key after leader
    surround_pending: Option<char>, // 'c' for cs, 'd' for ds, 'y' for ys
    surround_ys_pending: bool,      // waiting for text object after ys
    lsp_client: LspClient,
}

impl Editor {
    pub fn new() -> Result<Self> {
        let terminal = Terminal::new()?;
        let (width, height) = terminal.size();
        let viewport_height = (height as usize).saturating_sub(2);
        let window = Window::new(0, width as usize, viewport_height);
        let config = Config::load_from_file("editor.toml");
        let theme = Theme::webspinner();
        let screen = Screen::new(width as usize, height as usize);
        Ok(Self {
            terminal,
            screen,
            theme,
            buffers: vec![Buffer::new()],
            undo_manager: UndoManager::new(),
            mode: Mode::Normal,
            statusline: StatusLine::new(),
            command_bar: CommandBar::new(),
            command_buffer: String::new(),
            message: None,
            should_quit: false,
            registers: RegisterManager::new(),
            pending_operator: PendingOperator::None,
            config,
            selection: None,
            last_find: None,
            pending_key: None,
            count: 0,
            last_change: None,
            search_buffer: String::new(),
            search_pattern: None,
            search_forward: true,
            substitute_preview_pattern: None,
            substitute_preview_range: None,
            visual_cmd_range: None,
            pending_text_object: None,
            pending_register: None,
            waiting_for_register: false,
            global_marks: HashMap::new(),
            change_list: Vec::new(),
            change_index: 0,
            waiting_for_mark: None,
            command_history: Vec::new(),
            history_index: None,
            completion_candidates: Vec::new(),
            completion_index: None,
            jump_list: Vec::new(),
            jump_index: 0,
            recording_register: None,
            macro_buffer: Vec::new(),
            last_macro_register: None,
            windows: vec![window],
            active_window: 0,
            needs_render: true,
            showing_landing_page: false,
            viewing_help: false,
            help_return_buffer: None,
            help_return_cursor: None,
            was_showing_landing_page: false,
            markdown_preview_server: None,
            markdown_preview_url: None,
            markdown_preview_shutdown: None,
            zen_mode: false,
            file_watcher: None,
            file_events: None,
            file_changed_externally: false,
            fuzzy_finder: None,
            syntax: SyntaxHighlighter::new(),
            layout: crate::window::Layout::new_leaf(0),
            event_bus: EventBus::new(),
            plugins: Vec::new(),
            leader_pending: false,
            surround_pending: None,
            surround_ys_pending: false,
            lsp_client: LspClient::new(),
        })
    }

    pub(crate) fn current_buffer(&self) -> &Buffer {
        &self.buffers[self.windows[self.active_window].buffer_index]
    }

    pub(crate) fn current_buffer_mut(&mut self) -> &mut Buffer {
        let buffer_idx = self.windows[self.active_window].buffer_index;
        &mut self.buffers[buffer_idx]
    }

    pub(crate) fn current_window(&self) -> &Window {
        &self.windows[self.active_window]
    }

    pub(crate) fn current_window_mut(&mut self) -> &mut Window {
        &mut self.windows[self.active_window]
    }

    pub fn open<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let path = path.as_ref();
        let buffer = Buffer::from_file(path, &self.config)?;
        self.buffers[0] = buffer;
        self.windows[0].cursor = Cursor::default();
        self.registers.update_filename(path.to_string_lossy().to_string());
        if path.extension().map_or(false, |ext| ext == "md" || ext == "markdown") {
            self.start_markdown_preview(path.to_path_buf())?;
        } else {
            self.stop_markdown_preview();
        }
        self.stop_file_watcher();
        let path_to_watch = self.current_buffer().file_path().map(|p| p.to_path_buf());
        if let Some(p) = path_to_watch {
            if let Err(e) = self.start_file_watcher(&p) {
                log::error!("Failed to start file watcher: {}", e);
            }
        }
        // try to start LSP for this file type
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if let Some((cmd, args)) = crate::lsp::server_for_extension(ext) {
            let root = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            if let Err(e) = self.lsp_client.start(cmd, args, &root) {
                log::warn!("LSP start failed: {}", e);
            } else {
                let uri = format!("file://{}", path.display());
                let lang = ext.to_string();
                let text = self.current_buffer().get_all_text().unwrap_or_default();
                let _ = self.lsp_client.did_open(&uri, &lang, &text);
            }
        }
        self.emit_event(EditorEvent::BufferOpen { path: path.to_string_lossy().to_string() });
        Ok(())
    }

    pub fn show_landing_page(&mut self) {
        let (width, _height) = self.terminal.size();
        let logo = r#"░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░
░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░
░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░
░░░░░░░░░░░░░░░░░░░░░░░░░░░░░█░░░░░░░░░░░░░░░░░░░░
░░░░░░░░░░░░░░░░█░░▓░░░░░░░░░░░░░█░░░░░░░░░░░░░░░░
░░░░░░░░░░░░░░░░█░░█░░░░░░░░░░█░░░░░░░░▒░░░░░░░░░░
░░░░░░░░░░░░░░░░░░░█░░░░▓░▒░░░█░░░█░░░░░░░░░░░░░░░
░░░░░░░░░░░░░░░░█░░▓░░░█░░█░░░█░░█░░░░░█░░░░░░░░░░
░░░░░░░░░░▒░░░░░░█░░░████████░░░█░░░░░█░░░░░░░░░░░
░░░░░░░░░░░██░░░░░░░░████████░░░░░░░██░░░░░░░░░░░░
░░░░░░█░░░░░░░░███░░░░███████░░░░▒░░░░░░░░░█░░░░░░
░░░░░░█░░░░░░░░░░░███████░██████░░░░░░░░░░░█░░░░░░
░░░░░░░█░░░░░░░█░░░▓███████████░░░██░░░░░░█░░░░░░░
░░░░░░░░░░░░░░░░░▒█████████████▓█░░░░░░░░░░░░░░░░░
░░░░░░░░░░░░░░██░░░██████░█████░░░░█░░░░░░░░░░░░░░
░░░░░░░░░░░░░█░░░░█░░░███████░░░█░░░░█░░░░░░░░░░░░
░░░░░░░░░░░█░░░░░█░░░░░░░▓░░░░░░░█░░░░░█░░░░░░░░░░
░░░░░░░░░░█░░░░░░█░░░░░░░░░░░░░░░█░░░░░█░░░░░░░░░░
░░░░░░░░░░░░░░░░░▓░░░░░░░░░░░░░░░█░░░░░█░░░░░░░░░░
░░░░░░░░░░░█░░░░█░░░░░░░░░░░░░░░░█░░░░░█░░░░░░░░░░
░░░░░░░░░░░▓░░░░░█░░░░░░░░░░░░░░▓░░░░░█░░░░░░░░░░░
░░░░░░░░░░░░░▓░░░░█░░░░░░░░░░░░▒░░░░▒░░░░░░░░░░░░░
░░░░░░░░░░░░░░░░░░░█░░░░░░░░░░░█░░░░░░░░░░░░░░░░░░
░░░░░░░░░░░░░░░░░░░░░█░░░░░░░█░░░░░░░░░░░░░░░░░░░░
░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░
░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░
░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░"#;
        let instructions = vec![
            "", "bitsy v0.1.0", "a vim-compatible text editor",
            "made by @gongahkia on github", "",
            ":help       view help", ":e <file>   open a file", "",
            "note: this is also a buffer and can be edited!",
        ];
        let mut lines: Vec<String> = Vec::new();
        for logo_line in logo.lines() {
            let padding = (width as usize).saturating_sub(logo_line.chars().count()) / 2;
            lines.push(format!("{}{}", " ".repeat(padding), logo_line));
        }
        lines.push(String::new());
        for line in &instructions {
            let padding = (width as usize).saturating_sub(line.len()) / 2;
            lines.push(format!("{}{}", " ".repeat(padding), line));
        }
        let content = lines.join("\n");
        self.buffers[0] = Buffer::from_string(&content);
        self.current_buffer_mut().clear_modified();
        self.showing_landing_page = true;
        self.windows[0].cursor = Cursor::default();
    }

    pub fn run(&mut self) -> Result<()> {
        loop {
            if self.needs_render {
                self.render()?;
                self.needs_render = false;
            }
            if self.should_quit {
                self.stop_markdown_preview();
                break;
            }
            self.lsp_client.poll_notifications();
            self.check_for_file_changes();
            if let Some(event) = self.terminal.read_event()? {
                self.handle_event(event)?;
                self.needs_render = true;
            }
        }
        Ok(())
    }

    fn check_for_file_changes(&mut self) {
        if let Some(rx) = &self.file_events {
            if let Ok(Ok(event)) = rx.try_recv() {
                if matches!(event.kind, notify::EventKind::Modify(_)) {
                    self.file_changed_externally = true;
                    self.message = Some("File changed on disk. Reload? (y/n)".to_string());
                }
            }
        }
    }

    fn handle_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::Key(key) => self.handle_key(key)?,
            Event::Resize(width, height) => {
                self.terminal.update_size()?;
                let viewport_height = (height as usize).saturating_sub(2);
                self.windows[self.active_window]
                    .viewport
                    .resize(width as usize, viewport_height);
                self.screen.resize(width as usize, height as usize);
            }
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn emit_event(&mut self, event: EditorEvent) {
        let mut api = EditorApi::new();
        // run event bus handlers
        self.event_bus.emit(&event, &mut api);
        // run plugin handlers
        for plugin in &mut self.plugins {
            plugin.on_event(&event, &mut api);
        }
        // apply plugin API results
        for msg in api.drain_messages() {
            self.message = Some(msg);
        }
        for (line, col, text) in api.drain_inserts() {
            let target_line = if line == 0 { self.current_window().cursor.line } else { line };
            let target_col = if col == 0 { self.current_window().cursor.col } else { col };
            for (i, ch) in text.chars().enumerate() {
                self.current_buffer_mut().insert_char(target_line, target_col + i, ch);
            }
        }
        for (line, col) in api.drain_cursor_moves() {
            self.current_window_mut().cursor.line = line;
            self.current_window_mut().cursor.col = col;
        }
    }

    pub fn register_plugin(&mut self, plugin: Box<dyn Plugin>) {
        self.plugins.push(plugin);
    }

    pub(crate) fn recalculate_window_rects(&mut self) {
        let (w, h) = self.terminal.size();
        let viewport_height = (h as usize).saturating_sub(2);
        let bounding = crate::window::Rect { x: 0, y: 0, width: w as usize, height: viewport_height };
        let rects = self.layout.calculate_rects(bounding);
        for (win_idx, rect) in rects {
            if let Some(win) = self.windows.get_mut(win_idx) {
                win.set_rect(rect);
            }
        }
    }

    pub(crate) fn focus_direction(&mut self, dx: isize, dy: isize) {
        let cur = &self.windows[self.active_window].rect;
        let cx = cur.x as isize + cur.width as isize / 2 + dx * (cur.width as isize);
        let cy = cur.y as isize + cur.height as isize / 2 + dy * (cur.height as isize);
        let leaves = self.layout.leaves();
        let mut best = None;
        let mut best_dist = isize::MAX;
        for &idx in &leaves {
            if idx == self.active_window { continue; }
            if let Some(win) = self.windows.get(idx) {
                let wx = win.rect.x as isize + win.rect.width as isize / 2;
                let wy = win.rect.y as isize + win.rect.height as isize / 2;
                let dist = (cx - wx).abs() + (cy - wy).abs();
                if dist < best_dist { best_dist = dist; best = Some(idx); }
            }
        }
        if let Some(idx) = best { self.active_window = idx; }
    }

    fn stop_file_watcher(&mut self) {
        if let Some(mut watcher) = self.file_watcher.take() {
            if let Some(path) = self.current_buffer().file_path() {
                let _ = watcher.unwatch(path);
            }
        }
        self.file_events = None;
    }

    fn start_file_watcher(&mut self, path: &Path) -> notify::Result<()> {
        let (tx, rx) = channel();
        let mut watcher = RecommendedWatcher::new(tx, notify::Config::default())?;
        watcher.watch(path, notify::RecursiveMode::NonRecursive)?;
        self.file_watcher = Some(watcher);
        self.file_events = Some(rx);
        self.file_changed_externally = false;
        Ok(())
    }

    fn stop_markdown_preview(&mut self) {
        if let Some(shutdown) = self.markdown_preview_shutdown.take() {
            shutdown.store(true, Ordering::Relaxed);
        }
        if let Some(handle) = self.markdown_preview_server.take() {
            drop(handle);
        }
        self.markdown_preview_url = None;
    }

    fn start_markdown_preview(&mut self, path: PathBuf) -> Result<()> {
        self.stop_markdown_preview();
        let shutdown = Arc::new(AtomicBool::new(false));
        self.markdown_preview_shutdown = Some(shutdown.clone());
        let server = match Server::http("127.0.0.1:0") {
            Ok(s) => s,
            Err(e) => {
                return Err(Error::EditorError(format!("Failed to start markdown server: {}", e)));
            }
        };
        let addr = server.server_addr().to_string();
        let url = format!("http://{}", addr);
        self.markdown_preview_url = Some(url.clone());
        let handle = thread::spawn(move || {
            while !shutdown.load(Ordering::Relaxed) {
                if let Ok(Some(request)) = server.recv_timeout(Duration::from_millis(100)) {
                    let content = fs::read_to_string(&path)
                        .unwrap_or_else(|_| "Error reading file".to_string());
                    let mut options = Options::empty();
                    options.insert(Options::ENABLE_STRIKETHROUGH);
                    let parser = Parser::new_ext(&content, options);
                    let mut html_output = String::new();
                    html::push_html(&mut html_output, parser);
                    let response = Response::from_string(html_output).with_header(
                        "Content-Type: text/html".parse::<tiny_http::Header>().unwrap(),
                    );
                    let _ = request.respond(response);
                }
            }
        });
        self.markdown_preview_server = Some(handle);
        if webbrowser::open(&url).is_ok() {
            self.message = Some(format!("Markdown preview started at {}", url));
        } else {
            self.message = Some(format!("Preview running at {}, but couldn't open browser.", url));
        }
        Ok(())
    }
}

impl Drop for Editor {
    fn drop(&mut self) {
        self.lsp_client.stop();
        self.stop_markdown_preview();
        self.stop_file_watcher();
        for buffer in &self.buffers {
            buffer.remove_backup();
        }
    }
}
