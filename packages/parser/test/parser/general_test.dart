import 'package:parser/src/lexer/lexer.dart';
import 'package:parser/src/parser/ast/general.dart';
import 'package:parser/src/parser/grammar.dart';
import 'package:parser/src/source_span.dart';
import 'package:test/test.dart';

import 'utils.dart';

void main() {
  setUpAll(ParserGrammar.init);

  group('use lines', () {
    forAllMap<String, UseLine>(
      table: {
        'use date_time': UseLine(
          useKeyword:
              KeywordToken.use(span: SourceSpan(0, 3)) as UseKeywordToken,
          packageName: IdentifierToken('date_time', span: SourceSpan(4, 13)),
        ),
        'use firebase/core': UseLine(
          useKeyword:
              KeywordToken.use(span: SourceSpan(0, 3)) as UseKeywordToken,
          publisherName: IdentifierToken('firebase', span: SourceSpan(4, 12)),
          slash:
              OperatorToken(OperatorTokenType.slash, span: SourceSpan(12, 13)),
          packageName: IdentifierToken('core', span: SourceSpan(13, 17)),
        ),
        'use serialization.json': UseLine(
          useKeyword:
              KeywordToken.use(span: SourceSpan(0, 3)) as UseKeywordToken,
          packageName:
              IdentifierToken('serialization', span: SourceSpan(4, 17)),
          dot: OperatorToken(OperatorTokenType.dot, span: SourceSpan(17, 18)),
          moduleName: IdentifierToken('json', span: SourceSpan(18, 22)),
        ),
      },
      tester: (source, result) =>
          testParser(source, result: result, parser: ParserGrammar.useLine),
    );
  });
}
