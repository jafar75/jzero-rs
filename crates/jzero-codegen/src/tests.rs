#[cfg(test)]
mod tests {
    use jzero_ast::tree::reset_ids;
    use jzero_parser::parse_tree;
    use jzero_semantic::analyze;

    use crate::{emit::emit, generate};

    fn compile(src: &str) -> String {
        reset_ids();
        let mut tree = parse_tree(src).expect("parse failed");
        let sem = analyze(&mut tree);
        let ctx = generate(&tree, &sem);
        emit(&tree, &ctx)
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn has_op(out: &str, op: &str) -> bool {
        out.lines().any(|l| l.trim().starts_with(op))
    }

    fn count_op(out: &str, op: &str) -> usize {
        out.lines().filter(|l| l.trim().starts_with(op)).count()
    }

    fn has_section(out: &str, section: &str) -> bool {
        out.contains(section)
    }

    // ── Section structure ─────────────────────────────────────────────────────

    #[test]
    fn test_sections_present() {
        let out = compile(
            r#"public class hello {
                 public static void main(String argv[]) {
                   System.out.println("hi");
                 }
               }"#,
        );
        assert!(has_section(&out, ".string"),  "missing .string section");
        assert!(has_section(&out, ".global"),  "missing .global section");
        assert!(has_section(&out, ".code"),    "missing .code section");
        assert!(has_section(&out, "proc main"), "missing proc main");
        assert!(has_section(&out, "end"),      "missing end");
    }

