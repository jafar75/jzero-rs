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
| Syntax Tree Construction | `jzero-ast`, `jzero-parser` | Ch. 5 | ✅ Done |
| Symbol Tables | `jzero-symtab`, `jzero-semantic` | Ch. 6 | ✅ Done |
| Type Checking (base types) | `jzero-semantic` | Ch. 7 | ✅ Done |
| Type Checking (arrays, methods) | `jzero-semantic` | Ch. 8 | ⬜ Planned |
| Code Generation | `jzero-codegen` | Ch. 9 | ⬜ Planned |
| Bytecode Interpreter | `jzero-vm` | Ch. 10 | ⬜ Planned |

## Architecture

The project is organized as a Cargo workspace with separate crates for each compiler phase:

```
jzero-rs/
├── Cargo.toml              # Workspace root
├── crates/
│   ├── jzero-lexer/        # Lexical analysis (Logos)
│   ├── jzero-parser/       # Parsing & syntax tree construction (LALRPOP)
│   ├── jzero-ast/          # Syntax tree data structures & DOT output
│   ├── jzero-symtab/       # Symbol table types (SymTab, SymTabEntry, TypeInfo)
│   ├── jzero-cli/          # CLI tool (j0) for parsing and tree visualization
│   ├── jzero-semantic/     # Symbol table construction & type checking
│   ├── jzero-codegen/      # Bytecode generation (planned)
│   └── jzero-vm/           # Bytecode interpreter (planned)
└── tests/
    └── samples/            # Jzero source files for testing
```

Each crate enforces a clean one-way dependency chain:

```
jzero-lexer
     ↓
jzero-symtab   (no external deps — TypeInfo, SymTab, SymTabEntry)
     ↓
jzero-ast      (Tree + semantic attributes: stab, typ, is_const)
     ↓
jzero-parser   (LALRPOP grammar → Tree)
     ↓
jzero-semantic (symbol table construction + type checking)
     ↓
jzero-codegen  (planned)
     ↓
jzero-vm       (planned)
```

## Tool Mapping

