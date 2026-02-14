use cfgrammar::yacc::{YaccKind, YaccOriginalActionKind};
use lrlex::{CTTokenMapBuilder, DefaultLexerTypes};
use lrpar::CTParserBuilder;

fn main() {
    // Build the parser from our .y grammar.
    // We use Original/NoAction because Chapter 4 only needs accept/reject + error recovery,
    // no semantic actions yet.
    let ctp = CTParserBuilder::<DefaultLexerTypes<u8>>::new()
        .yacckind(YaccKind::Original(YaccOriginalActionKind::NoAction))
        .error_on_conflicts(false)   // allow shift/reduce & reduce/reduce conflicts (like yacc)
        .warnings_are_errors(false)  // don't fail on grammar warnings either
        .grammar_in_src_dir("jzero.y")
        .unwrap()
        .build()
        .unwrap();

    // Generate a token map module so we can map our Logos tokens → lrpar token IDs.
    // This creates constants like T_PUBLIC, T_CLASS, T_IDENTIFIER, etc.
    CTTokenMapBuilder::<u8>::new("token_map", ctp.token_map())
        .build()
        .unwrap();
}