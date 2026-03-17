// LSP client: JSON-RPC over stdio, no async runtime

mod protocol;
mod transport;

use std::collections::HashMap;
use std::path::Path;
use crate::config::Config;
pub use protocol::{CompletionItem, Diagnostic, DiagnosticSeverity, Position, Range as LspRange};

pub struct LspClient {
    transport: Option<transport::Transport>,
    initialized: bool,
    server_capabilities: Option<serde_json::Value>,
    request_id: u64,
    pending_completions: Vec<CompletionItem>,
    pub diagnostics: HashMap<String, Vec<Diagnostic>>, // uri -> diagnostics
}

impl LspClient {
    pub fn new() -> Self {
        Self {
            transport: None,
            initialized: false,
            server_capabilities: None,
            request_id: 0,
            pending_completions: Vec::new(),
            diagnostics: HashMap::new(),
        }
    }

    pub fn start(&mut self, command: &str, args: &[&str], root_path: &Path) -> Result<(), String> {
        let t = transport::Transport::spawn(command, args)
            .map_err(|e| format!("Failed to start LSP server: {}", e))?;
        self.transport = Some(t);
        self.send_initialize(root_path)?;
        Ok(())
    }

    pub fn is_running(&self) -> bool { self.transport.is_some() && self.initialized }

    fn next_id(&mut self) -> u64 { self.request_id += 1; self.request_id }

    fn send_initialize(&mut self, root_path: &Path) -> Result<(), String> {
        let id = self.next_id();
        let root_uri = format!("file://{}", root_path.display());
        let params = serde_json::json!({
            "processId": std::process::id(),
            "rootUri": root_uri,
            "capabilities": {
                "textDocument": {
                    "completion": { "completionItem": { "snippetSupport": false } },
                    "hover": { "contentFormat": ["plaintext"] },
                    "publishDiagnostics": { "relatedInformation": false }
                }
            }
        });
        self.send_request("initialize", id, params)?;
        // poll for response
        if let Some(resp) = self.poll_response(id, 5000) {
            self.server_capabilities = resp.get("result").cloned();
            self.send_notification("initialized", serde_json::json!({}))?;
            self.initialized = true;
        }
        Ok(())
    }

    pub fn did_open(&mut self, uri: &str, language_id: &str, text: &str) -> Result<(), String> {
        if !self.is_running() { return Ok(()); }
        self.send_notification("textDocument/didOpen", serde_json::json!({
            "textDocument": { "uri": uri, "languageId": language_id, "version": 1, "text": text }
        }))
    }

    pub fn did_change(&mut self, uri: &str, version: i64, text: &str) -> Result<(), String> {
        if !self.is_running() { return Ok(()); }
        self.send_notification("textDocument/didChange", serde_json::json!({
            "textDocument": { "uri": uri, "version": version },
            "contentChanges": [{ "text": text }]
        }))
    }

    pub fn did_save(&mut self, uri: &str) -> Result<(), String> {
        if !self.is_running() { return Ok(()); }
        self.send_notification("textDocument/didSave", serde_json::json!({
            "textDocument": { "uri": uri }
        }))
    }

