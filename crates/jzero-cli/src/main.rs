use std::env;
use std::fs;
use std::process::{self, Command};

use jzero_ast::tree::reset_ids;
use jzero_parser::parse_tree;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: j0 <source.java> [--png]");
        eprintln!();
        eprintln!("Parses a Jzero source file and outputs the syntax tree.");
        eprintln!();
        eprintln!("Options:");
        eprintln!("  --png    Also render the DOT file to PNG using Graphviz");
        process::exit(1);
    }

    let source_path = &args[1];
    let render_png = args.iter().any(|a| a == "--png");

    // Read source file
    let source = match fs::read_to_string(source_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading '{}': {}", source_path, e);
            process::exit(1);
        }
    };

    // Reset node IDs for deterministic output
    reset_ids();

    // Parse and build syntax tree
    let tree = match parse_tree(&source) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{}: {}", source_path, e);
            process::exit(1);
        }
    };

    // Print tree to stdout (like the book's j0)
    print!("{}", tree);

    // Write DOT file
    let dot_path = format!("{}.dot", source_path);
    let dot = tree.to_dot();
    if let Err(e) = fs::write(&dot_path, &dot) {
        eprintln!("Error writing '{}': {}", dot_path, e);
        process::exit(1);
    }
    eprintln!("DOT written to: {}", dot_path);

    // Optionally render PNG
    if render_png {
        let png_path = format!("{}.png", source_path);
        match Command::new("dot")
            .args(["-Tpng", &dot_path, "-o", &png_path])
            .status()
        {
            Ok(status) if status.success() => {
                eprintln!("PNG written to: {}", png_path);
            }
            Ok(status) => {
                eprintln!("dot exited with: {}", status);
                process::exit(1);
            }
            Err(e) => {
                eprintln!("Failed to run 'dot': {}", e);
                eprintln!("Install Graphviz to render PNGs: sudo apt install graphviz");
                process::exit(1);
            }
        }
    }
}