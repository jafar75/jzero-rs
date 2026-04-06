# jzero-rs

A compiler for **Jzero** — a small subset of Java — written entirely in Rust.

This project follows the book *Build Your Own Programming Language (Edition 2)* by Clinton L. Jeffery, replacing the original Java/Unicon tooling with idiomatic Rust equivalents.

## What is Jzero?

Jzero is a strict subset of Java designed for teaching compiler construction. Every valid Jzero program is also a valid Java program. It supports a minimal but complete set of features: classes, methods, control flow, basic types (`int`, `double`, `bool`, `string`), arrays, and simple I/O.

## Roadmap

| Phase | Crate | Book Chapter | Status |
|---|---|---|---|
| Lexical Analysis | `jzero-lexer` | Ch. 3 | ✅ Done |
| Parsing (accept/reject) | `jzero-parser` | Ch. 4 | ✅ Done |
| Syntax Tree Construction | `jzero-ast`, `jzero-parser` | Ch. 5 | ✅ Done |
| Symbol Tables | `jzero-symtab`, `jzero-semantic` | Ch. 6 | ✅ Done |
| Type Checking (base types) | `jzero-semantic` | Ch. 7 | ✅ Done |
| Type Checking (arrays, methods, structs) | `jzero-semantic` | Ch. 8 | ✅ Done |
| Intermediate Code Generation | `jzero-codegen` | Ch. 9 | ✅ Done |
| Bytecode Interpreter | `jzero-vm` | Ch. 10 | ⬜ Planned |

## Architecture

```
jzero-rs/
├── Cargo.toml
├── crates/
│   ├── jzero-lexer/        # Lexical analysis (Logos)
│   ├── jzero-parser/       # Parsing & syntax tree construction (LALRPOP)
│   ├── jzero-ast/          # Syntax tree data structures & DOT output
│   ├── jzero-symtab/       # Symbol table types (SymTab, SymTabEntry, TypeInfo)
│   ├── jzero-semantic/     # Symbol table construction & type checking
│   ├── jzero-codegen/      # Intermediate code generation
│   ├── jzero-cli/          # CLI tool (j0)
│   └── jzero-vm/           # Bytecode interpreter (planned)
└── tests/
    ├── hello.java           # Minimal hello-world test
    └── hello_loop.java      # Loop + array access golden test (Ch. 9)
```

Clean one-way dependency chain:

```
jzero-lexer → jzero-symtab → jzero-ast → jzero-parser → jzero-semantic → jzero-codegen
```

## Tool Mapping