    #[test]
    fn test_string_literal_interned() {
        let out = compile(
            r#"public class t {
                 public static void main(String argv[]) {
                   System.out.println("hello, jzero!");
                 }
               }"#,
        );
        assert!(has_section(&out, ".string"));
        assert!(out.contains("string \"hello, jzero!\""));
        // Label for string should appear before the string decl.
        let string_section = out.split(".code").next().unwrap_or("");
        assert!(string_section.contains("L"), "string label missing");
    }

    #[test]
    fn test_string_deduplication() {
        let out = compile(
            r#"public class t {
                 public static void main(String argv[]) {
                   System.out.println("dup");
                   System.out.println("dup");
                 }
               }"#,
        );
        // "dup" should appear exactly once in the string pool.
        assert_eq!(out.matches("string \"dup\"").count(), 1);
    }

    #[test]
    fn test_global_declared() {
        let out = compile(
            r#"public class myclass {
                 public static void main(String argv[]) {}
               }"#,
        );
        assert!(out.contains("myclass"), "class not in globals");
        assert!(has_section(&out, "global global:"));
    }

    // ── Variable layout ───────────────────────────────────────────────────────

    #[test]
    fn test_argv_at_loc8() {
        // argv is the first param → must land at loc:8 (loc:0 reserved for self).
        let out = compile(
            r#"public class t {
                 public static void main(String argv[]) {
                   int x;
                   x = argv.length;
                 }
               }"#,
        );
        // ASIZE dst, loc:8  — argv is the source of the ASIZE
        assert!(out.contains("loc:8"), "argv should be at loc:8");
    }

    #[test]
    fn test_local_var_at_loc16() {
        // First declared local after argv should be at loc:16.
        let out = compile(
            r#"public class t {
                 public static void main(String argv[]) {
                   int x;
                   x = argv.length;
                 }
               }"#,
        );
        // ASN loc:16, ...  — x is the target of the assignment
        assert!(out.contains("loc:16"), "first local should be at loc:16");
    }

    // ── Arithmetic ────────────────────────────────────────────────────────────

    #[test]
    fn test_addition_emits_add() {
        let out = compile(
            r#"public class t {
                 public static void main(String argv[]) {
                   int x;
                   x = x + 2;
                 }
               }"#,
        );
        assert!(has_op(&out, "ADD"), "ADD instruction missing");
    }

    #[test]
    fn test_subtraction_emits_sub() {
        let out = compile(
            r#"public class t {
                 public static void main(String argv[]) {
                   int x;
                   x = x - 1;
                 }
               }"#,
        );
        assert!(has_op(&out, "SUB"), "SUB instruction missing");
    }

    #[test]
    fn test_multiplication_emits_mul() {
        let out = compile(
            r#"public class t {
                 public static void main(String argv[]) {
                   int x;
                   x = x * 3;
                 }
               }"#,
        );
        assert!(has_op(&out, "MUL"), "MUL instruction missing");
    }

    #[test]
    fn test_imm_operand_for_int_literal() {
        let out = compile(
            r#"public class t {
                 public static void main(String argv[]) {
                   int x;
                   x = x + 42;
                 }
               }"#,
        );
        assert!(out.contains("imm:42"), "integer literal should appear as imm:42");
    }

    #[test]
    fn test_assignment_emits_asn() {
        let out = compile(
            r#"public class t {
                 public static void main(String argv[]) {
                   int x;
                   x = 5;
                 }
               }"#,
        );
        assert!(has_op(&out, "ASN"), "ASN instruction missing");
    }

    // ── Array access ──────────────────────────────────────────────────────────

    #[test]
    fn test_array_length_emits_asize() {
        let out = compile(
            r#"public class t {
                 public static void main(String argv[]) {
                   int x;
                   x = argv.length;
                 }
               }"#,
        );
        assert!(has_op(&out, "ASIZE"), "ASIZE instruction missing for .length");
    }

    // ── Control flow — while ─────────────────────────────────────────────────

    #[test]
    fn test_while_emits_loop_structure() {
        let out = compile(
            r#"public class t {
                 public static void main(String argv[]) {
                   int x;
                   x = 5;
                   while (x > 0) { x = x - 1; }
                 }
               }"#,
        );
        // Must have a conditional branch, a GOTO back to top, and a exit label.
        assert!(has_op(&out, "BGT") || has_op(&out, "BLT") ||
                has_op(&out, "BGE") || has_op(&out, "BLE"),
                "while loop missing conditional branch");
        assert!(has_op(&out, "GOTO"), "while loop missing GOTO");
        // At least two labels: loop top and loop exit.
        let lab_count = count_op(&out, "L");
        assert!(lab_count >= 2, "while loop should have at least 2 labels, got {}", lab_count);
    }

    #[test]
    fn test_while_condition_branches_correctly() {
        let out = compile(
            r#"public class t {
                 public static void main(String argv[]) {
                   int x;
                   x = 5;
                   while (x > 3) { x = x - 1; }
                 }
               }"#,
        );
        // x > 3 → BGT
        assert!(has_op(&out, "BGT"), "x > 3 should emit BGT");
        // imm:3 should appear as the RHS
        assert!(out.contains("imm:3"), "literal 3 should appear as imm:3");
    }

    // ── Control flow — if ─────────────────────────────────────────────────────

    #[test]
    fn test_if_emits_branch() {
        let out = compile(
            r#"public class t {
                 public static void main(String argv[]) {
                   int x;
                   x = 1;
                   if (x > 0) { x = 2; }
                 }
               }"#,
        );
        assert!(has_op(&out, "BGT"), "if (x > 0) should emit BGT");
        // IfThenStmt uses on_false directly — no unconditional GOTO needed.
    }

    #[test]
    fn test_if_else_emits_two_branches() {
        let out = compile(
            r#"public class t {
                 public static void main(String argv[]) {
                   int x;
                   x = 1;
                   if (x > 0) { x = 2; } else { x = 3; }
                 }
               }"#,
        );
        assert!(has_op(&out, "BGT"));
        // Should have GOTO to skip else + at least 2 labels
        assert!(count_op(&out, "GOTO") >= 1);
    }

    // ── Method calls ──────────────────────────────────────────────────────────

    #[test]
    fn test_println_emits_call() {
        let out = compile(
            r#"public class t {
                 public static void main(String argv[]) {
                   System.out.println("hi");
                 }
               }"#,
        );
        assert!(out.contains("CALL PrintStream__println"), "println should mangle to PrintStream__println");
        assert!(has_op(&out, "PARM"), "method call should emit PARM");
    }

    #[test]
    fn test_println_arg_is_strings_region() {
        let out = compile(
            r#"public class t {
                 public static void main(String argv[]) {
                   System.out.println("hello");
                 }
               }"#,
        );
        assert!(out.contains("PARM strings:0"), "string arg should be PARM strings:0");
    }

    #[test]
    fn test_method_call_parm_count() {
        let out = compile(
            r#"public class t {
                 public static void main(String argv[]) {
                   System.out.println("hi");
                 }
               }"#,
        );
        // CALL PrintStream__println,imm:1  — one explicit arg
        assert!(out.contains("CALL PrintStream__println,imm:1"));
    }

    // ── Return ────────────────────────────────────────────────────────────────

    #[test]
    fn test_void_method_ends_with_ret() {
        let out = compile(
            r#"public class t {
                 public static void main(String argv[]) {}
               }"#,
        );
        assert!(has_op(&out, "RET"), "void method should end with RET");
    }

    // ── hello_loop golden structure ───────────────────────────────────────────

    #[test]
    fn test_hello_loop_structure() {
        let src = r#"public class hello_loop {
                       public static void main(String argv[]) {
                         int x;
                         x = argv.length;
                         x = x + 2;
                         while (x > 3) {
                           System.out.println("hello, jzero!");
                           x = x - 1;
                         }
                       }
                     }"#;
        let out = compile(src);

        // Sections
        assert!(has_section(&out, ".string"));
        assert!(has_section(&out, ".global"));
        assert!(has_section(&out, ".code"));

        // String pool
        assert!(out.contains("string \"hello, jzero!\""));

        // Variable layout
        assert!(out.contains("loc:8"),  "argv at loc:8");
        assert!(out.contains("loc:16"), "x at loc:16");

        // Instruction sequence
        assert!(has_op(&out, "ASIZE"), "argv.length → ASIZE");
        assert!(has_op(&out, "ADD"),   "x + 2 → ADD");
        assert!(has_op(&out, "BGT"),   "x > 3 → BGT");
        assert!(has_op(&out, "GOTO"),  "loop needs GOTO");
        assert!(out.contains("CALL PrintStream__println"));
        assert!(has_op(&out, "SUB"),   "x - 1 → SUB");
        assert!(has_op(&out, "RET"),   "method ends with RET");

        // Operand values
        assert!(out.contains("imm:2"), "literal 2 as immediate");
        assert!(out.contains("imm:3"), "literal 3 as immediate");
        assert!(out.contains("imm:1"), "literal 1 as immediate");
    }
}