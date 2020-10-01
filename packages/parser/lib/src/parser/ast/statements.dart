import 'package:freezed_annotation/freezed_annotation.dart';

import '../../lexer/token.dart';
import '../../syntactic_entity.dart';
import 'node.dart';

part 'statements.freezed.dart';

abstract class Statement extends AstNode {
  const Statement();

  int get id;
}

@freezed
abstract class Block extends Statement implements _$Block {
  const factory Block(
    int id, {
    @required OperatorToken leftBrace,
    @Default(<Statement>[]) List<Statement> statements,
    @required OperatorToken rightBrace,
  }) = _Block;
  const Block._();

  @override
  Iterable<SyntacticEntity> get children =>
      [leftBrace, ...statements, rightBrace];
}
