use super::{parser::Parser, whitespace::whitespace};

pub fn list_of<T>(
    mut parser: Parser,
    mut parse_item: impl FnMut(Parser) -> Option<(Parser, T)>,
) -> (Parser, Vec<T>) {
    let mut items = vec![];
    while !parser.is_at_end() {
        let mut made_progress = false;

        if let Some((new_parser, function)) = parse_item(parser) {
            parser = new_parser;
            items.push(function);
            made_progress = true;
        }

        if let Some(new_parser) = whitespace(parser) {
            parser = new_parser;
            made_progress = true;
        }

        if !made_progress {
            break;
        }
    }
    (parser, items)
}
