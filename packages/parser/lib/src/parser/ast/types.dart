import 'package:freezed_annotation/freezed_annotation.dart';

import '../../lexer/token.dart';
import '../../syntactic_entity.dart';
import '../../utils.dart';
import 'expressions/expression.dart';
import 'node.dart';

part 'types.freezed.dart';

@freezed
abstract class Type extends AstNode implements _$Type {
  // TODO(JonasWanke): add type arguments
  const factory Type.user(UserType type) = _UserTypeType;
  const factory Type.tuple(TupleType type) = _TupleTypeType;
  const Type._();

  @override
  Iterable<SyntacticEntity> get children =>
      [when(user: (t) => t, tuple: (t) => t)];
}

@freezed
abstract class UserType extends AstNode implements _$UserType {
  const factory UserType({
    @required List<SimpleUserType> simpleTypes,
    @Default(<OperatorToken>[]) List<OperatorToken> dots,
  }) = _UserType;
  const UserType._();

  @override
  Iterable<SyntacticEntity> get children {
    assert(simpleTypes.length == dots.length + 1);
    return interleave(simpleTypes, dots);
  }
}

@freezed
abstract class SimpleUserType extends AstNode implements _$SimpleUserType {
  // TODO(JonasWanke): add type arguments
  const factory SimpleUserType(IdentifierToken name) = _SimpleUserType;
  const SimpleUserType._();

  @override
  Iterable<SyntacticEntity> get children => [name];
}

@freezed
abstract class FunctionType extends AstNode implements _$FunctionType {
  const factory FunctionType({
    // Must be a [GroupedType] or [UserType].
    Type receiver,
    OperatorToken receiverDot,
    @required OperatorToken leftParenthesis,
    @required List<Type> parameterTypes,
    @required List<OperatorToken> parameterCommata,
    @required OperatorToken rightParenthesis,
    @required OperatorToken arrow,
    @required Type returnType,
  }) = _FunctionType;
  const FunctionType._();

  @override
  Iterable<SyntacticEntity> get children => [
        if (receiver != null) receiver,
        if (receiverDot != null) receiverDot,
        leftParenthesis,
        ...interleave(parameterTypes, parameterCommata),
        rightParenthesis,
        arrow,
        returnType,
      ];
}

@freezed
abstract class TupleType extends AstNode implements _$TupleType {
  const factory TupleType({
    @required OperatorToken leftParenthesis,
    @required List<Type> types,
    @required List<OperatorToken> commata,
    @required OperatorToken rightParenthesis,
  }) = _TupleType;
  const TupleType._();

  @override
  Iterable<SyntacticEntity> get children =>
      [leftParenthesis, ...interleave(types, commata), rightParenthesis];
}
