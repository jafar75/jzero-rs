//! Type information for Jzero values, declarations, and expressions.
//!
//! Mirrors the book's `typeinfo` class hierarchy:
//!   typeinfo  →  Base type (int, double, bool, String, void, null, n/a, unknown)
//!   arraytype →  Array(element_type)
//!   methodtype → Method { return_type, parameters }
//!   classtype  → Class { name, st }
//!
//! In Rust we use a single enum instead of inheritance.

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use crate::symtab::SymTab;

// ─── Parameter ───────────────────────────────────────────────────────────────

/// A named, typed parameter — used by `MethodType` and `ClassType`.
/// Mirrors the book's `parameter` class (not a subtype of typeinfo).
#[derive(Debug, Clone)]
pub struct Parameter {
    pub name: String,
    pub param_type: Box<TypeInfo>,
}

impl Parameter {
    pub fn new(name: &str, param_type: TypeInfo) -> Self {
        Parameter {
            name: name.to_string(),
            param_type: Box::new(param_type),
        }
    }
}

impl fmt::Display for Parameter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.name, self.param_type)
    }
}

// ─── MethodType ──────────────────────────────────────────────────────────────

/// Type information for a method declaration.
#[derive(Debug, Clone)]
pub struct MethodType {
    pub return_type: Box<TypeInfo>,
    pub parameters: Vec<Parameter>,
}

impl MethodType {
    pub fn new(return_type: TypeInfo, parameters: Vec<Parameter>) -> Self {
        MethodType {
            return_type: Box::new(return_type),
            parameters,
        }
    }
}

impl fmt::Display for MethodType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let params: Vec<String> = self.parameters.iter().map(|p| p.to_string()).collect();
        write!(f, "method({}) -> {}", params.join(", "), self.return_type)
    }
}

// ─── ClassType ───────────────────────────────────────────────────────────────

/// Type information for a class declaration.
#[derive(Debug, Clone)]
pub struct ClassType {
    /// The class name, e.g. "hello", "System".
    pub name: String,
    /// The class's own symbol table, if available.
    pub st: Option<Rc<RefCell<SymTab>>>,
    /// Method signatures declared in this class.
    pub methods: Vec<Parameter>,
    /// Field declarations in this class.
    pub fields: Vec<Parameter>,
    /// Constructor types.
    pub constrs: Vec<TypeInfo>,
}

impl ClassType {
    /// Create a minimal class type with just a name (used during early passes).
    pub fn new(name: &str) -> Self {
        ClassType {
            name: name.to_string(),
            st: None,
            methods: Vec::new(),
            fields: Vec::new(),
            constrs: Vec::new(),
        }
    }

    /// Create a class type with an associated symbol table.
    pub fn with_symtab(name: &str, st: Rc<RefCell<SymTab>>) -> Self {
        ClassType {
            name: name.to_string(),
            st: Some(st),
            methods: Vec::new(),
            fields: Vec::new(),
            constrs: Vec::new(),
        }
    }
}

impl fmt::Display for ClassType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Display as just the name so Array(ClassType("String")) prints "String[]"
        // matching the book's typecheck output format.
        write!(f, "{}", self.name)
    }
}

// ─── TypeInfo ────────────────────────────────────────────────────────────────

/// The type of any value, expression, or declaration in Jzero.
///
/// Mirrors the book's `typeinfo` class hierarchy using a Rust enum.
#[derive(Debug, Clone)]
pub enum TypeInfo {
    /// A primitive or built-in base type.
    /// Covers: "int", "double", "boolean", "String", "void", "null", "n/a", "unknown"
    Base(String),

    /// An array type — wraps the element type.
    /// e.g. `int[]` → `Array(Box::new(Base("int")))`
    Array(Box<TypeInfo>),

    /// A method type — return type + ordered parameter list.
    Method(MethodType),

    /// A user-defined class type.
    Class(ClassType),
}

impl TypeInfo {
    // ─── Constructors ─────────────────────────────────────────────────────

    pub fn int()     -> Self { TypeInfo::Base("int".to_string()) }
    pub fn double()  -> Self { TypeInfo::Base("double".to_string()) }
    pub fn boolean() -> Self { TypeInfo::Base("boolean".to_string()) }
    pub fn string()  -> Self { TypeInfo::Base("String".to_string()) }
    pub fn void()    -> Self { TypeInfo::Base("void".to_string()) }
    pub fn null()    -> Self { TypeInfo::Base("null".to_string()) }
    pub fn na()      -> Self { TypeInfo::Base("n/a".to_string()) }
    pub fn unknown() -> Self { TypeInfo::Base("unknown".to_string()) }

