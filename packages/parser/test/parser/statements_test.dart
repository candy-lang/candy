import 'package:parser/src/lexer/lexer.dart';
import 'package:parser/src/parser/ast/expressions/expression.dart';
import 'package:parser/src/parser/ast/statements.dart';
import 'package:parser/src/parser/grammar.dart';
import 'package:parser/src/source_span.dart';
import 'package:test/test.dart';

import 'utils.dart';

void main() {
  setUpAll(ParserGrammar.init);

  group('empty', () {
    testParser('', result: <Statement>[], parser: ParserGrammar.statements);
  });

  group('expressions', () {
    Statement createStatement([int offset = 0]) {
      return Statement.expression(
        Literal<int>(
          IntegerLiteralToken(123, span: SourceSpan.fromStartLength(offset, 3)),
        ),
      );
    }

    forAllMap<String, List<Statement>>(
      table: {
        '123': [createStatement()],
        '123\n123': [createStatement(), createStatement(4)],
        '123;123': [createStatement(), createStatement(4)],
        '123\r\n123': [createStatement(), createStatement(5)],
        '123\n\n;\n123': [createStatement(), createStatement(7)],
        '123;123\n123;;;;;123': [
          createStatement(),
          createStatement(4),
          createStatement(8),
          createStatement(16),
        ],
      },
      tester: (source, result) =>
          testParser(source, result: result, parser: ParserGrammar.statements),
    );
  });
}
