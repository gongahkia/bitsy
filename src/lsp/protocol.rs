// LSP protocol types and parsing helpers

#[derive(Debug, Clone)]
pub struct Position { pub line: u32, pub character: u32 }

#[derive(Debug, Clone)]
pub struct Range { pub start: Position, pub end: Position }

#[derive(Debug, Clone)]
pub struct CompletionItem {
    pub label: String,
    pub detail: Option<String>,
    pub kind: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity { Error, Warning, Information, Hint }

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub range: Range,
    pub severity: DiagnosticSeverity,
    pub message: String,
}

pub fn parse_completion(value: &serde_json::Value) -> Vec<CompletionItem> {
    let items = if let Some(arr) = value.as_array() { arr }
    else if let Some(arr) = value.get("items").and_then(|v| v.as_array()) { arr }
    else { return Vec::new(); };
    items.iter().filter_map(|item| {
        let label = item.get("label")?.as_str()?.to_string();
        let detail = item.get("detail").and_then(|d| d.as_str()).map(|s| s.to_string());
        let kind = item.get("kind").and_then(|k| k.as_u64()).map(|k| k as u32);
        Some(CompletionItem { label, detail, kind })
    }).collect()
}

pub fn parse_hover(value: &serde_json::Value) -> Option<String> {
    if value.is_null() { return None; }
    let contents = value.get("contents")?;
    if let Some(s) = contents.as_str() { return Some(s.to_string()); }
    if let Some(obj) = contents.as_object() {
        return obj.get("value").and_then(|v| v.as_str()).map(|s| s.to_string());
    }
    if let Some(arr) = contents.as_array() {
        let parts: Vec<String> = arr.iter().filter_map(|v| {
            if let Some(s) = v.as_str() { Some(s.to_string()) }
            else { v.get("value").and_then(|v| v.as_str()).map(|s| s.to_string()) }
        }).collect();
        if !parts.is_empty() { return Some(parts.join("\n")); }
    }
    None
}

pub fn parse_definition(value: &serde_json::Value) -> Option<(String, u32, u32)> {
    if value.is_null() { return None; }
    // can be Location or Location[]
    let loc = if let Some(arr) = value.as_array() { arr.first()? } else { value };
    let uri = loc.get("uri")?.as_str()?.to_string();
    let range = loc.get("range")?;
    let start = range.get("start")?;
    let line = start.get("line")?.as_u64()? as u32;
    let character = start.get("character")?.as_u64()? as u32;
    Some((uri, line, character))
}

pub fn parse_diagnostic(value: &serde_json::Value) -> Option<Diagnostic> {
    let range_val = value.get("range")?;
    let start = range_val.get("start")?;
    let end = range_val.get("end")?;
    let range = Range {
        start: Position {
            line: start.get("line")?.as_u64()? as u32,
            character: start.get("character")?.as_u64()? as u32,
        },
        end: Position {
            line: end.get("line")?.as_u64()? as u32,
            character: end.get("character")?.as_u64()? as u32,
        },
    };
    let severity = match value.get("severity").and_then(|s| s.as_u64()) {
        Some(1) => DiagnosticSeverity::Error,
        Some(2) => DiagnosticSeverity::Warning,
        Some(3) => DiagnosticSeverity::Information,
        _ => DiagnosticSeverity::Hint,
    };
    let message = value.get("message")?.as_str()?.to_string();
    Some(Diagnostic { range, severity, message })
}
