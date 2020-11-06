import 'package:freezed_annotation/freezed_annotation.dart';

import '../../lexer/token.dart';
import '../../syntactic_entity.dart';
import '../../utils.dart';
import '../../visitor.dart';
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
    TypeArguments arguments,
  }) = _UserType;
  const UserType._();

  @override
  R accept<R>(AstVisitor<R> visitor) => visitor.visitUserType(this);

  @override
  Iterable<SyntacticEntity> get children {
    assert(simpleTypes.length == dots.length + 1);
    return [
      ...interleave(simpleTypes, dots),
      if (arguments != null) arguments,
    ];
  }
}

@freezed
abstract class SimpleUserType extends Type implements _$SimpleUserType {
  // TODO(JonasWanke): add type arguments
  const factory SimpleUserType(IdentifierToken name) = _SimpleUserType;
  const SimpleUserType._();

  @override
  R accept<R>(AstVisitor<R> visitor) => visitor.visitSimpleUserType(this);

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
  R accept<R>(AstVisitor<R> visitor) => visitor.visitGroupType(this);

  @override
  Iterable<SyntacticEntity> get children =>
      [leftParenthesis, type, rightParenthesis];
}

@freezed
abstract class FunctionType extends Type implements _$FunctionType {
  const factory FunctionType({
    // Must be a [UserType], [GroupType], or [TupleType].
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
  R accept<R>(AstVisitor<R> visitor) => visitor.visitFunctionType(this);

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
  R accept<R>(AstVisitor<R> visitor) => visitor.visitTupleType(this);

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
  R accept<R>(AstVisitor<R> visitor) => visitor.visitUnionType(this);

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
  R accept<R>(AstVisitor<R> visitor) => visitor.visitIntersectionType(this);

  @override
  Iterable<SyntacticEntity> get children => [leftType, ampersand, rightType];
}

@freezed
abstract class TypeParameters extends AstNode implements _$TypeParameters {
  const factory TypeParameters({
    @required OperatorToken leftAngle,
    @Default(<TypeParameter>[]) List<TypeParameter> parameters,
    @Default(<OperatorToken>[]) List<OperatorToken> commata,
    @required OperatorToken rightAngle,
  }) = _TypeParameters;
  const TypeParameters._();

  @override
  R accept<R>(AstVisitor<R> visitor) => visitor.visitTypeParameters(this);

  @override
  Iterable<SyntacticEntity> get children =>
      [leftAngle, ...interleave(parameters, commata), rightAngle];
}

@freezed
abstract class TypeParameter extends AstNode implements _$TypeParameter {
  const factory TypeParameter({
    @Default(<ModifierToken>[]) List<ModifierToken> modifiers,
    @required IdentifierToken name,
    OperatorToken colon,
    Type bound,
  }) = _TypeParameter;
  const TypeParameter._();

  @override
  R accept<R>(AstVisitor<R> visitor) => visitor.visitTypeParameter(this);

  @override
  Iterable<SyntacticEntity> get children =>
      [...modifiers, name, if (colon != null) colon, if (bound != null) bound];
}

@freezed
abstract class TypeArguments extends AstNode implements _$TypeArguments {
  const factory TypeArguments({
    @required OperatorToken leftAngle,
    @Default(<TypeArgument>[]) List<TypeArgument> arguments,
    @Default(<OperatorToken>[]) List<OperatorToken> commata,
    @required OperatorToken rightAngle,
  }) = _TypeArguments;
  const TypeArguments._();

  @override
  R accept<R>(AstVisitor<R> visitor) => visitor.visitTypeArguments(this);

  @override
  Iterable<SyntacticEntity> get children =>
      [leftAngle, ...interleave(arguments, commata), rightAngle];
}

@freezed
abstract class TypeArgument extends AstNode implements _$TypeArgument {
  const factory TypeArgument({
    @Default(<ModifierToken>[]) List<ModifierToken> modifiers,
    @required Type type,
  }) = _TypeArgument;
  const TypeArgument._();

  @override
  R accept<R>(AstVisitor<R> visitor) => visitor.visitTypeArgument(this);

  @override
  Iterable<SyntacticEntity> get children => [...modifiers, type];
}
