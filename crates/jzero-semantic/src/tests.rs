#[cfg(test)]
mod tests {
    use jzero_parser::parse_tree;
    use crate::analyze;

    // ─── Helper ───────────────────────────────────────────────────────────────

    fn run(src: &str) -> crate::SemanticResult {
        let mut tree = parse_tree(src).expect("parse failed");
        analyze(&mut tree)
    }

    // ─── Symbol table structure ───────────────────────────────────────────────

    #[test]
    fn test_hello_world_global_scope() {
        let src = r#"
public class hello {
    public static void main(String argv[]) {
        System.out.println("hello, jzero!");
    }
}
"#;
        let result = run(src);
        assert!(result.errors.is_empty(), "unexpected errors: {:?}", result.errors);

        let g = result.global.borrow();

        // Global scope has exactly 2 symbols: hello + System (predefined)
        assert_eq!(g.len(), 2);
        assert!(g.lookup_local("hello").is_some(), "hello not in global");
        assert!(g.lookup_local("System").is_some(), "System not in global");
    }

    #[test]
    fn test_hello_world_class_scope() {
        let src = r#"
public class hello {
    public static void main(String argv[]) {
        System.out.println("hello, jzero!");
    }
}
"#;
        let result = run(src);
        let g = result.global.borrow();

        // hello class scope has 2 symbols: main + System (injected by predefined)
        let hello_entry = g.lookup_local("hello").expect("hello not found");
        let class_st = hello_entry.st.as_ref().expect("hello has no child scope");
        let cs = class_st.borrow();
        assert_eq!(cs.scope, "class");

        let main_entry = cs.lookup_local("main").expect("main not in class scope");
        assert_eq!(main_entry.kind, jzero_symtab::entry::SymbolKind::Method);
    }

    #[test]
    fn test_method_scope_has_param() {
        let src = r#"
public class T {
    public static void main(String argv[]) {
    }
}
"#;
        let result = run(src);
        assert!(result.errors.is_empty(), "unexpected errors: {:?}", result.errors);

        let g = result.global.borrow();
        let class_entry = g.lookup_local("T").expect("T not found");
        let class_st = class_entry.st.as_ref().expect("T has no class scope");
        let main_entry = class_st.borrow()
            .lookup_local("main").cloned()
            .expect("main not found");
        let method_st = main_entry.st.as_ref().expect("main has no method scope").clone();

        // argv should be in the method scope as a param
        let argv = method_st.borrow().lookup_local("argv").cloned();
        assert!(argv.is_some(), "argv not in method scope");
        assert_eq!(argv.unwrap().kind, jzero_symtab::entry::SymbolKind::Param);
    }

    #[test]
    fn test_local_var_in_method_scope() {
        let src = r#"
public class T {
    public static void main(String argv[]) {
        int x;
        int y;
    }
}
"#;
        let result = run(src);
        assert!(result.errors.is_empty(), "unexpected errors: {:?}", result.errors);

        let g = result.global.borrow();
        let class_st = g.lookup_local("T").unwrap().st.clone().unwrap();
        let method_st = class_st.borrow()
            .lookup_local("main").cloned().unwrap()
            .st.clone().unwrap();

        let ms = method_st.borrow();
        assert!(ms.lookup_local("x").is_some(), "x not found");
        assert!(ms.lookup_local("y").is_some(), "y not found");
        assert_eq!(ms.lookup_local("x").unwrap().kind, jzero_symtab::entry::SymbolKind::Local);
    }

    // ─── Print output (visual) ────────────────────────────────────────────────

    #[test]
    fn test_print_hello_world() {
        let src = r#"
public class hello {
    public static void main(String argv[]) {
        System.out.println("hello, jzero!");
    }
}
"#;
        let result = run(src);
        assert!(result.errors.is_empty(), "unexpected errors: {:?}", result.errors);

        println!("\n=== Symbol Table ===");
        result.global.borrow().print(0);
        // Run with: cargo test test_print_hello_world -- --nocapture
        // Expected output:
        // global - 2 symbols
        //  hello
        //   class - 2 symbols
        //    main
        //     method - 0 symbols
        //    System
        //  System
        //   class - 1 symbols
        //    out
        //     class - 1 symbols
        //      println
    }

    // ─── Error detection ──────────────────────────────────────────────────────

    #[test]
    fn test_redeclared_local_variable() {
        let src = r#"
public class T {
    public static void main(String argv[]) {
        int x;
        int x;
    }
}
"#;
        let result = run(src);
        assert_eq!(result.errors.len(), 1);
        let err = result.errors[0].to_string();
        assert!(err.contains("redeclared"), "expected redeclared error, got: {}", err);
        assert!(err.contains("x"), "expected 'x' in error, got: {}", err);
    }

    #[test]
    fn test_redeclared_method() {
        let src = r#"
public class T {
    public static void foo() { }
    public static void foo() { }
}
"#;
        let result = run(src);
        assert_eq!(result.errors.len(), 1);
        let err = result.errors[0].to_string();
        assert!(err.contains("redeclared"));
        assert!(err.contains("foo"));
    }

    #[test]
    fn test_no_errors_for_valid_program() {
        let src = r#"
public class T {
    public static void main(String argv[]) {
        int x;
        int y;
        x = 42;
        y = x + 1;
    }
}
"#;
        let result = run(src);
        assert!(result.errors.is_empty(), "unexpected errors: {:?}", result.errors);
    }
}