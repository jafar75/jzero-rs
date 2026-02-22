/// A semantic error found during analysis.
#[derive(Debug, Clone)]
pub enum SemanticError {
    /// A variable was used but never declared.
    UndeclaredVariable {
        name: String,
        lineno: usize,
    },
    /// A variable was declared more than once in the same scope.
    RedeclaredVariable {
        name: String,
        lineno: usize,
    },
}

impl std::fmt::Display for SemanticError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SemanticError::UndeclaredVariable { name, lineno } =>
                write!(f, "line {}: undeclared variable '{}'", lineno, name),
            SemanticError::RedeclaredVariable { name, lineno } =>
                write!(f, "line {}: redeclared variable '{}'", lineno, name),
        }
    }
}