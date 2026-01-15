use bitsy::Editor;
use std::env;
use std::process;

fn main() {
    // Initialize logger
    env_logger::init();

    // Parse command-line arguments
    let args: Vec<String> = env::args().collect();

    // Create editor
    let mut editor = match Editor::new() {
        Ok(ed) => ed,
        Err(e) => {
            eprintln!("Failed to initialize editor: {}", e);
            process::exit(1);
        }
    };

    // Open file if specified, otherwise show landing page
    if args.len() > 1 {
        let filename = &args[1];
        if let Err(e) = editor.open(filename) {
            eprintln!("Failed to open file '{}': {}", filename, e);
            // Continue with empty buffer instead of exiting
        }
    } else {
        // No file specified, show the landing page
        editor.show_landing_page();
    }

    // Run the editor
    if let Err(e) = editor.run() {
        eprintln!("Editor error: {}", e);
        process::exit(1);
    }
}