    pub fn completion(&mut self, uri: &str, line: u32, character: u32) -> Result<Vec<CompletionItem>, String> {
        if !self.is_running() { return Ok(Vec::new()); }
        let id = self.next_id();
        self.send_request("textDocument/completion", id, serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": line, "character": character }
        }))?;
        if let Some(resp) = self.poll_response(id, 2000) {
            if let Some(result) = resp.get("result") {
                return Ok(protocol::parse_completion(result));
            }
        }
        Ok(Vec::new())
    }

    pub fn hover(&mut self, uri: &str, line: u32, character: u32) -> Result<Option<String>, String> {
        if !self.is_running() { return Ok(None); }
        let id = self.next_id();
        self.send_request("textDocument/hover", id, serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": line, "character": character }
        }))?;
        if let Some(resp) = self.poll_response(id, 2000) {
            if let Some(result) = resp.get("result") {
                return Ok(protocol::parse_hover(result));
            }
        }
        Ok(None)
    }

    pub fn definition(&mut self, uri: &str, line: u32, character: u32) -> Result<Option<(String, u32, u32)>, String> {
        if !self.is_running() { return Ok(None); }
        let id = self.next_id();
        self.send_request("textDocument/definition", id, serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": line, "character": character }
        }))?;
        if let Some(resp) = self.poll_response(id, 2000) {
            if let Some(result) = resp.get("result") {
                return Ok(protocol::parse_definition(result));
            }
        }
        Ok(None)
    }

    /// poll for notifications (diagnostics, etc) -- non-blocking
    pub fn poll_notifications(&mut self) {
        if let Some(ref mut t) = self.transport {
            while let Some(msg) = t.try_recv() {
                if let Some(method) = msg.get("method").and_then(|m| m.as_str()) {
                    if method == "textDocument/publishDiagnostics" {
                        if let Some(params) = msg.get("params") {
                            if let (Some(uri), Some(diags)) = (
                                params.get("uri").and_then(|u| u.as_str()),
                                params.get("diagnostics").and_then(|d| d.as_array()),
                            ) {
                                let parsed: Vec<Diagnostic> = diags.iter()
                                    .filter_map(|d| protocol::parse_diagnostic(d))
                                    .collect();
                                self.diagnostics.insert(uri.to_string(), parsed);
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn stop(&mut self) {
        if self.transport.is_some() {
            let id = self.next_id();
            let _ = self.send_request("shutdown", id, serde_json::json!(null));
            let _ = self.send_notification("exit", serde_json::json!(null));
            if let Some(ref mut t) = self.transport { t.kill(); }
        }
        self.transport = None;
        self.initialized = false;
    }

    fn send_request(&mut self, method: &str, id: u64, params: serde_json::Value) -> Result<(), String> {
        let msg = serde_json::json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        if let Some(ref mut t) = self.transport {
            t.send(&msg).map_err(|e| e.to_string())
        } else { Err("LSP not connected".into()) }
    }

    fn send_notification(&mut self, method: &str, params: serde_json::Value) -> Result<(), String> {
        let msg = serde_json::json!({ "jsonrpc": "2.0", "method": method, "params": params });
        if let Some(ref mut t) = self.transport {
            t.send(&msg).map_err(|e| e.to_string())
        } else { Err("LSP not connected".into()) }
    }

    fn poll_response(&mut self, id: u64, timeout_ms: u64) -> Option<serde_json::Value> {
        if let Some(ref mut t) = self.transport {
            let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
            while std::time::Instant::now() < deadline {
                if let Some(msg) = t.try_recv() {
                    if msg.get("id").and_then(|i| i.as_u64()) == Some(id) {
                        return Some(msg);
                    }
                    // handle notifications while waiting
                    if let Some(method) = msg.get("method").and_then(|m| m.as_str()) {
                        if method == "textDocument/publishDiagnostics" {
                            if let Some(params) = msg.get("params") {
                                if let (Some(uri), Some(diags)) = (
                                    params.get("uri").and_then(|u| u.as_str()),
                                    params.get("diagnostics").and_then(|d| d.as_array()),
                                ) {
                                    let parsed: Vec<Diagnostic> = diags.iter()
                                        .filter_map(|d| protocol::parse_diagnostic(d))
                                        .collect();
                                    self.diagnostics.insert(uri.to_string(), parsed);
                                }
                            }
                        }
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
        None
    }
}

impl Drop for LspClient {
    fn drop(&mut self) { self.stop(); }
}

/// determine LSP server command for a file extension
pub fn server_for_extension(ext: &str) -> Option<(&'static str, &'static [&'static str])> {
    match ext {
        "rs" => Some(("rust-analyzer", &[])),
        "py" => Some(("pyright-langserver", &["--stdio"])),
        "js" | "jsx" | "ts" | "tsx" => Some(("typescript-language-server", &["--stdio"])),
        "go" => Some(("gopls", &[])),
        "c" | "cpp" | "h" | "hpp" => Some(("clangd", &[])),
        _ => None,
    }
}
