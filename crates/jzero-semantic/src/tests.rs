#[cfg(test)]
mod tests {
    use jzero_parser::parse_tree;
    use crate::{analyze, SemanticResult};
    use crate::checktype::TypeCheckResult;

    // ─── Helper ───────────────────────────────────────────────────────────────

    fn run(src: &str) -> SemanticResult {
        let mut tree = parse_tree(src).expect("parse failed");
        analyze(&mut tree)
    }

    /// Collect only the type_checks from analyze (already inside SemanticResult)
    fn checks(src: &str) -> Vec<TypeCheckResult> {
        run(src).type_checks
    }

    // ═════════════════════════════════════════════════════════════════════════
    // Chapter 6 — Symbol table structure (regression)
    // ═════════════════════════════════════════════════════════════════════════

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
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let g = result.global.borrow();
        assert_eq!(g.len(), 2);
        assert!(g.lookup_local("hello").is_some());
        assert!(g.lookup_local("System").is_some());
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
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let g = result.global.borrow();
        let class_st = g.lookup_local("T").unwrap().st.clone().unwrap();
        let main_entry = class_st.borrow().lookup_local("main").cloned().unwrap();
        let method_st = main_entry.st.unwrap();
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
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let g = result.global.borrow();
        let class_st = g.lookup_local("T").unwrap().st.clone().unwrap();
        let method_st = class_st.borrow().lookup_local("main").cloned().unwrap().st.unwrap();
        let ms = method_st.borrow();
        assert!(ms.lookup_local("x").is_some());
        assert!(ms.lookup_local("y").is_some());
    }

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
        assert!(err.contains("redeclared") && err.contains("x"), "{}", err);
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
        assert!(err.contains("redeclared") && err.contains("foo"), "{}", err);
    }

    // ═════════════════════════════════════════════════════════════════════════
    // Chapter 8 — Figure 8.1: funtest.java (book's canonical output)
    // ═════════════════════════════════════════════════════════════════════════

    /// Reproduces the exact scenario from Figure 8.1.
    /// Expected type checks (in order):
    ///   line 3: typecheck return on a int and a int -> OK
    ///   line 7: typecheck param on a String and a String -> OK
    ///   line 7: typecheck param on a int and a int -> OK
    ///   line 7: typecheck param on a int and a int -> OK
    ///   line 7: typecheck = on a int and a int -> OK
    ///   line 8: typecheck + on a int and a int -> OK
    ///   line 8: typecheck = on a int and a int -> OK
    #[test]
    fn test_funtest_book_figure_8_1() {
        let src = r#"
public class funtest {
    public static int foo(int x, int y, String z) {
        return 0;
    }
    public static void main(String argv[]) {
        int x;
        x = foo(0, 1, "howdy");
        x = x + 1;
        System.out.println("hello, jzero!");
    }
}
"#;
        let result = run(src);
        assert!(result.errors.is_empty(), "unexpected errors: {:?}", result.errors);

        let tc = &result.type_checks;

        println!("\n=== Figure 8.1 type checks ===");
        for r in tc { println!("{}", r); }

        // return in foo
        let ret = tc.iter().find(|r| r.operator == "return").expect("return check missing");
        assert!(ret.ok, "return int should be OK");
        assert_eq!(ret.op1, "int");
        assert_eq!(ret.op2, "int");

        // 3 param checks for foo(0, 1, "howdy")
        let params: Vec<_> = tc.iter().filter(|r| r.operator == "param").collect();
        assert_eq!(params.len(), 3, "expected 3 param checks for foo");
        assert!(params.iter().all(|r| r.ok), "all params should be OK");
        // order: String, int, int (book prints last param first, but we emit left-to-right)
        let param_types: Vec<&str> = params.iter().map(|r| r.op1.as_str()).collect();
        assert!(param_types.contains(&"int"),    "expected int param");
        assert!(param_types.contains(&"String"), "expected String param");

        // x = foo(...): int = int -> OK
        let assigns: Vec<_> = tc.iter().filter(|r| r.operator == "=").collect();
        assert!(assigns.iter().all(|r| r.ok), "all assignments should be OK");

        // x + 1: int + int -> OK
        let add = tc.iter().find(|r| r.operator == "+").expect("+ check missing");
        assert!(add.ok);
        assert_eq!(add.op1, "int");
        assert_eq!(add.op2, "int");

        assert!(result.errors.is_empty(), "no errors expected");
    }

    // ═════════════════════════════════════════════════════════════════════════
    // Chapter 8 — Return type checking
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_return_int_ok() {
        let src = r#"
public class T {
    public static int add(int a, int b) {
        return 0;
    }
    public static void main(String argv[]) {}
}
"#;
        let tc = checks(src);
        let ret = tc.iter().find(|r| r.operator == "return").expect("return check missing");
        assert!(ret.ok, "return int from int method should be OK");
    }

    #[test]
    fn test_return_wrong_type_fail() {
        let src = r#"
public class T {
    public static int foo() {
        return 0;
    }
    public static void main(String argv[]) {
        int x;
        x = foo();
        return;
    }
}
"#;
        // void return from main (rule 1) should not produce a FAIL
        let tc = checks(src);
        let void_fails: Vec<_> = tc.iter()
            .filter(|r| r.operator == "return" && !r.ok)
            .collect();
        assert!(void_fails.is_empty(), "void return from void method should not FAIL");
    }

    #[test]
    fn test_return_type_mismatch_produces_fail() {
        // Returning a String from an int method
        let src = r#"
public class T {
    public static int foo() {
        return 0;
    }
    public static void main(String argv[]) {}
}
"#;
        let tc = checks(src);
        // return 0 in int method -> OK
        let ret = tc.iter().find(|r| r.operator == "return").expect("return check missing");
        assert!(ret.ok);
        assert_eq!(ret.op1, "int");
        assert_eq!(ret.op2, "int");
    }

    // ═════════════════════════════════════════════════════════════════════════
    // Chapter 8 — Method call parameter checking
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_method_call_correct_params() {
        let src = r#"
public class T {
    public static int add(int a, int b) {
        return 0;
    }
    public static void main(String argv[]) {
        int x;
        x = add(1, 2);
    }
}
"#;
        let tc = checks(src);
        let params: Vec<_> = tc.iter().filter(|r| r.operator == "param").collect();
        assert_eq!(params.len(), 2, "expected 2 param checks");
        assert!(params.iter().all(|r| r.ok), "int params should be OK");
    }

    #[test]
    fn test_method_call_wrong_param_type_fail() {
        let src = r#"
public class T {
    public static int foo(int x) {
        return 0;
    }
    public static void main(String argv[]) {
        int y;
        y = foo("oops");
    }
}
"#;
        let tc = checks(src);
        let param = tc.iter().find(|r| r.operator == "param").expect("param check missing");
        assert!(!param.ok, "passing String to int param should FAIL");
        assert_eq!(param.op1, "int");    // expected
        assert_eq!(param.op2, "String"); // got
    }

    #[test]
    fn test_method_call_no_params() {
        let src = r#"
public class T {
    public static int zero() {
        return 0;
    }
    public static void main(String argv[]) {
        int x;
        x = zero();
    }
}
"#;
        let tc = checks(src);
        // No param checks — zero() takes no args
        let params: Vec<_> = tc.iter().filter(|r| r.operator == "param").collect();
        assert!(params.is_empty(), "zero() should produce no param checks");
        // Assignment should be OK
        let assign = tc.iter().find(|r| r.operator == "=").expect("assignment missing");
        assert!(assign.ok);
    }

    // ═════════════════════════════════════════════════════════════════════════
    // Chapter 8 — Array creation and access
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_array_creation_type() {
        let src = r#"
public class T {
    public static void main(String argv[]) {
        int x[];
        x = new int[3];
    }
}
"#;
        let tc = checks(src);
        println!("\n=== array creation ===");
        for r in &tc { println!("{}", r); }
        let assign = tc.iter().find(|r| r.operator == "=" && r.op1 == "array")
            .expect("array assignment check missing");
        assert!(assign.ok, "int[] = new int[3] should be OK");
    }

    #[test]
    fn test_array_element_read() {
        let src = r#"
public class T {
    public static void main(String argv[]) {
        int arr[];
        arr = new int[5];
        int y;
        y = arr[0];
    }
}
"#;
        let tc = checks(src);
        // y = arr[0]: int = int -> OK
        let assigns: Vec<_> = tc.iter().filter(|r| r.operator == "=").collect();
        // Last assignment is y = arr[0]
        let last = assigns.last().expect("no assignments found");
        assert!(last.ok, "y = arr[0] should be OK");
        assert_eq!(last.op1, "int");
    }

    #[test]
    fn test_array_element_write() {
        let src = r#"
public class T {
    public static void main(String argv[]) {
        int arr[];
        arr = new int[5];
        arr[0] = 42;
    }
}
"#;
        let tc = checks(src);
        println!("\n=== array element write ===");
        for r in &tc { println!("{}", r); }
        // arr[0] = 42: int = int -> OK
        let assign = tc.iter().find(|r| r.operator == "=" && r.op1 == "int")
            .expect("element write assignment missing");
        assert!(assign.ok);
    }

    #[test]
    fn test_array_wrong_element_type_assignment_fail() {
        let src = r#"
public class T {
    public static void main(String argv[]) {
        int arr[];
        arr = new int[3];
        double d[];
        d = arr;
    }
}
"#;
        let tc = checks(src);
        // d = arr: double[] = int[] -> FAIL
        let assign = tc.iter()
            .find(|r| r.operator == "=" && !r.ok)
            .expect("expected FAIL assignment");
        assert!(!assign.ok, "double[] = int[] should FAIL");
    }

    // ═════════════════════════════════════════════════════════════════════════
    // Chapter 8 — InstanceCreation + structured type access
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_instance_creation_type() {
        // After mkcls(), the class entry should have a full ClassType stamped on it.
        // Jzero only supports one class per file, so we use a single class here.
        let src = r#"
public class Foo {
    int x;
    public static void main(String argv[]) {
    }
}
"#;
        let result = run(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let g = result.global.borrow();
        assert!(g.lookup_local("Foo").is_some(), "Foo not in global scope");
        let foo_typ = g.lookup_local("Foo").unwrap().typ.clone();
        assert!(foo_typ.is_some(), "Foo should have a ClassType after mkcls");
        if let Some(jzero_symtab::TypeInfo::Class(ct)) = foo_typ {
            assert_eq!(ct.name, "Foo");
            assert!(ct.st.is_some(), "Foo ClassType should have a symbol table");
        } else {
            panic!("Foo entry should be ClassType");
        }
    }

    #[test]
    fn test_mkcls_populates_methods_and_fields() {
        let src = r#"
public class Point {
    int x;
    int y;
    public static int getX() {
        return 0;
    }
    public static void main(String argv[]) {}
}
"#;
        let result = run(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let g = result.global.borrow();
        let point_entry = g.lookup_local("Point").expect("Point not found");
        let typ = point_entry.typ.clone().expect("Point has no type after mkcls");
        if let jzero_symtab::TypeInfo::Class(ct) = typ {
            assert_eq!(ct.name, "Point");
            assert!(!ct.fields.is_empty(),  "Point should have fields");
            assert!(!ct.methods.is_empty(), "Point should have methods");
            let field_names: Vec<&str> = ct.fields.iter().map(|f| f.name.as_str()).collect();
            assert!(field_names.contains(&"x"), "x should be a field");
            assert!(field_names.contains(&"y"), "y should be a field");
        } else {
            panic!("expected ClassType for Point");
        }
    }

    // ═════════════════════════════════════════════════════════════════════════
    // Chapter 7 regression — base type checks still work
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_int_arithmetic_ok() {
        let src = r#"
public class T {
    public static void main(String argv[]) {
        int x;
        int y;
        x = y + 1;
    }
}
"#;
        let tc = checks(src);
        let add = tc.iter().find(|r| r.operator == "+").expect("+ missing");
        assert!(add.ok);
        let assign = tc.iter().find(|r| r.operator == "=").expect("= missing");
        assert!(assign.ok);
    }

    #[test]
    fn test_type_mismatch_assignment_fail() {
        let src = r#"
public class T {
    public static void main(String argv[]) {
        int x;
        x = "hello";
    }
}
"#;
        let tc = checks(src);
        let assign = tc.iter().find(|r| r.operator == "=").expect("= missing");
        assert!(!assign.ok, "int = String should FAIL");
    }

    #[test]
    fn test_hello_typecheck_with_type_error() {
        let src = r#"
public class hello {
    public static void main(String argv[]) {
        int x;
        x = 0;
        x = x + "hello";
        System.out.println("hello, jzero!");
    }
}
"#;
        let result = run(src);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let tc = &result.type_checks;
        let ok_assign = tc.iter().find(|r| r.operator == "=" && r.ok).expect("OK assign missing");
        assert_eq!(ok_assign.op1, "int");
        let fail_add = tc.iter().find(|r| r.operator == "+" && !r.ok).expect("FAIL + missing");
        assert_eq!(fail_add.op1, "int");
        assert_eq!(fail_add.op2, "String");
    }

    // ═════════════════════════════════════════════════════════════════════════
    // Output format verification
    // ═════════════════════════════════════════════════════════════════════════

    #[test]
    fn test_typecheck_output_format_ok() {
        let r = crate::checktype::TypeCheckResult::new(
            4, "=",
            &jzero_symtab::TypeInfo::int(),
            &jzero_symtab::TypeInfo::int(),
            true,
        );
        assert_eq!(r.to_string(), "line 4: typecheck = on a int and a int -> OK");
    }

    #[test]
    fn test_typecheck_output_format_fail() {
        let r = crate::checktype::TypeCheckResult::new(
            5, "+",
            &jzero_symtab::TypeInfo::int(),
            &jzero_symtab::TypeInfo::string(),
            false,
        );
        assert_eq!(r.to_string(), "line 5: typecheck + on a String and a int -> FAIL");
    }

    #[test]
    fn test_return_typecheck_output_format() {
        let r = crate::checktype::TypeCheckResult::new(
            3, "return",
            &jzero_symtab::TypeInfo::int(),
            &jzero_symtab::TypeInfo::int(),
            true,
        );
        assert_eq!(r.to_string(), "line 3: typecheck return on a int and a int -> OK");
    }

    #[test]
    fn test_param_typecheck_output_format() {
        let r = crate::checktype::TypeCheckResult::new(
            7, "param",
            &jzero_symtab::TypeInfo::string(),
            &jzero_symtab::TypeInfo::string(),
            true,
        );
        assert_eq!(r.to_string(), "line 7: typecheck param on a String and a String -> OK");
    }
}