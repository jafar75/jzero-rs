//! Phase 5 — Human-readable assembler output.
//!
//! Renders the generated intermediate code in the format shown in the book,
//! matching the golden output for `hello_loop.java`.

use jzero_ast::tree::Tree;
use crate::context::CodegenContext;
use crate::address::Address;
use crate::tac::{Op, Tac};

/// Render the full program output as a string.
pub fn emit(tree: &Tree, ctx: &CodegenContext) -> String {
    let mut out = String::new();

    // ── .string section ───────────────────────────────────────────────────
    if !ctx.strings.is_empty() {
        out.push_str(".string\n");
        for entry in &ctx.strings {
            // Label printed as "L0:" (the lab address), then the string decl.
            out.push_str(&format!("{}:\n", entry.label));
            out.push_str(&format!("string \"{}\"\n", entry.value));
        }
    }

    // ── .global section ───────────────────────────────────────────────────
    if !ctx.globals.is_empty() {
        out.push_str(".global\n");
        for (name, addr) in &ctx.globals {
            // addr is always Regional for globals.
            if let Address::Regional { region, offset } = addr {
                out.push_str(&format!("global {}:{},{}\n", region, offset, name));
            }
        }
    }

    // ── .code section ─────────────────────────────────────────────────────
    out.push_str(".code\n");
    emit_methods(tree, ctx, &mut out);

    out
}

/// Walk the tree looking for MethodDecl nodes and emit each one.
fn emit_methods(tree: &Tree, ctx: &CodegenContext, out: &mut String) {
    if tree.sym == "MethodDecl" {
        emit_method(tree, ctx, out);
        return;
    }
    for kid in &tree.kids {
        emit_methods(kid, ctx, out);
    }
}

/// Emit a single method's proc/end block.
fn emit_method(tree: &Tree, ctx: &CodegenContext, out: &mut String) {
    // Find the method name from the MethodDeclarator → IDENTIFIER leaf.
    let name = find_method_name(tree).unwrap_or_else(|| "unknown".to_string());

    // Count parameters.
    let nparms = count_params(tree);

    // Local frame size = local_offset - 8 (subtract the reserved self slot).
    // We don't track per-method size yet; use 0 for now — the VM can compute it.
    out.push_str(&format!("proc {},0,{}\n", name, nparms));

    // Emit icode from the Block child (kids[1] of MethodDecl).
    if let Some(block) = tree.kids.get(1) {
        if let Some(info) = ctx.node(block.id) {
            for tac in &info.icode {
                out.push_str(&format_tac(tac));
                out.push('\n');
            }
        }
    }

    out.push_str("RET\n");
    out.push_str("end\n");
}

/// Format a single TAC instruction, matching the golden output style.
fn format_tac(tac: &Tac) -> String {
    match &tac.op {
        Op::Lab => {
            // Labels are printed on their own line with a colon: "L138:"
            if let Some(a) = &tac.op1 {
                format!("{}:", a)
            } else {
                String::new()
            }
        }
        Op::Goto => {
            if let Some(a) = &tac.op1 {
                format!("GOTO {}", a)
            } else {
                "GOTO".to_string()
            }
        }
        Op::Ret => {
            if let Some(a) = &tac.op1 {
                format!("RET {}", a)
            } else {
                "RET".to_string()
            }
        }
        Op::Parm => {
            if let Some(a) = &tac.op1 {
                format!("PARM {}", a)
            } else {
                "PARM".to_string()
            }
        }
        Op::Call => {
            match (&tac.op1, &tac.op2) {
                (Some(method), Some(nargs)) => format!("CALL {},{}", method, nargs),
                (Some(method), None)        => format!("CALL {}", method),
                _                           => "CALL".to_string(),
            }
        }
        // Three-operand instructions: OP dst,src1,src2
        Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Mod
        | Op::Load | Op::Store => {
            match (&tac.op1, &tac.op2, &tac.op3) {
                (Some(a), Some(b), Some(c)) => format!("{} {},{},{}", tac.op, a, b, c),
                (Some(a), Some(b), None)    => format!("{} {},{}", tac.op, a, b),
                (Some(a), None, None)       => format!("{} {}", tac.op, a),
                _                           => format!("{}", tac.op),
            }
        }
        // Two-operand instructions: OP dst,src
        Op::Asn | Op::Neg | Op::Asize | Op::NewArray | Op::Addr => {
            match (&tac.op1, &tac.op2) {
                (Some(a), Some(b)) => format!("{} {},{}", tac.op, a, b),
                (Some(a), None)    => format!("{} {}", tac.op, a),
                _                  => format!("{}", tac.op),
            }
        }
        // Conditional branches: Bxx label,src1,src2
        Op::Blt | Op::Ble | Op::Bgt | Op::Bge | Op::Beq | Op::Bne => {
            match (&tac.op1, &tac.op2, &tac.op3) {
                (Some(a), Some(b), Some(c)) => format!("{} {},{},{}", tac.op, a, b, c),
                (Some(a), Some(b), None)    => format!("{} {},{}", tac.op, a, b),
                _                           => format!("{}", tac.op),
            }
        }
        // Pseudo-instructions / declarations — handled by emit sections above.
        _ => format!("{}", tac),
    }
}

// ─── Tree helpers ─────────────────────────────────────────────────────────────

fn find_method_name(tree: &Tree) -> Option<String> {
    if tree.sym == "MethodDeclarator" {
        return tree.kids.first()
            .and_then(|n| n.tok.as_ref())
            .map(|t| t.text.clone());
    }
    tree.kids.iter().find_map(find_method_name)
}

fn count_params(tree: &Tree) -> usize {
    if tree.sym == "FormalParm" {
        return 1;
    }
    tree.kids.iter().map(count_params).sum()
}