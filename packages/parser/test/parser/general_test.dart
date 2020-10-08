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
        'use crate.utils': UseLine.localAbsolute(
          useKeyword:
              KeywordToken.use(span: SourceSpan(0, 3)) as UseKeywordToken,
          crateKeyword: CrateKeywordToken(span: SourceSpan(4, 9)),
          dots: [
            OperatorToken(OperatorTokenType.dot, span: SourceSpan(9, 10)),
          ],
          pathSegments: [IdentifierToken('utils', span: SourceSpan(10, 15))],
        ),
        'use ...utils.primitives': UseLine.localRelative(
          useKeyword:
              KeywordToken.use(span: SourceSpan(0, 3)) as UseKeywordToken,
          leadingDots: [
            OperatorToken(OperatorTokenType.dot, span: SourceSpan(4, 5)),
            OperatorToken(OperatorTokenType.dot, span: SourceSpan(5, 6)),
            OperatorToken(OperatorTokenType.dot, span: SourceSpan(6, 7)),
          ],
          dots: [
            OperatorToken(OperatorTokenType.dot, span: SourceSpan(12, 13)),
          ],
          pathSegments: [
            IdentifierToken('utils', span: SourceSpan(7, 12)),
            IdentifierToken('primitives', span: SourceSpan(13, 23)),
          ],
        ),
        'use date_time': UseLine.global(
          useKeyword: UseKeywordToken(span: SourceSpan(0, 3)),
          packagePathSegments: [
            IdentifierToken('date_time', span: SourceSpan(4, 13)),
          ],
        ),
        'use firebase/core.errors': UseLine.global(
          useKeyword: UseKeywordToken(span: SourceSpan(0, 3)),
          packagePathSegments: [
            IdentifierToken('firebase', span: SourceSpan(4, 12)),
            IdentifierToken('core', span: SourceSpan(13, 17)),
          ],
          slashes: [
            OperatorToken(OperatorTokenType.slash, span: SourceSpan(12, 13)),
          ],
          dot: OperatorToken(OperatorTokenType.dot, span: SourceSpan(17, 18)),
          moduleName: IdentifierToken('errors', span: SourceSpan(18, 24)),
        ),
      },
      tester: (source, result) =>
          testParser(source, result: result, parser: ParserGrammar.useLine),
    );
  });
}