| Original (Book) | Rust Equivalent | Notes |
|---|---|---|
| JFlex (lexer generator) | [Logos](https://github.com/maciejhirsz/logos) | Derive-macro based, zero-copy |
| BYACC/J (parser generator) | [LALRPOP](https://github.com/lalrpop/lalrpop) | LR(1) with lane table algorithm |
| `tree.java` / `tree.icn` | `jzero-ast` crate | Uniform `Tree` struct with DOT output |
| `symtab.java` / `symtab_entry.java` | `jzero-symtab` crate | Rust enum-based type hierarchy |
| `typeinfo.java` + subclasses | `TypeInfo` enum | `Base`, `Array`, `Method`, `Class` variants |
| Graphviz DOT visualization | Graphviz (same) | CLI tool generates `.dot` files |
| Java bytecode | Custom bytecode format (planned) | — |
| Java VM | Custom Rust VM (planned) | — |

## Semantic Analysis (Chapters 6–7)

Semantic analysis runs in four phases, all implemented in `jzero-semantic`:

### Phase 1 — Symbol table construction (Ch. 6)

A two-pass tree traversal builds one `SymTab` per scope:

- **First pass** over each class body registers all field and method signatures upfront, allowing forward references within a class.
- **Second pass** walks method bodies to insert parameters and local variables.
- The predefined `System.out.println` hierarchy is inserted into the global scope before the user's code is walked.
- Each `Tree` node gets its `stab` field set to the nearest enclosing scope's symbol table (an inherited attribute, propagated top-down).

Errors detected: **redeclared variables**.

Example symbol table output for `hello.java`:

```
global - 2 symbols
 hello
  class - 2 symbols
   main
    method - 0 symbols
   System
 System
  class - 1 symbols
   out
    class - 1 symbols
     println
```

### Phase 2 — Leaf type assignment (Ch. 7)

A pre-order pass stamps `typ` on all literal and operator leaf nodes before any expression checking:

| Token | Type |
|---|---|
| `INTLIT` | `int` |
| `DOUBLELIT` | `double` |
| `STRINGLIT` | `String` |
| `BOOLLIT` | `boolean` |
| `NULL` | `null` |
| operators (`+`, `-`, `=`, …) | `n/a` |

### Phase 3 — Declaration type assignment (Ch. 7)

Two cooperative traversals process `FieldDecl`, `LocalVarDecl`, and `FormalParm` nodes:

- **`calc_type`** (post-order) synthesizes a `TypeInfo` from the `Type` child node — resolving keywords (`int`, `void`, …) and identifiers (user-defined class types) into `TypeInfo` values.
- **`assign_type`** (top-down) inherits the resolved type downward through `VarDeclarator` nodes, wrapping in `TypeInfo::Array(...)` when array brackets are present, and finally storing the type in the corresponding `SymTabEntry`.

### Phase 4 — Expression type checking (Ch. 7)

A selective traversal checks types in method bodies using three cooperative functions:

- **`check_type`** — dispatches on node type to compute and verify the type of each expression.
- **`check_kids`** — controls which children are visited (e.g. only the body of a `MethodDecl`, not its header).
- **`check_types`** — enforces operator-specific type compatibility rules and records each result.

Results are collected as `Vec<TypeCheckResult>` with the format:

```
line 4: typecheck = on a int and a int -> OK
line 5: typecheck + on a String and a int -> FAIL
```

Both OK and FAIL results are collected — the compiler reports all type errors in one pass.

### TypeInfo hierarchy

The book's `typeinfo` class hierarchy is represented as a single Rust enum in `jzero-symtab`:

```rust
pub enum TypeInfo {
    Base(String),       // "int", "double", "boolean", "String", "void", "null", "n/a"
    Array(Box<TypeInfo>),
    Method(MethodType), // return_type + Vec<Parameter>
    Class(ClassType),   // name + optional SymTab reference
}
```

### Semantic attributes on Tree nodes

Two semantic attributes were added to `Tree` in Chapter 6–7:

| Field | Kind | Description |
|---|---|---|
| `stab: Option<Rc<RefCell<SymTab>>>` | Inherited | Nearest enclosing scope's symbol table |
| `is_const: Option<bool>` | Synthesized | Whether this subtree is a compile-time constant |
| `typ: Option<TypeInfo>` | Synthesized | The type of the value this node computes |

All fields default to `None` — the parser remains unaware of semantic concerns.

## Syntax Tree (Chapter 5)

The parser builds a **syntax tree** (not a full parse tree) during parsing. Internal nodes are only created when a grammar rule has two or more children — single-child productions pass through without creating a node, keeping the tree compact.

The tree uses a uniform node structure (`jzero_ast::tree::Tree`):

- **Leaf nodes** hold token information (category, source text, line number)
- **Internal nodes** hold a production rule name, rule alternative number, and child nodes

Left-factored grammar rules (like `IdentifierStartedStmt` and `DotTail`) use deferred closures (`TreeAction`) to reconstruct logical tree nodes such as `MethodCall`, `FieldAccess`, and `Assignment`, hiding the left-factoring from the tree structure.

### Tree visualization

The CLI tool `j0` parses a Jzero source file and produces both text output and a Graphviz DOT file:

```bash
# Parse and print tree + write DOT file
cargo run --bin j0 -- tests/hello.java

# Also render to PNG (requires Graphviz)
cargo run --bin j0 -- tests/hello.java --png
```

Example output for `hello.java`:

```
ClassDecl#0 (2 kids)
  [IDENTIFIER] hello (line 1)
  MethodDecl#0 (2 kids)
    MethodHeader#0 (2 kids)
      [VOID] void (line 2)
      MethodDeclarator#0 (2 kids)
        [IDENTIFIER] main (line 2)
        FormalParm#0 (2 kids)
          [IDENTIFIER] String (line 2)
          VarDeclarator#1 (1 kids)
            VarDeclarator#0 (1 kids)
              [IDENTIFIER] argv (line 2)
    Block#0 (1 kids)
      MethodCall#0 (2 kids)
        FieldAccess#0 (2 kids)
          FieldAccess#0 (2 kids)
            [IDENTIFIER] System (line 3)
            [IDENTIFIER] out (line 3)
          [IDENTIFIER] println (line 3)
        [STRINGLIT] "hello, jzero!" (line 3)
```

### Differences from the book's tree

The tree is structurally equivalent to the book's output with a few minor differences due to grammar adaptations:

- **`FieldAccess` instead of `QualifiedName`** — dotted names like `System.out` are represented as nested `FieldAccess` nodes rather than `QualifiedName` chains, since we removed `QualifiedName` from the grammar in Chapter 4.
- **`ClassBody` is flattened** — the book has an explicit `ClassBody#0` node; ours folds its children directly into `ClassDecl`.
- **`VarDeclarator` nesting for arrays** — `argv[]` produces `VarDeclarator#1(VarDeclarator#0(argv))` to explicitly record array brackets, whereas the book uses a single node.

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
cargo test -p jzero-ast
cargo test -p jzero-symtab
cargo test -p jzero-semantic

# Parse a Jzero source file and visualize the syntax tree
cargo run --bin j0 -- tests/hello.java --png
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