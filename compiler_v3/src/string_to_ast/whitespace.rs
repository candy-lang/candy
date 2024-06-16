use super::{literal::octothorpe, parser::Parser};
use extension_trait::extension_trait;
use tracing::instrument;

#[instrument(level = "trace")]
pub fn whitespace(mut parser: Parser) -> Option<Parser> {
    let start_offset = parser.offset();
    loop {
        let parser_from_iteration_start = parser;

        // Whitespace
        // TODO: report tabs as errors
        parser = parser
            .consume_while(|c| matches!(c, ' ' | '\r' | '\n' | '\t'))
            .map_or(parser, |(new_parser, _)| new_parser);

        // Comment
        parser = comment(parser).unwrap_or(parser);

        if parser == parser_from_iteration_start {
            break;
        }
    }
    Some(parser).take_if(|parser| parser.offset() != start_offset)
}

#[instrument(level = "trace")]
fn comment(parser: Parser) -> Option<Parser> {
    octothorpe(parser)?
        .consume_while(|c| !matches!(c, '\n' | '\r'))
        .map_or(None, |(parser, _)| Some(parser))
}

#[extension_trait]
pub impl<'s> AndTrailingWhitespace<'s> for Parser<'s> {
    #[must_use]
    fn and_trailing_whitespace(self) -> Self {
        whitespace(self).unwrap_or(self)
    }
}
#[extension_trait]
pub impl<T, 's> ValueAndTrailingWhitespace<'s> for (Parser<'s>, T) {
    #[must_use]
    fn and_trailing_whitespace(self) -> Self {
        (self.0.and_trailing_whitespace(), self.1)
    }
}
#[extension_trait]
pub impl<'s> OptionAndTrailingWhitespace<'s> for Option<Parser<'s>> {
    #[must_use]
    fn and_trailing_whitespace(self) -> Self {
        self.map(AndTrailingWhitespace::and_trailing_whitespace)
    }
}
#[extension_trait]
pub impl<T, 's> OptionWithValueAndTrailingWhitespace<'s> for Option<(Parser<'s>, T)> {
    #[must_use]
    fn and_trailing_whitespace(self) -> Self {
        self.map(ValueAndTrailingWhitespace::and_trailing_whitespace)
    }
}
