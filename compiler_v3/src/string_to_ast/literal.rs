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
define_literal!(colon_equals_sign, ":=");
define_literal!(opening_parenthesis, "(");
define_literal!(closing_parenthesis, ")");
define_literal!(opening_bracket, "[");
define_literal!(closing_bracket, "]");
define_literal!(opening_curly_brace, "{");
define_literal!(closing_curly_brace, "}");
define_literal!(bar, "|");
define_literal!(arrow, "->");
define_literal!(double_quote, "\"");
define_literal!(octothorpe, "#");
