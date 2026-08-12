import 'parser.dart';
import 'whitespace.dart';

(Parser, List<T>) listOf<T>(
  Parser parser,
  (Parser, T)? Function(Parser) parseItem,
) {
  final items = <T>[];
  while (!parser.isAtEnd) {
    var madeProgress = false;

    if (parseItem(parser) case final itemResult?) {
      parser = itemResult.$1;
      items.add(itemResult.$2);
      madeProgress = true;
    }

    if (whitespace(parser) case final whitespaceResult?) {
      parser = whitespaceResult;
      madeProgress = true;
    }

    if (!madeProgress) break;
  }
  return (parser, items);
}