| Original (Book) | Rust Equivalent | Notes |
|---|---|---|
| JFlex | [Logos](https://github.com/maciejhirsz/logos) | Derive-macro based, zero-copy |
| BYACC/J | [LALRPOP](https://github.com/lalrpop/lalrpop) | LR(1) with lane table algorithm |
| `tree.java` | `jzero-ast` crate | Uniform `Tree` struct with DOT output |
| `symtab.java` | `jzero-symtab` crate | Rust enum-based type hierarchy |
| `typeinfo.java` + subclasses | `TypeInfo` enum | `Base`, `Array`, `Method`, `Class` variants |
| `address.java` + `tac.java` | `jzero-codegen` crate | `Address` enum + `Tac` struct |

## Building & Testing

```bash
# Build everything
cargo build

# Run all tests
cargo test

# Run tests for a specific crate
cargo test -p jzero-semantic
cargo test -p jzero-codegen

# Parse a file and visualize the syntax tree
cargo run --bin j0 -- tests/hello.java --png

# Run full compiler pipeline and print intermediate code
cargo run --bin j0 -- tests/hello_loop.java --codegen
```

## Example Output

```java
// tests/hello_loop.java
public class hello_loop {
   public static void main(String argv[]) {
      int x;
      x = argv.length;
      x = x + 2;
      while (x > 3) {
         System.out.println("hello, jzero!");
         x = x - 1;
      }
   }
}
```

Running `j0 tests/hello_loop.java --codegen` produces:

```
.string
L5:
string "hello, jzero!"
.global
global global:0,System
global global:8,hello_loop
.code
proc main,0,1
ASIZE loc:24,loc:8
ASN loc:16,loc:24
ADD loc:32,loc:16,imm:2
ASN loc:16,loc:32
L2:
BGT L3,loc:16,imm:3
GOTO L6
L3:
PARM strings:0
PARM global:0
CALL PrintStream__println,imm:1
SUB loc:48,loc:16,imm:1
ASN loc:16,loc:48
GOTO L2
L6:
RET
end
no errors
```

## Semantic Analysis (Chapters 6–8)

### Symbol table construction (Ch. 6)

A two-pass tree traversal builds one `SymTab` per scope:

- **First pass** registers all field and method signatures, enabling forward references within a class.
- **Second pass** walks method bodies to insert parameters and local variables.
- `System.out.println` is pre-registered in the global scope.
- Each `Tree` node gets its `stab` field set to the nearest enclosing scope (inherited, top-down).

Errors detected: redeclared variables.

### Type checking (Ch. 7–8)

Three cooperative functions — `check_type`, `check_kids`, `check_types` — perform a selective post-order traversal of method bodies.

Key design decisions:
- **`TypeInfo` enum** replaces the book's `typeinfo` class hierarchy — `Base`, `Array`, `Method`, `Class` variants with exhaustive matching.
- **`"return"` dummy symbol** — each method's stab gets a `"return"` entry with the declared return type. `ReturnStmt` nodes look it up directly, avoiding threading return type through the tree.
- **`mkcls` pass** — after symbol tables are fully built, a dedicated pass constructs a complete `ClassType` for each class (partitioning fields from methods), making instance creation type-checking possible.
- **Two-pass class walking** — the first pass registers all members before any method bodies are walked, enabling forward references.

Results are collected as `Vec<TypeCheckResult>`:
```
line 4: typecheck = on a int and a int -> OK
line 5: typecheck + on a int and a int -> OK
```

## Intermediate Code Generation (Chapter 9)

The `jzero-codegen` crate implements three-address code (TAC) generation via five passes over the typed syntax tree:

### Data structures

**`Address`** — a memory location in the generated program:
```rust
pub enum Address {
    Regional { region: Region, offset: i64 },  // loc:8, global:0, imm:42, strings:0, L3
    Symbol(String),                             // PrintStream__println
}
```

Regions: `Loc` (stack), `Global` (static), `Strings` (read-only string pool), `Lab` (code label), `Class` (heap-relative), `Imm` (immediate value), `Self_` (implicit this pointer).

**`Tac`** — a single three-address instruction:
```rust
pub struct Tac { pub op: Op, pub op1: Option<Address>, pub op2: Option<Address>, pub op3: Option<Address> }
```

Opcodes: `ADD`, `SUB`, `MUL`, `DIV`, `MOD`, `NEG`, `ASN`, `ASIZE`, `LOAD`, `STORE`, `NEWARRAY`, `GOTO`, `LAB`, `BLT`, `BLE`, `BGT`, `BGE`, `BEQ`, `BNE`, `PARM`, `CALL`, `RET`.

**`CodegenContext`** — owns all codegen state that lives outside the AST:
- Label counter (`genlabel()` mints fresh `Address::lab(id)`)
- Per-method local offset counter (`genlocal()` allocates 8-byte slots starting at `loc:8`)
- `node_info: HashMap<u32, NodeInfo>` — parallel structure keyed by `Tree::id` storing `icode`, `addr`, `first`, `follow`, `on_true`, `on_false`
- `var_addrs: HashMap<String, Address>` — variable layout map keyed by `(scope_ptr, name)`
- String pool and global variable list

### Pipeline

```
analyze(tree) → SemanticResult
      ↓
assign_addresses()   Pass 1: walk symtab tree, assign Address to every variable/param
      ↓
genfirst(tree)       Pass 2: post-order, synthesize `first` entry-point labels
      ↓
genfollow(tree)      Pass 3: pre-order, inherit `follow` exit-point labels
      ↓
gentargets(tree)     Pass 4: pre-order, inherit `on_true`/`on_false` for Boolean exprs
      ↓
gencode(tree)        Pass 5: post-order, emit Vec<Tac> for each node
      ↓
emit(tree, ctx)      Render assembler-style text output
```

### Variable layout

Local offsets are assigned per-method starting at `loc:8` (reserving `loc:0` for the implicit self pointer), allocating 8 bytes per slot in declaration order:

| Symbol | Kind | Address |
|--------|------|---------|
| `argv` | Param | `loc:8` |
| `x` | Local | `loc:16` |
| temporaries | Compiler-generated | `loc:24`, `loc:32`, … |

### Key design decisions

- **AST stays clean** — codegen state lives entirely in `CodegenContext::node_info`, keyed by `Tree::id`. No new fields added to `Tree`.
- **`addr_of` returns `Address` (not `Option`)** — falls back to `imm:0` for non-value nodes (operator leaves), eliminating unwrap noise at call sites.
- **`while` guarantees an exit label** — if a `WhileStmt` is the last statement in a method (no natural follow), `gen_while` mints a fresh label rather than leaving `on_false` as `None`.
- **`reemit_condition`** — because `gen_rel_expr` runs before `gen_while` has set `on_false`, the while handler re-emits the condition's branch instructions after setting `on_false`.
- **Method name mangling** — dotted calls like `System.out.println(...)` appear in the tree as `MethodCall#0` with a `FieldAccess` chain as `kids[0]`. The codegen detects this, skips the chain recursion (which would emit spurious LOADs), walks only the args, and emits `CALL PrintStream__println` via `mangle_method`.
- **String pool** — string literals are interned with a `Lab`-region label (printed as `L0:`) and referenced in instructions via a `Strings`-region address (`strings:0`).

## Parser Design Notes

**Why LALRPOP over grmtools/lrpar?** The original grammar has inherent LALR(1) ambiguities. grmtools resolved conflicts in ways that broke dotted method calls like `System.out.println(...)`. LALRPOP's LR(1) lane table algorithm handles more grammars without conflicts, and its explicit conflict reporting made it easier to restructure the grammar correctly.

**Key adaptations:**
- **Left-factored `BlockStmt`** — when an `IDENTIFIER` starts a statement, the parser defers the type-vs-expression decision using `TreeAction` closures.
- **`FieldAccess` instead of `QualifiedName`** — dotted names are represented as nested `FieldAccess` nodes, built left-recursively in `AccessExpr`.
- **`VarDeclarator` nesting for arrays** — `argv[]` produces `VarDeclarator#1(VarDeclarator#0(argv))` to explicitly encode array brackets.
- **`DoubleLit` regex fix** — the original regex matched a bare `.` as a double literal, breaking all method-call parsing. Fixed to require at least one digit on either side of the decimal point.

## References

- [*Build Your Own Programming Language (Edition 2)*](https://a.co/d/0hHvJYWA) — Clinton L. Jeffery
- [Logos](https://github.com/maciejhirsz/logos) — fast lexer generator for Rust
- [LALRPOP](https://github.com/lalrpop/lalrpop) — LR(1) parser generator for Rust