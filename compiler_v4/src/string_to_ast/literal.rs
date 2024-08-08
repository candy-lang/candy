use super::parser::Parser;
use tracing::instrument;

macro_rules! define_literal {
    ($name:ident, $string:expr) => {
        #[instrument(level = "trace")]
        pub fn $name(parser: Parser) -> Option<Parser> {
            parser.consume_literal($string)
        }
    };
}

define_literal!(equals_sign, "=");
define_literal!(comma, ",");
define_literal!(dot, ".");
define_literal!(colon, ":");
define_literal!(opening_parenthesis, "(");
define_literal!(closing_parenthesis, ")");
define_literal!(opening_curly_brace, "{");
define_literal!(closing_curly_brace, "}");
define_literal!(double_quote, "\"");
define_literal!(octothorpe, "#");
define_literal!(arrow, "=>");

pub const KEYWORDS: &[&str] = &["struct", "enum", "fun", "let", "switch"];
define_literal!(struct_keyword, "struct");
define_literal!(enum_keyword, "enum");
define_literal!(fun_keyword, "fun");
define_literal!(let_keyword, "let");
define_literal!(switch_keyword, "switch");
