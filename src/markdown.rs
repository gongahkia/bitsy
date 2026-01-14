
use anyhow::Result;
use lazy_static::lazy_static;
use portpicker::pick_unused_port;
use pulldown_cmark::{html, Options, Parser};
use std::fs;
use std::io::Read;
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use tiny_http::{Response, Server};
use webbrowser;
use ws::{listen, Message, Sender};

const HTML_WRAPPER: &str = r#"
<!DOCTYPE html>
<html>
<head>
<title>Markdown Preview</title>
<style>
    body { font-family: sans-serif; }
    .container { max-width: 800px; margin: 0 auto; padding: 20px; }
</style>
</head>
<body>
<div class="container" id="content">
</div>
<script>
    const content = document.getElementById('content');
    const socket = new WebSocket('ws://##WEBSOCKET_ADDR##');
    socket.onmessage = function(event) {
        content.innerHTML = event.data;
    };
</script>
</body>
</html>
"#;

lazy_static! {
    static ref MARKDOWN_SERVER: Arc<Mutex<Option<MarkdownServer>>> = Arc::new(Mutex::new(None));
}

struct MarkdownServer {
    http_port: u16,
    ws_port: u16,
    broadcaster: Sender,
}

pub fn open_preview_if_markdown(file_path: &str) {
    if file_path.ends_with(".md") || file_path.ends_with(".markdown") {
        let mut server_guard = MARKDOWN_SERVER.lock().unwrap();
        if server_guard.is_none() {
            *server_guard = Some(MarkdownServer::start().expect("Failed to start markdown server"));
        }

        if let Some(server) = server_guard.as_ref() {
            let _ = webbrowser::open(&format!("http://localhost:{}", server.http_port));
            update_preview(file_path);
        }
    }
}

pub fn update_preview(file_path: &str) {
    if let Some(server) = MARKDOWN_SERVER.lock().unwrap().as_ref() {
        if let Ok(markdown) = fs::read_to_string(file_path) {
            let html = markdown_to_html(&markdown);
            server.broadcast_html(&html);
        }
    }
}

impl MarkdownServer {
    fn start() -> Result<Self> {
        let http_port = pick_unused_port().expect("No free ports");
        let ws_port = pick_unused_port().expect("No free ports");

        let (tx, rx) = mpsc::channel();

        let ws_server = thread::spawn(move || {
            listen(format!("127.0.0.1:{}", ws_port), |out| {
                tx.send(out).unwrap();
                move |msg| Ok(())
            })
            .unwrap();
        });

        let broadcaster = rx.recv()?;

        let http_server = thread::spawn(move || {
            let addr = format!("127.0.0.1:{}", http_port);
            let server = Server::http(&addr).unwrap();
            for request in server.incoming_requests() {
                let html_content = HTML_WRAPPER.replace("##WEBSOCKET_ADDR##", &format!("127.0.0.1:{}", ws_port));
                let response = Response::from_string(html_content).with_header(
                    "Content-Type: text/html".parse::<tiny_http::Header>().unwrap(),
                );
                request.respond(response).unwrap();
            }
        });

        Ok(Self {
            http_port,
            ws_port,
            broadcaster,
        })
    }

    fn broadcast_html(&self, html: &str) {
        self.broadcaster.broadcast(Message::text(html)).unwrap();
    }
}

fn markdown_to_html(markdown: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    let parser = Parser::new_ext(markdown, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}
