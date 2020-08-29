import 'package:freezed_annotation/freezed_annotation.dart';

import '../../lexer/token.dart';
import '../../syntactic_entity.dart';
import '../../utils.dart';
import 'node.dart';

part 'types.freezed.dart';

abstract class Type extends AstNode {
  const Type();
}

@freezed
abstract class UserType extends Type implements _$UserType {
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
abstract class SimpleUserType extends Type implements _$SimpleUserType {
  // TODO(JonasWanke): add type arguments
  const factory SimpleUserType(IdentifierToken name) = _SimpleUserType;
  const SimpleUserType._();

  @override
  Iterable<SyntacticEntity> get children => [name];
}

@freezed
abstract class GroupType extends Type implements _$GroupType {
  const factory GroupType({
    @required OperatorToken leftParenthesis,
    @required Type type,
    @required OperatorToken rightParenthesis,
  }) = _GroupType;
  const GroupType._();

  @override
  Iterable<SyntacticEntity> get children =>
      [leftParenthesis, type, rightParenthesis];
}

@freezed
abstract class FunctionType extends Type implements _$FunctionType {
  const factory FunctionType({
    // Must be a wrapped [GroupType] or [UserType].
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
abstract class TupleType extends Type implements _$TupleType {
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

@freezed
abstract class UnionType extends Type implements _$UnionType {
  const factory UnionType({
    @required Type leftType,
    @required OperatorToken bar,
    @required Type rightType,
  }) = _UnionType;
  const UnionType._();

  @override
  Iterable<SyntacticEntity> get children => [leftType, bar, rightType];
}

@freezed
abstract class IntersectionType extends Type implements _$IntersectionType {
  const factory IntersectionType({
    @required Type leftType,
    @required OperatorToken ampersand,
    @required Type rightType,
  }) = _IntersectionType;
  const IntersectionType._();

  @override
  Iterable<SyntacticEntity> get children => [leftType, ampersand, rightType];
}
