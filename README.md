# jzero-rs

A compiler + bytecode VM for **Jzero** — a small subset of Java — written entirely in Rust.

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
| Bytecode Generation | `jzero-codegen` | Ch. 13 | ✅ Done |
| Bytecode Interpreter / VM | `jzero-vm` | Ch. 12 | ✅ Done |

Chapters 10 (IDE/syntax coloring) and 11 (transpiler) were intentionally skipped as they
are detours unrelated to the core compiler pipeline.

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
│   ├── jzero-codegen/      # TAC + bytecode generation
│   ├── jzero-vm/           # Bytecode interpreter
│   └── jzero-cli/          # CLI tool (j0)
└── tests/
    ├── hello.java           # Minimal hello-world test
    └── hello_loop.java      # End-to-end golden test (loop + array + I/O)
```

Clean one-way dependency chain:

```
jzero-lexer → jzero-symtab → jzero-ast → jzero-parser → jzero-semantic → jzero-codegen → jzero-vm
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
| `byc.java` + `j0machine.java` | `jzero-codegen` + `jzero-vm` | Bytecode format + stack-machine VM |

## Building & Testing

```bash
# Build everything
cargo build

# Run all tests (single-threaded to avoid global ID counter races)
cargo test --workspace -- --test-threads=1

# Parse a file and visualize the syntax tree
cargo run --bin j0 -- tests/hello.java --png

# Print TAC intermediate code (Chapter 9)
cargo run --bin j0 -- tests/hello_loop.java --codegen

# Compile to bytecode and print assembler listing (Chapter 13)
cargo run --bin j0 -- tests/hello_loop.java --bytecode

# Compile and execute in the VM (Chapters 12+13)
cargo run --bin j0 -- tests/hello_loop.java --run a b c d e
```

## End-to-End Example

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

Running `j0 tests/hello_loop.java --codegen` produces TAC intermediate code:

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

Running `j0 tests/hello_loop.java --run a b c d e` executes the program end-to-end:

```
hello, jzero!
hello, jzero!
hello, jzero!
hello, jzero!
no errors
```

## Semantic Analysis (Chapters 6–8)

### Symbol table construction (Ch. 6)

A two-pass tree traversal builds one `SymTab` per scope:

- **First pass** registers all field and method signatures, enabling forward references within a class.
- **Second pass** walks method bodies to insert parameters and local variables.
- `System.out.println` is pre-registered in the global scope.
- Each `Tree` node gets its `stab` field set to the nearest enclosing scope (inherited, top-down).

### Type checking (Ch. 7–8)

Three cooperative functions — `check_type`, `check_kids`, `check_types` — perform a selective post-order traversal of method bodies.

Key design decisions:
- **`TypeInfo` enum** replaces the book's `typeinfo` class hierarchy — `Base`, `Array`, `Method`, `Class` variants with exhaustive matching.
- **`"return"` dummy symbol** — each method's stab gets a `"return"` entry with the declared return type. `ReturnStmt` nodes look it up directly.
- **`mkcls` pass** — after symbol tables are fully built, a dedicated pass constructs a complete `ClassType` for each class.

## Intermediate Code Generation (Chapter 9)

Five passes over the typed syntax tree produce TAC:

```
assign_addresses → genfirst → genfollow → gentargets → gencode → emit
```

See the README architecture section for full details on `Address`, `Tac`, and `CodegenContext`.

## Bytecode Generation + VM (Chapters 12+13)

### Bytecode format

Each instruction is a fixed 8-byte word: `[opcode:1][region:1][operand:6, little-endian]`.

The `.j0` binary file layout:
```
Word 0:   magic   "Jzero!!\0"
Word 1:   version "1.0\0\0\0\0\0"
Word 2:   first-instruction word offset
Word 3…:  data section (string literals, NUL-terminated)
Word N…:  startup sequence + instructions
```

The startup sequence calls `main` with the real `argc` (number of CLI arguments passed after `--run`), so `argv.length` behaves correctly without any hardcoding.

### Instruction set (23 opcodes)

`HALT NOOP ADD SUB MUL DIV MOD NEG PUSH POP CALL RETURN GOTO BIF LT LE GT GE EQ NEQ LOCAL LOAD STORE`

### TAC → bytecode translation

| TAC | Bytecode |
|---|---|
| `ADD op1,op2,op3` | `PUSH op2, PUSH op3, ADD, POP op1` |
| `ASN op1,op2` | `PUSH op2, POP op1` |
| `BGT label,op2,op3` | `PUSH op2, PUSH op3, GT, BIF label` |
| `PARM arg` + `CALL fn,n` | `PUSH fn_addr, PUSH arg, …, CALL n` |
| `Proc` (method entry) | `LOCAL n` (pre-allocates local slots) |

Label resolution is two-pass: pass 1 records byte offsets, pass 2 patches branch targets. All GOTO/BIF targets are then relocated by `code_base_bytes` to be absolute offsets from word 0.

### VM calling convention

Saved registers `(ip, bp, fn_slot)` are kept in an off-stack `call_stack: Vec<(usize, i64, i64)>`, leaving the data stack clean for locals:

```
stack[bp+0] = fn_addr  (loc:0)
stack[bp+1] = arg0     (loc:8)
stack[bp+2] = local0   (loc:16)
…
```

`LOCAL n` pre-allocates all local slots at function entry so expression temporaries never overwrite them.

## Parser Design Notes

**Why LALRPOP over grmtools/lrpar?** The original grammar has inherent LALR(1) ambiguities. grmtools resolved conflicts silently in ways that broke dotted method calls like `System.out.println(...)`. LALRPOP's LR(1) lane table algorithm handles more grammars without conflicts, and its explicit conflict reporting made it easier to restructure the grammar correctly.

## References

- [*Build Your Own Programming Language (Edition 2)*](https://a.co/d/0hHvJYWA) — Clinton L. Jeffery
- [Logos](https://github.com/maciejhirsz/logos) — fast lexer generator for Rust
- [LALRPOP](https://github.com/lalrpop/lalrpop) — LR(1) parser generator for Rust