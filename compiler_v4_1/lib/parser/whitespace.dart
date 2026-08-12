import 'literal.dart';
import 'parser.dart';

Parser? whitespace(Parser parser) {
  final startOffset = parser.offset;
  while (true) {
    final parserFromIterationStart = parser;

    // Whitespace
    // TODO(JonasWanke): report tabs as errors
    parser = parser
        .consumeWhile(
          (char) => char == ' ' || char == '\r' || char == '\n' || char == '\t',
        )
        .$1;

    // Comment
    parser = comment(parser) ?? parser;

    if (parser == parserFromIterationStart) {
      break;
    }
  }
  return parser.offset != startOffset ? parser : null;
}

Parser? comment(Parser parser) =>
    octothorpe(parser)?.consumeWhile((char) => char != '\n' && char != '\r').$1;

extension ParserAndTrailingWhitespace on Parser {
  Parser andTrailingWhitespace() => whitespace(this) ?? this;
}

extension NullableParserAndTrailingWhitespace on Parser? {
  Parser? andTrailingWhitespace() => this?.andTrailingWhitespace();
}

extension ParserAndValueAndTrailingWhitespace<T> on (Parser, T) {
  (Parser, T) andTrailingWhitespace() =>
      (this.$1.andTrailingWhitespace(), this.$2);
}

extension NullableParserAndValueAndTrailingWhitespace<T> on (Parser, T)? {
  (Parser, T)? andTrailingWhitespace() => this?.andTrailingWhitespace();
}
