import 'package:parser/src/lexer/lexer.dart';
import 'package:parser/src/parser/ast/declarations.dart';
import 'package:parser/src/parser/ast/expressions/expression.dart';
import 'package:parser/src/parser/ast/statements.dart';
import 'package:parser/src/parser/grammar.dart';
import 'package:parser/src/source_span.dart';
import 'package:test/test.dart';

import 'types_test.dart';
import 'utils.dart';

void main() {
  setUpAll(ParserGrammar.init);

  group('FunctionDeclaration', () {
    forAllMap<String, FunctionDeclaration>(
      table: {
        'fun foo(): Foo.Bar {}': FunctionDeclaration(
          funKeyword:
              KeywordToken.fun(span: SourceSpan(0, 3)) as FunKeywordToken,
          name: IdentifierToken('foo', span: SourceSpan(4, 7)),
          leftParenthesis:
              OperatorToken(OperatorTokenType.lparen, span: SourceSpan(7, 8)),
          rightParenthesis:
              OperatorToken(OperatorTokenType.rparen, span: SourceSpan(8, 9)),
          colon:
              OperatorToken(OperatorTokenType.colon, span: SourceSpan(9, 10)),
          returnType: createTypeFooBar(11),
          body: Block(
            leftBrace: OperatorToken(
              OperatorTokenType.lcurl,
              span: SourceSpan(19, 20),
            ),
            rightBrace: OperatorToken(
              OperatorTokenType.rcurl,
              span: SourceSpan(20, 21),
            ),
          ),
        ),
        'external fun foo(bar: Foo.Bar, baz: Foo.Bar = defaultBaz): Foo.Bar':
            FunctionDeclaration(
          modifiers: [FunctionModifierToken.external(span: SourceSpan(0, 8))],
          funKeyword:
              KeywordToken.fun(span: SourceSpan(9, 12)) as FunKeywordToken,
          name: IdentifierToken('foo', span: SourceSpan(13, 16)),
          leftParenthesis: OperatorToken(
            OperatorTokenType.lparen,
            span: SourceSpan(16, 17),
          ),
          valueParameters: [
            FunctionValueParameter(
              name: IdentifierToken('bar', span: SourceSpan(17, 20)),
              colon: OperatorToken(
                OperatorTokenType.colon,
                span: SourceSpan(20, 21),
              ),
              type: createTypeFooBar(22),
            ),
            FunctionValueParameter(
              name: IdentifierToken('baz', span: SourceSpan(31, 34)),
              colon: OperatorToken(
                OperatorTokenType.colon,
                span: SourceSpan(34, 35),
              ),
              type: createTypeFooBar(36),
              equals: OperatorToken(
                OperatorTokenType.equals,
                span: SourceSpan(44, 45),
              ),
              defaultValue: Identifier(
                IdentifierToken('defaultBaz', span: SourceSpan(46, 56)),
              ),
            ),
          ],
          valueParameterCommata: [
            OperatorToken(OperatorTokenType.comma, span: SourceSpan(29, 30)),
          ],
          rightParenthesis:
              OperatorToken(OperatorTokenType.rparen, span: SourceSpan(56, 57)),
          colon:
              OperatorToken(OperatorTokenType.colon, span: SourceSpan(57, 58)),
          returnType: createTypeFooBar(59),
        ),
      },
      tester: (source, result) => testParser(source,
          result: result, parser: ParserGrammar.functionDeclaration),
    );
  });
}
