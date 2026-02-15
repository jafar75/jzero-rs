# jzero-rs

A compiler and bytecode interpreter for **Jzero** — a small subset of Java — written entirely in Rust.

This project follows the book *Build Your Own Programming Language (Edition 2)* by Clinton L. Jeffery, but replaces the original Java/Unicon tooling with Rust equivalents.

## What is Jzero?

Jzero is a strict subset of Java designed for teaching compiler construction. Every valid Jzero program is also a valid Java program. It supports a minimal but complete set of features: classes, methods, control flow, basic types (`int`, `double`, `bool`, `string`), arrays, and simple I/O.

## Roadmap

The project follows the compilation pipeline, chapter by chapter:

| Phase | Crate | Book Chapter | Status |
|---|---|---|---|
| Lexical Analysis | `jzero-lexer` | Ch. 3 | ✅ Done |
| Parsing (accept/reject) | `jzero-parser` | Ch. 4 | ✅ Done |
| AST Construction | `jzero-parser` | Ch. 5 | ⬜ Planned |
| Semantic Analysis | `jzero-semantic` | Ch. 6–7 | ⬜ Planned |
| Code Generation | `jzero-codegen` | Ch. 8–9 | ⬜ Planned |
| Bytecode Interpreter | `jzero-vm` | Ch. 10 | ⬜ Planned |

## Architecture

The project is organized as a Cargo workspace with separate crates for each compiler phase:

```
jzero-rs/
├── Cargo.toml              # Workspace root
├── crates/
│   ├── jzero-lexer/        # Lexical analysis (Logos)
│   ├── jzero-parser/       # Parsing & AST construction (LALRPOP)
│   ├── jzero-semantic/     # Type checking & name resolution
│   ├── jzero-codegen/      # Bytecode generation
│   ├── jzero-vm/           # Bytecode interpreter
│   └── jzero-common/       # Shared types (errors, source spans)
└── tests/
    └── samples/            # Jzero source files for testing
```

Each crate enforces a clean one-way dependency chain:

```
lexer → parser → semantic → codegen → vm
          ↑          ↑          ↑       ↑
          └──────────┴──────────┴───────┘
                      common
```

## Tool Mapping

| Original (Book) | Rust Equivalent | Notes |
|---|---|---|
| JFlex (lexer generator) | [Logos](https://github.com/maciejhirsz/logos) | Derive-macro based, zero-copy |
| BYACC/J (parser generator) | [LALRPOP](https://github.com/lalrpop/lalrpop) | LR(1) with lane table algorithm |
| Java bytecode | Custom bytecode format | — |
| Java VM | Custom Rust VM | — |

## Parser Design Notes

The book's BYACC/J grammar relies on LALR(1) with implicit shift-over-reduce conflict resolution. Adapting it to Rust required some significant changes:

**Why LALRPOP over grmtools/lrpar?** The original grammar has inherent LALR(1) ambiguities (the `IDENTIFIER` at the start of a statement can begin either a type for variable declaration or an expression for method calls/assignments). grmtools' LALR parser resolved these conflicts in ways that broke dotted method calls like `System.out.println(...)`. LALRPOP's LR(1) lane table algorithm handles more grammars without conflicts, and its explicit conflict reporting made it easier to restructure the grammar correctly.

**Key adaptations from the book grammar:**

- **Left-factored `BlockStmt`** — when an `IDENTIFIER` starts a statement, the parser defers the type-vs-expression decision until enough tokens disambiguate (e.g., a second `IDENTIFIER` means variable declaration, `"."` means field access/method call, `"("` means method call).
- **Merged `FieldAccess`/`MethodCall` into `AccessExpr`** — a left-recursive rule that builds chains of `.field` and `.method(args)` accesses, avoiding the mutual recursion between `Primary`, `FieldAccess`, and `MethodCall` that caused conflicts.
- **Removed `QualifiedName` from `Type`** — dotted names like `System.out` are handled purely as expression-level field accesses rather than as qualified type names, since Jzero doesn't need fully-qualified types.
- **Removed standalone `InstantiationExpr`** — its production (`Name "(" ArgListOpt ")"`) was identical to one of `MethodCall`'s alternatives, creating a reduce/reduce conflict.

The Logos lexer (`jzero-lexer`) feeds tokens into the LALRPOP parser through a thin adapter layer (`jzero-parser/src/lexer.rs`) that wraps bare `Token` variants with borrowed string slices for value-bearing tokens (identifiers, literals).

## Building & Testing

```bash
# Build everything
cargo build

# Run all tests
cargo test

# Test a specific crate
cargo test -p jzero-lexer
cargo test -p jzero-parser
```

## Example

A simple Jzero program:

```java
public class hello {
    public static void main(String argv[]) {
        System.out.println("hello, jzero!");
    }
}
```

## References

- [*Build Your Own Programming Language (Edition 2)*](https://a.co/d/0hHvJYWA) — Clinton L. Jeffery
- [Logos](https://github.com/maciejhirsz/logos) — fast lexer generator for Rust
- [LALRPOP](https://github.com/lalrpop/lalrpop) — LR(1) parser generator for Rust