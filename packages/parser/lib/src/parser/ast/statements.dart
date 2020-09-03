import 'package:freezed_annotation/freezed_annotation.dart';

import '../../lexer/token.dart';
import '../../syntactic_entity.dart';
import 'expressions/expression.dart';
import 'node.dart';

part 'statements.freezed.dart';

@freezed
abstract class Block extends AstNode implements _$Block {
  const factory Block({
    @required OperatorToken leftBrace,
    @Default(<Statement>[]) List<Statement> statements,
    @required OperatorToken rightBrace,
  }) = _Block;
  const Block._();

  @override
  Iterable<SyntacticEntity> get children => [
        leftBrace,
        ...statements,
        rightBrace,
      ];
}

@freezed
abstract class Statement extends AstNode implements _$Statement {
  const factory Statement.expression(Expression expression) =
      _ExpressionStatement;
  const Statement._();

  @override
  Iterable<SyntacticEntity> get children => [when(expression: (e) => e)];
}
