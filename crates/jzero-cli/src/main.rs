use std::env;
use std::fs;
use std::process::{self, Command};

use jzero_ast::tree::reset_ids;
use jzero_parser::parse_tree;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: j0 <source.java> [--png] [--codegen] [--bytecode] [--run]");
        eprintln!();
        eprintln!("Options:");
        eprintln!("  --png       Render the DOT file to PNG using Graphviz");
        eprintln!("  --codegen   Run semantic analysis + codegen, print TAC IR");
        eprintln!("  --bytecode  Compile to bytecode, print assembler listing");
        eprintln!("  --run       Compile to bytecode and execute it in the VM");
        process::exit(1);
    }

    let source_path = &args[1];
    let render_png    = args.iter().any(|a| a == "--png");
    let do_codegen    = args.iter().any(|a| a == "--codegen");
    let do_bytecode   = args.iter().any(|a| a == "--bytecode");
    let do_run        = args.iter().any(|a| a == "--run");

    // Read source file
    let source = match fs::read_to_string(source_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading '{}': {}", source_path, e);
            process::exit(1);
        }
    };

    reset_ids();

    let mut tree = match parse_tree(&source) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{}: {}", source_path, e);
            process::exit(1);
        }
    };

    // ── TAC IR path (--codegen) ───────────────────────────────────────────────
    if do_codegen {
        let sem = jzero_semantic::analyze(&mut tree);
        for err in &sem.errors { eprintln!("{}", err); }
        let ctx = jzero_codegen::generate(&tree, &sem);
        let asm = jzero_codegen::emit::emit(&tree, &ctx);
        print!("{}", asm);
        if sem.errors.is_empty() { println!("no errors"); }
        return;
    }

    // ── Bytecode path (--bytecode and/or --run) ───────────────────────────────
    if do_bytecode || do_run {
        let sem = jzero_semantic::analyze(&mut tree);
        for err in &sem.errors { eprintln!("{}", err); }
        if !sem.errors.is_empty() { process::exit(1); }

        // Collect program arguments (everything after the source file and flags).
        let prog_args: Vec<String> = args[2..].iter()
            .filter(|a| !a.starts_with("--"))
            .cloned()
            .collect();
        let argc = prog_args.len() as i64;

        let ctx    = jzero_codegen::generate(&tree, &sem);
        let output = jzero_codegen::pipeline::compile_bytecode(&tree, &ctx, argc);

        if do_bytecode {
            print!("{}", output.text);
            let j0_path = j0_path(source_path);
            if let Err(e) = fs::write(&j0_path, &output.binary) {
                eprintln!("Error writing '{}': {}", j0_path, e);
                process::exit(1);
            }
            eprintln!(".j0 written to: {}", j0_path);
        }

        if do_run {
            match jzero_vm::run(&output.binary, &prog_args) {
                Ok(out) => {
                    print!("{}", out);
                    println!("no errors");
                }
                Err(e) => {
                    eprintln!("VM error: {}", e);
                    process::exit(1);
                }
            }
        }
        return;
    }

    // ── Default path: tree + DOT ──────────────────────────────────────────────
    print!("{}", tree);

    let dot_path = format!("{}.dot", source_path);
    let dot = tree.to_dot();
    if let Err(e) = fs::write(&dot_path, &dot) {
        eprintln!("Error writing '{}': {}", dot_path, e);
        process::exit(1);
    }
    eprintln!("DOT written to: {}", dot_path);

    if render_png {
        let png_path = format!("{}.png", source_path);
        match Command::new("dot")
            .args(["-Tpng", &dot_path, "-o", &png_path])
            .status()
        {
            Ok(s) if s.success() => eprintln!("PNG written to: {}", png_path),
            Ok(s) => { eprintln!("dot exited with: {}", s); process::exit(1); }
            Err(e) => {
                eprintln!("Failed to run 'dot': {}", e);
                eprintln!("Install Graphviz: sudo apt install graphviz");
                process::exit(1);
            }
        }
    }
}

/// Derive the `.j0` output path from the source path.
/// `tests/hello.java` → `tests/hello.j0`
fn j0_path(source: &str) -> String {
    if let Some(stem) = source.strip_suffix(".java") {
        format!("{}.j0", stem)
    } else {
        format!("{}.j0", source)
    }
}