import 'package:parser/src/lexer/lexer.dart';
import 'package:parser/src/parser/ast/expressions/expressions.dart';
import 'package:parser/src/parser/ast/statements.dart';
import 'package:parser/src/parser/grammar.dart';
import 'package:parser/src/source_span.dart';
import 'package:test/test.dart';

import 'utils.dart';

void main() {
  setUp(ParserGrammar.init);

  group('empty', () {
    testParser('', result: <Statement>[], parser: ParserGrammar.statements);
  });

  group('expressions', () {
    forAllMap<String, List<Statement>>(
      table: {
        '123': [createStatement123(0)],
        '123\n123': [createStatement123(0), createStatement123(1, 4)],
        '123;123': [createStatement123(0), createStatement123(1, 4)],
        '123\r\n123': [createStatement123(0), createStatement123(1, 5)],
        '123\n\n;\n123': [createStatement123(0), createStatement123(1, 7)],
        '123;123\n123;;;;;123': [
          createStatement123(0),
          createStatement123(1, 4),
          createStatement123(2, 8),
          createStatement123(3, 16),
        ],
      },
      tester: (source, result) =>
          testParser(source, result: result, parser: ParserGrammar.statements),
    );
  });

  group('blocks', () {
    forAllMap<String, Block>(
      table: {
        '{}': Block(
          0,
          leftBrace:
              OperatorToken(OperatorTokenType.lcurl, span: SourceSpan(0, 1)),
          statements: [],
          rightBrace:
              OperatorToken(OperatorTokenType.rcurl, span: SourceSpan(1, 2)),
        ),
        '{  \n\n\n123\n 123;\n}': Block(
          2,
          leftBrace:
              OperatorToken(OperatorTokenType.lcurl, span: SourceSpan(0, 1)),
          statements: [createStatement123(0, 6), createStatement123(1, 11)],
          rightBrace:
              OperatorToken(OperatorTokenType.rcurl, span: SourceSpan(16, 17)),
        ),
      },
      tester: (source, result) =>
          testParser(source, result: result, parser: ParserGrammar.block),
    );
  });
}

Statement createStatement123(int id, [int offset = 0]) {
  return Literal<int>(
    id,
    IntegerLiteralToken(123, span: SourceSpan.fromStartLength(offset, 3)),
  );
}
