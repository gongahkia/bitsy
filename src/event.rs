// editor event system for plugin pub-sub

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EditorEvent {
    BufferOpen { path: String },
    BufferSave { path: String },
    BufferClose { path: String },
    BufferModified,
    CursorMoved { line: usize, col: usize },
    ModeChanged { mode: String },
    InsertChar { ch: char },
    InsertLeave,
    CommandExecuted { command: String },
}

pub type EventHandler = Box<dyn Fn(&EditorEvent, &mut EditorApi)>;

/// safe API facade for plugins
pub struct EditorApi {
    messages: Vec<String>,
    commands: Vec<String>,
    inserts: Vec<(usize, usize, String)>, // (line, col, text)
    cursor_moves: Vec<(usize, usize)>,
}

impl EditorApi {
    pub fn new() -> Self {
        Self { messages: Vec::new(), commands: Vec::new(), inserts: Vec::new(), cursor_moves: Vec::new() }
    }
    pub fn show_message(&mut self, msg: &str) { self.messages.push(msg.to_string()); }
    pub fn execute_command(&mut self, cmd: &str) { self.commands.push(cmd.to_string()); }
    pub fn insert_text(&mut self, line: usize, col: usize, text: &str) {
        self.inserts.push((line, col, text.to_string()));
    }
    pub fn set_cursor(&mut self, line: usize, col: usize) { self.cursor_moves.push((line, col)); }

    // drain results for editor to apply
    pub fn drain_messages(&mut self) -> Vec<String> { std::mem::take(&mut self.messages) }
    pub fn drain_commands(&mut self) -> Vec<String> { std::mem::take(&mut self.commands) }
    pub fn drain_inserts(&mut self) -> Vec<(usize, usize, String)> { std::mem::take(&mut self.inserts) }
    pub fn drain_cursor_moves(&mut self) -> Vec<(usize, usize)> { std::mem::take(&mut self.cursor_moves) }
}

pub struct EventBus {
    handlers: HashMap<String, Vec<EventHandler>>, // plugin_name -> handlers
    subscriptions: Vec<(String, Vec<EditorEvent>)>, // (plugin_name, events)
}

impl EventBus {
    pub fn new() -> Self {
        Self { handlers: HashMap::new(), subscriptions: Vec::new() }
    }

    pub fn subscribe(&mut self, plugin_name: &str, events: Vec<EditorEvent>, handler: EventHandler) {
        self.handlers.entry(plugin_name.to_string()).or_default().push(handler);
        self.subscriptions.push((plugin_name.to_string(), events));
    }

    pub fn emit(&self, event: &EditorEvent, api: &mut EditorApi) {
        for (plugin_name, subscribed_events) in &self.subscriptions {
            if subscribed_events.contains(event) || subscribed_events.is_empty() {
                if let Some(handlers) = self.handlers.get(plugin_name) {
                    for handler in handlers { handler(event, api); }
                }
            }
        }
    }
}

/// plugin trait -- plugins implement this
pub trait Plugin {
    fn name(&self) -> &str;
    fn on_event(&mut self, event: &EditorEvent, api: &mut EditorApi);
    fn commands(&self) -> Vec<String> { Vec::new() } // commands this plugin registers
}

/// simple auto-pairs plugin as example
pub struct AutoPairsPlugin;

impl Plugin for AutoPairsPlugin {
    fn name(&self) -> &str { "auto-pairs" }
    fn on_event(&mut self, event: &EditorEvent, api: &mut EditorApi) {
        if let EditorEvent::InsertChar { ch } = event {
            let pair = match ch {
                '(' => Some(')'),
                '[' => Some(']'),
                '{' => Some('}'),
                '"' => Some('"'),
                '\'' => Some('\''),
                _ => None,
            };
            if let Some(closing) = pair {
                api.insert_text(0, 0, &closing.to_string()); // placeholder -- editor applies at cursor
            }
        }
    }
}

/// comment toggle plugin example
pub struct CommentTogglePlugin {
    pub comment_str: String, // e.g. "// " for Rust
}

impl Plugin for CommentTogglePlugin {
    fn name(&self) -> &str { "comment-toggle" }
    fn on_event(&mut self, _event: &EditorEvent, _api: &mut EditorApi) {}
    fn commands(&self) -> Vec<String> { vec!["ToggleComment".to_string()] }
}
