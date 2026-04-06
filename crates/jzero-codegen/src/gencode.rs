//! Phase 4 — Intermediate code emission.

use jzero_ast::tree::Tree;
use jzero_symtab::SymTab;

use crate::address::Address;
use crate::context::CodegenContext;
use crate::layout::var_key;
use crate::tac::{Op, Tac};

// ═══════════════════════════════════════════════════════════════════════════════
// Public entry point
// ═══════════════════════════════════════════════════════════════════════════════

pub fn gencode(tree: &Tree, ctx: &mut CodegenContext) {
    // Special case: MethodCall where kids[0] is a FieldAccess chain.
    // We must NOT recurse into kids[0] — it's the method name, not a value
    // to load. Instead we handle recursion manually inside gen_method_call_field.
    if tree.sym == "MethodCall" {
        let name_is_field_access = !tree.kids.is_empty()
            && !tree.kids[0].is_leaf()
            && tree.kids[0].sym == "FieldAccess";
        if name_is_field_access {
            gen_method_call_field(tree, ctx);
            return;
        }
    }

    // Normal post-order: children first.
    for kid in &tree.kids {
        gencode(kid, ctx);
    }

    match tree.sym.as_str() {
        _ if tree.is_leaf()    => gen_leaf(tree, ctx),
        "AddExpr"              => gen_add_expr(tree, ctx),
        "MulExpr"              => gen_mul_expr(tree, ctx),
        "UnaryMinus"           => gen_unary_minus(tree, ctx),
        "UnaryNot"             => gen_unary_not(tree, ctx),
        "RelExpr"              => gen_rel_expr(tree, ctx),
        "EqExpr"               => gen_eq_expr(tree, ctx),
        "CondAndExpr"          => gen_cond_and(tree, ctx),
        "CondOrExpr"           => gen_cond_or(tree, ctx),
        "Assignment"           => gen_assignment(tree, ctx),
        "ArrayAccess"          => gen_array_access(tree, ctx),
        "ArrayCreation"        => gen_array_creation(tree, ctx),
        "InstanceCreation"     => gen_instance_creation(tree, ctx),
        "MethodCall"           => gen_method_call(tree, ctx),
        "FieldAccess"          => gen_field_access(tree, ctx),
        "ReturnStmt"           => gen_return(tree, ctx),
        "IfThenStmt"           => gen_if_then(tree, ctx),
        "IfThenElseStmt"       => gen_if_then_else(tree, ctx),
        "WhileStmt"            => gen_while(tree, ctx),
        "ForStmt"              => gen_for(tree, ctx),
        "BreakStmt"            => gen_break(tree, ctx),
        _                      => default_concat(tree, ctx),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Leaves
// ═══════════════════════════════════════════════════════════════════════════════

fn gen_leaf(tree: &Tree, ctx: &mut CodegenContext) {
    let tok = match &tree.tok { Some(t) => t, None => return };
    let addr = match tok.category.as_str() {
        "INTLIT"     => { let v: i64 = tok.text.parse().unwrap_or(0); Some(Address::imm(v)) }
        "DOUBLELIT"  => Some(ctx.intern_string(&tok.text)),
        "BOOLLIT"    => Some(Address::imm(if tok.text == "true" { 1 } else { 0 })),
        "STRINGLIT"  => { let raw = tok.text.trim_matches('"'); Some(ctx.intern_string(raw)) }
        "NULL"       => Some(Address::imm(0)),
        "IDENTIFIER" => lookup_var(tree, ctx),
        _            => None,
    };
    let info = ctx.node_mut(tree.id);
    info.icode = vec![];
    info.addr  = addr;
}

// ═══════════════════════════════════════════════════════════════════════════════
// Arithmetic
// ═══════════════════════════════════════════════════════════════════════════════

fn gen_add_expr(tree: &Tree, ctx: &mut CodegenContext) {
    if tree.kids.len() < 3 { return default_concat(tree, ctx); }
    let dst = ctx.genlocal();
    let lhs = addr_of(&tree.kids[0], ctx);
    let rhs = addr_of(&tree.kids[2], ctx);
    let op  = if tree.rule == 0 { Op::Add } else { Op::Sub };
    let mut icode = concat_kids_icode(tree, ctx);
    icode.push(Tac::new3(op, dst.clone(), lhs, rhs));
    let info = ctx.node_mut(tree.id);
    info.icode = icode; info.addr = Some(dst);
}

fn gen_mul_expr(tree: &Tree, ctx: &mut CodegenContext) {
    if tree.kids.len() < 3 { return default_concat(tree, ctx); }
    let dst = ctx.genlocal();
    let lhs = addr_of(&tree.kids[0], ctx);
    let rhs = addr_of(&tree.kids[2], ctx);
    let op  = match tree.rule { 0 => Op::Mul, 1 => Op::Div, _ => Op::Mod };
    let mut icode = concat_kids_icode(tree, ctx);
    icode.push(Tac::new3(op, dst.clone(), lhs, rhs));
    let info = ctx.node_mut(tree.id);
    info.icode = icode; info.addr = Some(dst);
}

fn gen_unary_minus(tree: &Tree, ctx: &mut CodegenContext) {
    if tree.kids.is_empty() { return default_concat(tree, ctx); }
    let dst     = ctx.genlocal();
    let operand = addr_of(&tree.kids[0], ctx);
    let mut icode = concat_kids_icode(tree, ctx);
    icode.push(Tac::new2(Op::Neg, dst.clone(), operand));
    let info = ctx.node_mut(tree.id);
    info.icode = icode; info.addr = Some(dst);
}

fn gen_unary_not(tree: &Tree, ctx: &mut CodegenContext) {
    if tree.kids.is_empty() { return default_concat(tree, ctx); }
    let dst     = ctx.genlocal();
    let operand = addr_of(&tree.kids[0], ctx);
    let mut icode = concat_kids_icode(tree, ctx);
    icode.push(Tac::new3(Op::Sub, dst.clone(), Address::imm(1), operand));
    let info = ctx.node_mut(tree.id);
    info.icode = icode; info.addr = Some(dst);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Relational / equality
// ═══════════════════════════════════════════════════════════════════════════════

fn gen_rel_expr(tree: &Tree, ctx: &mut CodegenContext) {
    if tree.kids.len() < 3 { return default_concat(tree, ctx); }
    let op_cat = tree.kids[1].tok.as_ref().map(|t| t.category.as_str()).unwrap_or("");
    let branch_op = match op_cat {
        "LESS"         => Op::Blt,
        "LESSEQUAL"    => Op::Ble,
        "GREATER"      => Op::Bgt,
        "GREATEREQUAL" => Op::Bge,
        _              => Op::Blt,
    };
    emit_condition(tree, branch_op, ctx);
}

fn gen_eq_expr(tree: &Tree, ctx: &mut CodegenContext) {
    if tree.kids.len() < 3 { return default_concat(tree, ctx); }
    let branch_op = if tree.rule == 0 { Op::Beq } else { Op::Bne };
    emit_condition(tree, branch_op, ctx);
}

fn emit_condition(tree: &Tree, branch_op: Op, ctx: &mut CodegenContext) {
    let lhs      = addr_of(&tree.kids[0], ctx);
    let rhs      = addr_of(&tree.kids[2], ctx);
    let on_true  = ctx.node(tree.id).and_then(|n| n.on_true.clone());
    let on_false = ctx.node(tree.id).and_then(|n| n.on_false.clone());
    let mut icode = concat_kids_icode(tree, ctx);
    if let Some(t) = on_true  { icode.push(Tac::new3(branch_op, t, lhs, rhs)); }
    if let Some(f) = on_false { icode.push(Tac::new1(Op::Goto, f)); }
    ctx.node_mut(tree.id).icode = icode;
}

// ═══════════════════════════════════════════════════════════════════════════════
// Boolean short-circuit
// ═══════════════════════════════════════════════════════════════════════════════

fn gen_cond_and(tree: &Tree, ctx: &mut CodegenContext) {
    if tree.kids.len() < 3 { return default_concat(tree, ctx); }
    ctx.node_mut(tree.id).icode = concat_kids_icode(tree, ctx);
}

fn gen_cond_or(tree: &Tree, ctx: &mut CodegenContext) {
    if tree.kids.len() < 3 { return default_concat(tree, ctx); }
    ctx.node_mut(tree.id).icode = concat_kids_icode(tree, ctx);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Assignment
// ═══════════════════════════════════════════════════════════════════════════════

fn gen_assignment(tree: &Tree, ctx: &mut CodegenContext) {
    if tree.kids.len() < 3 { return default_concat(tree, ctx); }
    let op_cat   = tree.kids[1].tok.as_ref().map(|t| t.category.as_str()).unwrap_or("ASSIGN");
    let lhs_addr = addr_of(&tree.kids[0], ctx);
    let rhs_addr = addr_of(&tree.kids[2], ctx);
    let mut icode = concat_kids_icode(tree, ctx);
    match op_cat {
        "ASSIGN" => {
            icode.push(Tac::new2(Op::Asn, lhs_addr.clone(), rhs_addr));
        }
        "PLUSASSIGN" => {
            let tmp = ctx.genlocal();
            icode.push(Tac::new3(Op::Add, tmp.clone(), lhs_addr.clone(), rhs_addr));
            icode.push(Tac::new2(Op::Asn, lhs_addr.clone(), tmp));
        }
        "MINUSASSIGN" => {
            let tmp = ctx.genlocal();
            icode.push(Tac::new3(Op::Sub, tmp.clone(), lhs_addr.clone(), rhs_addr));
            icode.push(Tac::new2(Op::Asn, lhs_addr.clone(), tmp));
        }
        _ => { icode.push(Tac::new2(Op::Asn, lhs_addr.clone(), rhs_addr)); }
    }
    let info = ctx.node_mut(tree.id);
    info.icode = icode; info.addr = Some(lhs_addr);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Arrays
// ═══════════════════════════════════════════════════════════════════════════════

fn gen_array_creation(tree: &Tree, ctx: &mut CodegenContext) {
    if tree.kids.len() < 2 { return default_concat(tree, ctx); }
    let dst  = ctx.genlocal();
    let size = addr_of(&tree.kids[1], ctx);
    let mut icode = concat_kids_icode(tree, ctx);
    icode.push(Tac::new2(Op::NewArray, dst.clone(), size));
    let info = ctx.node_mut(tree.id);
    info.icode = icode; info.addr = Some(dst);
}

fn gen_array_access(tree: &Tree, ctx: &mut CodegenContext) {
    if tree.kids.len() < 2 { return default_concat(tree, ctx); }
    let dst   = ctx.genlocal();
    let base  = addr_of(&tree.kids[0], ctx);
    let index = addr_of(&tree.kids[1], ctx);
    let mut icode = concat_kids_icode(tree, ctx);
    icode.push(Tac::new3(Op::Load, dst.clone(), base, index));
    let info = ctx.node_mut(tree.id);
    info.icode = icode; info.addr = Some(dst);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Instance creation
// ═══════════════════════════════════════════════════════════════════════════════

fn gen_instance_creation(tree: &Tree, ctx: &mut CodegenContext) {
    if tree.kids.is_empty() { return default_concat(tree, ctx); }
    let dst        = ctx.genlocal();
    let class_addr = addr_of(&tree.kids[0], ctx);
    let n_args     = (tree.kids.len() - 1) as i64;
    let mut icode  = concat_kids_icode(tree, ctx);
    for kid in tree.kids[1..].iter().rev() {
        if let Some(a) = ctx.node(kid.id).and_then(|n| n.addr.clone()) {
            icode.push(Tac::new1(Op::Parm, a));
        }
    }
    icode.push(Tac::new1(Op::Parm, Address::self_ptr()));
    icode.push(Tac::new2(Op::Call, class_addr, Address::imm(n_args)));
    icode.push(Tac::new2(Op::Asn, dst.clone(), Address::imm(0)));
    let info = ctx.node_mut(tree.id);
    info.icode = icode; info.addr = Some(dst);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Method calls
// ═══════════════════════════════════════════════════════════════════════════════

/// Handle MethodCall where kids[0] is a FieldAccess chain (e.g. System.out.println).
/// We recurse manually — into the receiver base and args, but NOT into the
/// full kids[0] FieldAccess chain (which would emit spurious LOADs).
fn gen_method_call_field(tree: &Tree, ctx: &mut CodegenContext) {
    let fa = &tree.kids[0];
    let (base_chain, method_name) = split_field_chain(fa);
    let mangled = mangle_method(&base_chain, &method_name);

    // Recurse into args only — no recursion into the method chain at all.
    let args_start = 1usize;
    for kid in &tree.kids[args_start..] {
        gencode(kid, ctx);
    }

    let n_args = (tree.kids.len() - args_start) as i64;
    let dst    = ctx.genlocal();
    let mut icode = vec![];

    // Arg icode.
    for kid in &tree.kids[args_start..] {
        icode.extend(take_icode(kid, ctx));
    }
    // Push args in reverse order.
    for kid in tree.kids[args_start..].iter().rev() {
        icode.push(Tac::new1(Op::Parm, addr_of(kid, ctx)));
    }
    // Receiver: look up the base identifier's address directly from stab.
    // For System.out.println, base_chain = ["System", "out"],
    // so the object is the System global — look up "System" in the stab.
    let receiver_addr = base_chain.first()
        .and_then(|base_name| {
            // Find the IDENTIFIER leaf for the base in the FieldAccess chain.
            find_base_leaf(fa)
                .and_then(|leaf| find_in_chain(leaf.stab.as_ref()?, base_name, ctx))
        })
        .unwrap_or_else(Address::self_ptr);
    icode.push(Tac::new1(Op::Parm, receiver_addr));
    icode.push(Tac::new2(Op::Call, Address::symbol(&mangled), Address::imm(n_args)));

    let info = ctx.node_mut(tree.id);
    info.icode = icode;
    info.addr  = Some(dst);
}

/// Handle direct MethodCall (kids[0] is a plain IDENTIFIER or rule >= 2).
fn gen_method_call(tree: &Tree, ctx: &mut CodegenContext) {
    if tree.kids.is_empty() { return default_concat(tree, ctx); }
    let dst = ctx.genlocal();
    let mut icode = vec![];

    if tree.rule >= 2 {
        // Explicit dotted call: kids = [base_expr, method_leaf, args...]
        let method_name = tree.kids[1].tok.as_ref()
            .map(|t| t.text.as_str()).unwrap_or("unknown");
        let base_chain = collect_field_chain(&tree.kids[0]);
        let mangled    = mangle_method(&base_chain, method_name);
        let args_start = 2usize;
        let n_args     = (tree.kids.len() - args_start) as i64;
        for kid in &tree.kids[args_start..] {
            icode.extend(take_icode(kid, ctx));
        }
        for kid in tree.kids[args_start..].iter().rev() {
            icode.push(Tac::new1(Op::Parm, addr_of(kid, ctx)));
        }
        let obj_addr = ctx.node(tree.kids[0].id)
            .and_then(|n| n.addr.clone())
            .unwrap_or_else(Address::self_ptr);
        icode.push(Tac::new1(Op::Parm, obj_addr));
        icode.push(Tac::new2(Op::Call, Address::symbol(&mangled), Address::imm(n_args)));
    } else {
        // Direct call: kids[0] = method name leaf, kids[1..] = args.
        let method_addr = addr_of(&tree.kids[0], ctx);
        let args_start  = 1usize;
        let n_args      = (tree.kids.len() - args_start) as i64;
        for kid in &tree.kids[args_start..] {
            icode.extend(take_icode(kid, ctx));
        }
        for kid in tree.kids[args_start..].iter().rev() {
            icode.push(Tac::new1(Op::Parm, addr_of(kid, ctx)));
        }
        icode.push(Tac::new1(Op::Parm, Address::self_ptr()));
        icode.push(Tac::new2(Op::Call, method_addr, Address::imm(n_args)));
    }

    let info = ctx.node_mut(tree.id);
    info.icode = icode; info.addr = Some(dst);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Field access
// ═══════════════════════════════════════════════════════════════════════════════

fn gen_field_access(tree: &Tree, ctx: &mut CodegenContext) {
    if tree.kids.len() < 2 { return default_concat(tree, ctx); }
    let dst       = ctx.genlocal();
    let base_addr = addr_of(&tree.kids[0], ctx);
    if tree.kids[1].tok.as_ref().map(|t| t.text.as_str()) == Some("length") {
        let mut icode = concat_kids_icode(tree, ctx);
        icode.push(Tac::new2(Op::Asize, dst.clone(), base_addr));
        let info = ctx.node_mut(tree.id);
        info.icode = icode; info.addr = Some(dst);
        return;
    }
    let field_addr = addr_of(&tree.kids[1], ctx);
    let mut icode  = concat_kids_icode(tree, ctx);
    icode.push(Tac::new3(Op::Load, dst.clone(), base_addr, field_addr));
    let info = ctx.node_mut(tree.id);
    info.icode = icode; info.addr = Some(dst);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Control flow
// ═══════════════════════════════════════════════════════════════════════════════

fn gen_return(tree: &Tree, ctx: &mut CodegenContext) {
    let mut icode = concat_kids_icode(tree, ctx);
    if tree.rule == 0 && !tree.kids.is_empty() {
        let val = addr_of(&tree.kids[0], ctx);
        icode.push(Tac::new1(Op::Ret, val));
    } else {
        icode.push(Tac::new0(Op::Ret));
    }
    ctx.node_mut(tree.id).icode = icode;
}

fn gen_if_then(tree: &Tree, ctx: &mut CodegenContext) {
    if tree.kids.len() < 2 { return default_concat(tree, ctx); }
    let cond_first = ctx.node(tree.kids[0].id).and_then(|n| n.first.clone());
    let follow     = ctx.node(tree.id).and_then(|n| n.follow.clone());
    let mut icode  = vec![];
    if let Some(f) = cond_first { icode.push(Tac::new1(Op::Lab, f)); }
    icode.extend(take_icode(&tree.kids[0], ctx));
    icode.extend(take_icode(&tree.kids[1], ctx));
    if let Some(f) = follow     { icode.push(Tac::new1(Op::Lab, f)); }
    ctx.node_mut(tree.id).icode = icode;
}

fn gen_if_then_else(tree: &Tree, ctx: &mut CodegenContext) {
    if tree.kids.len() < 3 { return default_concat(tree, ctx); }
    let cond_first = ctx.node(tree.kids[0].id).and_then(|n| n.first.clone());
    let else_first = ctx.node(tree.kids[2].id).and_then(|n| n.first.clone());
    let follow     = ctx.node(tree.id).and_then(|n| n.follow.clone());
    let mut icode  = vec![];
    if let Some(f) = cond_first     { icode.push(Tac::new1(Op::Lab, f)); }
    icode.extend(take_icode(&tree.kids[0], ctx));
    icode.extend(take_icode(&tree.kids[1], ctx));
    if let Some(f) = follow.clone() { icode.push(Tac::new1(Op::Goto, f)); }
    if let Some(f) = else_first     { icode.push(Tac::new1(Op::Lab, f)); }
    icode.extend(take_icode(&tree.kids[2], ctx));
    if let Some(f) = follow         { icode.push(Tac::new1(Op::Lab, f)); }
    ctx.node_mut(tree.id).icode = icode;
}

fn gen_while(tree: &Tree, ctx: &mut CodegenContext) {
    if tree.kids.len() < 2 { return default_concat(tree, ctx); }
    let cond_first = ctx.node(tree.kids[0].id).and_then(|n| n.first.clone());
    let on_true    = ctx.node(tree.kids[0].id).and_then(|n| n.on_true.clone());
    let follow     = ctx.node(tree.id)
        .and_then(|n| n.follow.clone())
        .unwrap_or_else(|| ctx.genlabel());
    ctx.node_mut(tree.kids[0].id).on_false = Some(follow.clone());
    reemit_condition(&tree.kids[0], ctx);

    let mut icode = vec![];
    if let Some(f) = cond_first.clone() { icode.push(Tac::new1(Op::Lab, f)); }
    icode.extend(take_icode(&tree.kids[0], ctx));
    if let Some(t) = on_true            { icode.push(Tac::new1(Op::Lab, t)); }
    icode.extend(take_icode(&tree.kids[1], ctx));
    if let Some(f) = cond_first         { icode.push(Tac::new1(Op::Goto, f)); }
    icode.push(Tac::new1(Op::Lab, follow));
    ctx.node_mut(tree.id).icode = icode;
}

fn gen_for(tree: &Tree, ctx: &mut CodegenContext) {
    if tree.kids.len() < 4 { return default_concat(tree, ctx); }
    let cond_first = ctx.node(tree.kids[1].id).and_then(|n| n.first.clone());
    let on_true    = ctx.node(tree.kids[1].id).and_then(|n| n.on_true.clone());
    let follow     = ctx.node(tree.id).and_then(|n| n.follow.clone());
    let mut icode  = vec![];
    icode.extend(take_icode(&tree.kids[0], ctx));
    if let Some(f) = cond_first.clone() { icode.push(Tac::new1(Op::Lab, f)); }
    icode.extend(take_icode(&tree.kids[1], ctx));
    if let Some(t) = on_true            { icode.push(Tac::new1(Op::Lab, t)); }
    icode.extend(take_icode(&tree.kids[3], ctx));
    icode.extend(take_icode(&tree.kids[2], ctx));
    if let Some(f) = cond_first         { icode.push(Tac::new1(Op::Goto, f)); }
    if let Some(f) = follow             { icode.push(Tac::new1(Op::Lab, f)); }
    ctx.node_mut(tree.id).icode = icode;
}

fn gen_break(tree: &Tree, ctx: &mut CodegenContext) {
    let follow = ctx.node(tree.id).and_then(|n| n.follow.clone());
    let mut icode = vec![];
    if let Some(f) = follow { icode.push(Tac::new1(Op::Goto, f)); }
    ctx.node_mut(tree.id).icode = icode;
}

// ═══════════════════════════════════════════════════════════════════════════════
// Helpers — general
// ═══════════════════════════════════════════════════════════════════════════════

fn addr_of(tree: &Tree, ctx: &CodegenContext) -> Address {
    ctx.node(tree.id)
        .and_then(|n| n.addr.clone())
        .unwrap_or_else(|| Address::imm(0))
}

fn concat_kids_icode(tree: &Tree, ctx: &CodegenContext) -> Vec<Tac> {
    let mut out = vec![];
    for kid in &tree.kids {
        if let Some(info) = ctx.node(kid.id) {
            out.extend(info.icode.iter().cloned());
        }
    }
    out
}

fn take_icode(tree: &Tree, ctx: &CodegenContext) -> Vec<Tac> {
    ctx.node(tree.id).map(|n| n.icode.clone()).unwrap_or_default()
}

fn default_concat(tree: &Tree, ctx: &mut CodegenContext) {
    ctx.node_mut(tree.id).icode = concat_kids_icode(tree, ctx);
}

fn reemit_condition(tree: &Tree, ctx: &mut CodegenContext) {
    if tree.kids.len() < 3 { return; }
    let branch_op = match tree.sym.as_str() {
        "RelExpr" => {
            let cat = tree.kids[1].tok.as_ref().map(|t| t.category.as_str()).unwrap_or("");
            match cat {
                "LESS"         => Op::Blt,
                "LESSEQUAL"    => Op::Ble,
                "GREATER"      => Op::Bgt,
                "GREATEREQUAL" => Op::Bge,
                _              => Op::Blt,
            }
        }
        "EqExpr" => if tree.rule == 0 { Op::Beq } else { Op::Bne },
        _ => return,
    };
    emit_condition(tree, branch_op, ctx);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Helpers — method name mangling
// ═══════════════════════════════════════════════════════════════════════════════

/// Split a FieldAccess chain into (base_chain, method_name).
/// FieldAccess(FieldAccess(System, out), println) → (["System", "out"], "println")
fn split_field_chain(tree: &Tree) -> (Vec<String>, String) {
    if tree.sym == "FieldAccess" && tree.kids.len() >= 2 {
        let method = tree.kids[1].tok.as_ref()
            .map(|t| t.text.clone())
            .unwrap_or_default();
        let base = collect_field_chain(&tree.kids[0]);
        return (base, method);
    }
    let name = tree.tok.as_ref().map(|t| t.text.clone()).unwrap_or_default();
    (vec![], name)
}

/// Find the leftmost leaf in a FieldAccess chain (the base identifier).
fn find_base_leaf(tree: &Tree) -> Option<&Tree> {
    if tree.is_leaf() { return Some(tree); }
    if tree.sym == "FieldAccess" && !tree.kids.is_empty() {
        return find_base_leaf(&tree.kids[0]);
    }
    None
}

/// Walk a FieldAccess chain and collect all identifier names.
/// FieldAccess(FieldAccess(System, out), _) → ["System", "out"]
fn collect_field_chain(tree: &Tree) -> Vec<String> {
    if tree.is_leaf() {
        return tree.tok.as_ref()
            .map(|t| vec![t.text.clone()])
            .unwrap_or_default();
    }
    if tree.sym == "FieldAccess" && tree.kids.len() >= 2 {
        let mut chain = collect_field_chain(&tree.kids[0]);
        if let Some(tok) = &tree.kids[1].tok {
            chain.push(tok.text.clone());
        }
        return chain;
    }
    vec![]
}

/// Mangle a dotted call into a C-style symbol name.
fn mangle_method(chain: &[String], method: &str) -> String {
    if chain == ["System", "out"] && method == "println" {
        return "PrintStream__println".to_string();
    }
    chain.last()
        .map(|c| format!("{}__{}", c, method))
        .unwrap_or_else(|| method.to_string())
}

// ═══════════════════════════════════════════════════════════════════════════════
// Helpers — variable lookup
// ═══════════════════════════════════════════════════════════════════════════════

fn lookup_var(tree: &Tree, ctx: &CodegenContext) -> Option<Address> {
    let tok  = tree.tok.as_ref()?;
    let stab = tree.stab.as_ref()?;
    find_in_chain(stab, &tok.text, ctx)
}

fn find_in_chain(
    scope: &std::rc::Rc<std::cell::RefCell<SymTab>>,
    name: &str,
    ctx: &CodegenContext,
) -> Option<Address> {
    if scope.borrow().lookup_local(name).is_some() {
        let key = var_key(scope, name);
        if let Some(addr) = ctx.var_addrs.get(&key) {
            return Some(addr.clone());
        }
    }
    let parent = scope.borrow().parent.clone()?;
    find_in_chain(&parent, name, ctx)
}