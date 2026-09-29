//! The external scanner of the tree-sitter grammar, `src/scanner.c`: the one token a grammar of
//! regular tokens cannot say, the end of a line where a kind of `line_ends` stops. It is zero
//! wide, found before a line break, a `//` comment, the `}` closing the block, or the end of the
//! file, and only where the parser can take it.

use super::LANGUAGE;

/// The hidden rule of the token, as `grammar.js` names it among its externals.
pub(super) const LINE_END: &str = "_line_end";

/// The scanner, for the language named [`LANGUAGE`].
pub(super) fn source() -> String {
    let scanner = format!("tree_sitter_{LANGUAGE}_external_scanner");
    format!(
        r#"#include "tree_sitter/parser.h"

enum TokenType {{ LINE_END }};

void *{scanner}_create(void) {{ return 0; }}
void {scanner}_destroy(void *payload) {{ (void)payload; }}
unsigned {scanner}_serialize(void *payload, char *buffer) {{ (void)payload; (void)buffer; return 0; }}
void {scanner}_deserialize(void *payload, const char *buffer, unsigned length) {{
  (void)payload; (void)buffer; (void)length;
}}

bool {scanner}_scan(void *payload, TSLexer *lexer, const bool *valid) {{
  (void)payload;
  if (!valid[LINE_END]) return false;
  lexer->result_symbol = LINE_END;
  lexer->mark_end(lexer);
  while (lexer->lookahead == ' ' || lexer->lookahead == '\t' || lexer->lookahead == '\r') {{
    lexer->advance(lexer, true);
  }}
  if (lexer->eof(lexer) || lexer->lookahead == '\n' || lexer->lookahead == '}}') return true;
  if (lexer->lookahead != '/') return false;
  lexer->advance(lexer, true);
  return lexer->lookahead == '/';
}}
"#
    )
}