    pub fn array(element: TypeInfo) -> Self {
        TypeInfo::Array(Box::new(element))
    }

    pub fn method(return_type: TypeInfo, parameters: Vec<Parameter>) -> Self {
        TypeInfo::Method(MethodType::new(return_type, parameters))
    }

    pub fn class(name: &str) -> Self {
        TypeInfo::Class(ClassType::new(name))
    }

    pub fn class_with_symtab(name: &str, st: Rc<RefCell<SymTab>>) -> Self {
        TypeInfo::Class(ClassType::with_symtab(name, st))
    }

    // ─── Queries ──────────────────────────────────────────────────────────

    /// The basetype string — matches the book's `typeinfo.basetype` field.
    /// Used in typecheck messages: "typecheck + on a int and a int -> OK"
    pub fn basetype(&self) -> &str {
        match self {
            TypeInfo::Base(s)    => s.as_str(),
            TypeInfo::Array(_)   => "array",
            TypeInfo::Method(_)  => "method",
            TypeInfo::Class(ct)  => &ct.name,
        }
    }

    /// Human-readable string — mirrors the book's `typeinfo.str()` method.
    pub fn str(&self) -> String {
        self.basetype().to_string()
    }

    /// Returns true if this is a numeric type (int or double).
    pub fn is_numeric(&self) -> bool {
        matches!(self, TypeInfo::Base(s) if s == "int" || s == "double")
    }

    /// Returns true if this is the boolean type.
    pub fn is_boolean(&self) -> bool {
        matches!(self, TypeInfo::Base(s) if s == "boolean")
    }

    /// Returns true if two types have the same basetype string.
    pub fn same_base(&self, other: &TypeInfo) -> bool {
        self.basetype() == other.basetype()
    }
}

impl fmt::Display for TypeInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypeInfo::Base(s)   => write!(f, "{}", s),
            TypeInfo::Array(t)  => write!(f, "{}[]", t),
            TypeInfo::Method(m) => write!(f, "{}", m),
            TypeInfo::Class(c)  => write!(f, "{}", c),
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_types() {
        assert_eq!(TypeInfo::int().basetype(), "int");
        assert_eq!(TypeInfo::double().basetype(), "double");
        assert_eq!(TypeInfo::boolean().basetype(), "boolean");
        assert_eq!(TypeInfo::string().basetype(), "String");
        assert_eq!(TypeInfo::void().basetype(), "void");
        assert_eq!(TypeInfo::null().basetype(), "null");
        assert_eq!(TypeInfo::na().basetype(), "n/a");
        assert_eq!(TypeInfo::unknown().basetype(), "unknown");
    }

    #[test]
    fn test_array_type() {
        let t = TypeInfo::array(TypeInfo::int());
        assert_eq!(t.basetype(), "array");
        assert_eq!(t.to_string(), "int[]");

        // Nested: int[][]
        let t2 = TypeInfo::array(TypeInfo::array(TypeInfo::int()));
        assert_eq!(t2.to_string(), "int[][]");
    }

    #[test]
    fn test_method_type() {
        let params = vec![
            Parameter::new("argv", TypeInfo::array(TypeInfo::string())),
        ];
        let t = TypeInfo::method(TypeInfo::void(), params);
        assert_eq!(t.basetype(), "method");
        assert_eq!(t.to_string(), "method(argv: String[]) -> void");
    }

    #[test]
    fn test_class_type() {
        let t = TypeInfo::class("hello");
        assert_eq!(t.basetype(), "hello");
        // Display uses the class name directly (not "class(hello)")
        // so Array(ClassType) prints as "hello[]" not "class(hello)[]"
        assert_eq!(t.to_string(), "hello");
    }

    #[test]
    fn test_same_base() {
        assert!(TypeInfo::int().same_base(&TypeInfo::int()));
        assert!(!TypeInfo::int().same_base(&TypeInfo::double()));
        assert!(!TypeInfo::int().same_base(&TypeInfo::string()));
    }

    #[test]
    fn test_is_numeric() {
        assert!(TypeInfo::int().is_numeric());
        assert!(TypeInfo::double().is_numeric());
        assert!(!TypeInfo::string().is_numeric());
        assert!(!TypeInfo::boolean().is_numeric());
    }

    #[test]
    fn test_str_matches_book() {
        // The book prints: "typecheck + on a int and a int -> OK"
        // So str() must return bare basetype, not "class(hello)" etc.
        assert_eq!(TypeInfo::int().str(), "int");
        assert_eq!(TypeInfo::string().str(), "String");
        assert_eq!(TypeInfo::class("hello").str(), "hello");
        assert_eq!(TypeInfo::array(TypeInfo::int()).str(), "array");
    }
}