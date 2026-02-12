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
| Parsing & AST | `jzero-parser` | Ch. 4–5 | ⬜ Planned |
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
│   ├── jzero-parser/       # Parsing & AST construction
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

| Original (Book) | Rust Equivalent |
|---|---|
| JFlex (lexer generator) | Logos crate |
| BYACC/J (parser generator) | TBD (lalrpop / pest / hand-written) |
| Java bytecode | Custom bytecode format |
| Java VM | Custom Rust VM |

## Building & Testing

```bash
# Build everything
cargo build

# Run all tests
cargo test

# Test a specific crate
cargo test -p jzero-lexer
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