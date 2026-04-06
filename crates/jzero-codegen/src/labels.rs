//! Phase 3 — Label annotation passes.
//!
//! Three passes over the syntax tree, in order:
//!
//! 1. [`genfirst`]    — post-order: synthesize `first` labels (entry points)
//! 2. [`genfollow`]   — pre-order:  inherit `follow` labels (exit points)
//! 3. [`gentargets`]  — pre-order:  inherit `on_true`/`on_false` for Booleans
//!
//! All state is stored in [`CodegenContext::node_info`], keyed by `Tree::id`.
//! The AST is not mutated.

use jzero_ast::tree::Tree;
use crate::address::Address;
use crate::context::CodegenContext;

// ═══════════════════════════════════════════════════════════════════════════════
// Pass 1 — genfirst (post-order, synthesized)
// ═══════════════════════════════════════════════════════════════════════════════

/// Assign a `first` label to every node that generates code.
///
/// For most expression operators, `first` is inherited from the leftmost
/// operand child that already has one.  If no child has one, a new label
/// is minted (the node itself will emit the first instruction).
/// For nodes higher up the tree that don't generate instructions themselves,
/// `first` propagates up from any child that has one.
pub fn genfirst(tree: &Tree, ctx: &mut CodegenContext) {
    // Post-order: recurse first.
    for kid in &tree.kids {
        genfirst(kid, ctx);
    }

    let first: Option<Address> = match tree.sym.as_str() {
        // ── Arithmetic / relational — first comes from lhs operand (kids[0]),
        //    then rhs operand (kids[2], skipping the operator leaf kids[1]),
        //    then mint a new label because this node will emit an instruction.
        "AddExpr" | "MulExpr" | "RelExpr" | "EqExpr" => {
            Some(first_from_kid(tree, ctx, 0)
                .or_else(|| first_from_kid(tree, ctx, 2))
                .unwrap_or_else(|| ctx.genlabel()))
        }

        // ── Boolean short-circuit — same shape.
        "CondAndExpr" | "CondOrExpr" => {
            Some(first_from_kid(tree, ctx, 0)
                .or_else(|| first_from_kid(tree, ctx, 2))
                .unwrap_or_else(|| ctx.genlabel()))
        }

        // ── Unary — operand is kids[0].
        "UnaryMinus" | "UnaryNot" => {
            Some(first_from_kid(tree, ctx, 0)
                .unwrap_or_else(|| ctx.genlabel()))
        }

        // ── Assignment — lhs is kids[0], rhs is kids[2].
        "Assignment" => {
            Some(first_from_kid(tree, ctx, 0)
                .or_else(|| first_from_kid(tree, ctx, 2))
                .unwrap_or_else(|| ctx.genlabel()))
        }

        // ── Control flow — first comes from the condition (kids[0]).
        "WhileStmt" | "IfThenStmt" | "IfThenElseStmt" => {
            Some(first_from_kid(tree, ctx, 0)
                .unwrap_or_else(|| ctx.genlabel()))
        }

        // ── For loop — first comes from init (kids[0]).
        "ForStmt" => {
            Some(first_from_kid(tree, ctx, 0)
                .unwrap_or_else(|| ctx.genlabel()))
        }

        // ── Return — first comes from the expression, if present.
        "ReturnStmt" => {
            Some(first_from_kid(tree, ctx, 0)
                .unwrap_or_else(|| ctx.genlabel()))
        }

        // ── Method call — first from the name/base (kids[0]).
        "MethodCall" => {
            Some(first_from_kid(tree, ctx, 0)
                .unwrap_or_else(|| ctx.genlabel()))
        }

        // ── Array/instance creation — always emits an instruction.
        "ArrayCreation" | "InstanceCreation" | "ArrayAccess" => {
            Some(ctx.genlabel())
        }

        // ── Leaves that carry values — they generate no instructions,
        //    so they get no first label (None propagates up).
        _ if tree.is_leaf() => None,

        // ── Everything else (Block, ClassDecl, MethodDecl, etc.) —
        //    propagate first from any child that has one.
        _ => {
            tree.kids.iter().find_map(|k| first_from_kid_by_id(k, ctx))
        }
    };

    if let Some(addr) = first {
        ctx.node_mut(tree.id).first = Some(addr);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Pass 2 — genfollow (pre-order, inherited)
// ═══════════════════════════════════════════════════════════════════════════════

/// Propagate `follow` labels down the tree.
///
/// The `follow` of a node is whatever label should be jumped to after
/// all of that node's code has executed.  It is an inherited attribute:
/// parents set it on children before recursing.
///
/// Call this after [`genfirst`] has run on the whole tree.
pub fn genfollow(tree: &Tree, ctx: &mut CodegenContext) {
    let my_follow = ctx.node(tree.id).and_then(|n| n.follow.clone());

    match tree.sym.as_str() {
        // ── Block: sequence of statements.
        //    kids[i].follow = kids[i+1].first
        //    kids[last].follow = our follow
        "Block" => {
            let stmts = &tree.kids;
            let n = stmts.len();
            for i in 0..n {
                let follow = if i + 1 < n {
                    // next sibling's first
                    ctx.node(stmts[i + 1].id).and_then(|n| n.first.clone())
                } else {
                    // last stmt inherits our follow
                    my_follow.clone()
                };
                if let Some(f) = follow {
                    ctx.node_mut(stmts[i].id).follow = Some(f);
                }
            }
        }

        // ── WhileStmt: kids = [cond, body]
        //    cond.follow = our follow (if cond is false → exit)
        //    body.follow = cond.first (loop back)
        "WhileStmt" if tree.kids.len() == 2 => {
            let cond_first = ctx.node(tree.kids[0].id).and_then(|n| n.first.clone());
            // cond's follow is the loop exit (our follow)
            if let Some(f) = my_follow.clone() {
                ctx.node_mut(tree.kids[0].id).follow = Some(f);
            }
            // body's follow loops back to cond
            if let Some(f) = cond_first {
                ctx.node_mut(tree.kids[1].id).follow = Some(f);
            }
        }

        // ── IfThenStmt: kids = [cond, body]
        //    cond.follow = our follow
        //    body.follow = our follow
        "IfThenStmt" if tree.kids.len() == 2 => {
            if let Some(f) = my_follow.clone() {
                ctx.node_mut(tree.kids[0].id).follow = Some(f.clone());
                ctx.node_mut(tree.kids[1].id).follow = Some(f);
            }
        }

        // ── IfThenElseStmt: kids = [cond, then_body, else_body]
        //    cond.follow = our follow
        //    then.follow = our follow
        //    else.follow = our follow
        "IfThenElseStmt" if tree.kids.len() == 3 => {
            if let Some(f) = my_follow.clone() {
                ctx.node_mut(tree.kids[0].id).follow = Some(f.clone());
                ctx.node_mut(tree.kids[1].id).follow = Some(f.clone());
                ctx.node_mut(tree.kids[2].id).follow = Some(f);
            }
        }

        // ── ForStmt: kids = [init, cond, update, body]
        //    init.follow  = cond.first
        //    cond.follow  = our follow (exit)
        //    body.follow  = update.first (or cond.first if no update)
        //    update.follow = cond.first (loop back)
        "ForStmt" if tree.kids.len() == 4 => {
            let cond_first  = ctx.node(tree.kids[1].id).and_then(|n| n.first.clone());
            let upd_first   = ctx.node(tree.kids[2].id).and_then(|n| n.first.clone());

            if let Some(cf) = cond_first.clone() {
                ctx.node_mut(tree.kids[0].id).follow = Some(cf.clone()); // init → cond
                ctx.node_mut(tree.kids[3].id).follow =                   // body → update or cond
                    Some(upd_first.clone().unwrap_or(cf));
            }
            if let Some(f) = my_follow.clone() {
                ctx.node_mut(tree.kids[1].id).follow = Some(f);          // cond → exit
            }
            if let Some(cf) = cond_first {
                ctx.node_mut(tree.kids[2].id).follow = Some(cf);         // update → cond
            }
        }

        // ── AddExpr/MulExpr: kids = [lhs, op_leaf, rhs]
        //    both operands inherit our follow.
        "AddExpr" | "MulExpr" | "RelExpr" | "EqExpr"
        | "CondAndExpr" | "CondOrExpr" => {
            if let Some(f) = my_follow.clone() {
                if tree.kids.len() >= 1 {
                    ctx.node_mut(tree.kids[0].id).follow = Some(f.clone());
                }
                if tree.kids.len() >= 3 {
                    ctx.node_mut(tree.kids[2].id).follow = Some(f);
                }
            }
        }

        // ── Default: propagate our follow to all children.
        _ => {
            if let Some(f) = my_follow.clone() {
                for kid in &tree.kids {
                    ctx.node_mut(kid.id).follow = Some(f.clone());
                }
            }
        }
    }

    // Recurse pre-order (children already have their follow set above).
    for kid in &tree.kids {
        genfollow(kid, ctx);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Pass 3 — gentargets (pre-order, inherited)
// ═══════════════════════════════════════════════════════════════════════════════

/// Assign `on_true` and `on_false` labels to Boolean expressions.
///
/// These are inherited from control-flow parents down into conditions.
/// Call this after [`genfollow`] has run.
pub fn gentargets(tree: &Tree, ctx: &mut CodegenContext) {
    let my_on_true  = ctx.node(tree.id).and_then(|n| n.on_true.clone());
    let my_on_false = ctx.node(tree.id).and_then(|n| n.on_false.clone());
    let my_follow   = ctx.node(tree.id).and_then(|n| n.follow.clone());

    match tree.sym.as_str() {
        // ── IfThenStmt: kids = [cond, body]
        //    cond.on_true  = body.first  (jump into then-branch)
        //    cond.on_false = our follow  (skip then-branch)
        "IfThenStmt" if tree.kids.len() == 2 => {
            let then_first = ctx.node(tree.kids[1].id).and_then(|n| n.first.clone());
            if let Some(t) = then_first {
                ctx.node_mut(tree.kids[0].id).on_true = Some(t);
            }
            if let Some(f) = my_follow.clone() {
                ctx.node_mut(tree.kids[0].id).on_false = Some(f);
            }
        }

        // ── IfThenElseStmt: kids = [cond, then_body, else_body]
        //    cond.on_true  = then.first
        //    cond.on_false = else.first
        "IfThenElseStmt" if tree.kids.len() == 3 => {
            let then_first = ctx.node(tree.kids[1].id).and_then(|n| n.first.clone());
            let else_first = ctx.node(tree.kids[2].id).and_then(|n| n.first.clone());
            if let Some(t) = then_first {
                ctx.node_mut(tree.kids[0].id).on_true = Some(t);
            }
            if let Some(f) = else_first {
                ctx.node_mut(tree.kids[0].id).on_false = Some(f);
            }
        }

        // ── WhileStmt: kids = [cond, body]
        //    cond.on_true  = body.first  (enter loop body)
        //    cond.on_false = our follow  (exit loop)
        "WhileStmt" if tree.kids.len() == 2 => {
            let body_first = ctx.node(tree.kids[1].id).and_then(|n| n.first.clone());
            if let Some(t) = body_first {
                ctx.node_mut(tree.kids[0].id).on_true = Some(t);
            }
            if let Some(f) = my_follow.clone() {
                ctx.node_mut(tree.kids[0].id).on_false = Some(f);
            }
        }

        // ── ForStmt: kids = [init, cond, update, body]
        //    cond.on_true  = body.first
        //    cond.on_false = our follow
        "ForStmt" if tree.kids.len() == 4 => {
            let body_first = ctx.node(tree.kids[3].id).and_then(|n| n.first.clone());
            if let Some(t) = body_first {
                ctx.node_mut(tree.kids[1].id).on_true = Some(t);
            }
            if let Some(f) = my_follow.clone() {
                ctx.node_mut(tree.kids[1].id).on_false = Some(f);
            }
        }

        // ── CondAndExpr: kids = [lhs, op_leaf, rhs]
        //    lhs.on_true  = rhs.first   (lhs true → evaluate rhs)
        //    lhs.on_false = our on_false (short-circuit)
        //    rhs.on_true  = our on_true
        //    rhs.on_false = our on_false
        "CondAndExpr" if tree.kids.len() >= 3 => {
            let rhs_first = ctx.node(tree.kids[2].id).and_then(|n| n.first.clone());
            if let Some(t) = rhs_first {
                ctx.node_mut(tree.kids[0].id).on_true = Some(t);
            }
            if let Some(f) = my_on_false.clone() {
                ctx.node_mut(tree.kids[0].id).on_false = Some(f.clone());
                ctx.node_mut(tree.kids[2].id).on_false = Some(f);
            }
            if let Some(t) = my_on_true.clone() {
                ctx.node_mut(tree.kids[2].id).on_true = Some(t);
            }
        }

        // ── CondOrExpr: kids = [lhs, op_leaf, rhs]
        //    lhs.on_true  = our on_true  (short-circuit)
        //    lhs.on_false = rhs.first    (lhs false → evaluate rhs)
        //    rhs.on_true  = our on_true
        //    rhs.on_false = our on_false
        "CondOrExpr" if tree.kids.len() >= 3 => {
            let rhs_first = ctx.node(tree.kids[2].id).and_then(|n| n.first.clone());
            if let Some(t) = my_on_true.clone() {
                ctx.node_mut(tree.kids[0].id).on_true = Some(t.clone());
                ctx.node_mut(tree.kids[2].id).on_true = Some(t);
            }
            if let Some(f) = rhs_first {
                ctx.node_mut(tree.kids[0].id).on_false = Some(f);
            }
            if let Some(f) = my_on_false.clone() {
                ctx.node_mut(tree.kids[2].id).on_false = Some(f);
            }
        }

        // ── Default: propagate on_true/on_false to all children.
        _ => {
            for kid in &tree.kids {
                if let Some(t) = my_on_true.clone() {
                    ctx.node_mut(kid.id).on_true = Some(t);
                }
                if let Some(f) = my_on_false.clone() {
                    ctx.node_mut(kid.id).on_false = Some(f);
                }
            }
        }
    }

    // Recurse pre-order.
    for kid in &tree.kids {
        gentargets(kid, ctx);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════════

/// Get the `first` of `tree.kids[idx]` if it exists.
fn first_from_kid(tree: &Tree, ctx: &CodegenContext, idx: usize) -> Option<crate::address::Address> {
    let kid = tree.kids.get(idx)?;
    ctx.node(kid.id)?.first.clone()
}

/// Get the `first` of a specific kid node directly.
fn first_from_kid_by_id(kid: &Tree, ctx: &CodegenContext) -> Option<crate::address::Address> {
    ctx.node(kid.id)?.first.clone()
}