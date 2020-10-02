import 'package:freezed_annotation/freezed_annotation.dart';

import 'ids.dart';

part 'expressions.freezed.dart';
part 'expressions.g.dart';

@freezed
abstract class Expression implements _$Expression {
  const factory Expression.identifier(
    DeclarationLocalId id,
    Identifier identifier,
  ) = IdentifierExpression;
  const factory Expression.literal(DeclarationLocalId id, Literal literal) =
      LiteralExpression;
  const factory Expression.call(
    DeclarationLocalId id,
    Expression target,
    List<ValueArgument> valueArguments,
  ) = CallExpression;

  factory Expression.fromJson(Map<String, dynamic> json) =>
      _$ExpressionFromJson(json);
  const Expression._();
}

@freezed
abstract class Identifier implements _$Identifier {
  // ignore: non_constant_identifier_names
  const factory Identifier.this_() = ThisIdentifier;
  // ignore: non_constant_identifier_names
  const factory Identifier.super_() = SuperIdentifier;
  const factory Identifier.it() = ItIdentifier;
  const factory Identifier.field() = FieldIdentifier;
  const factory Identifier.trait(DeclarationId id) = TraitIdentifier;
  // ignore: non_constant_identifier_names
  const factory Identifier.class_(DeclarationId id) = ClassIdentifier;
  const factory Identifier.property(DeclarationId id) = PropertyIdentifier;
  const factory Identifier.parameter(String name, int disambiguator) =
      ParameterIdentifier;
  @deprecated
  const factory Identifier.printFunction() = PrintFunctionIdentifier;

  factory Identifier.fromJson(Map<String, dynamic> json) =>
      _$IdentifierFromJson(json);
  const Identifier._();
}

@freezed
abstract class Literal implements _$Literal {
  // ignore: avoid_positional_boolean_parameters
  const factory Literal.boolean(bool value) = BooleanLiteral;
  const factory Literal.integer(int value) = IntegerLiteral;

  factory Literal.fromJson(Map<String, dynamic> json) =>
      _$LiteralFromJson(json);
  const Literal._();
}

@freezed
abstract class ValueArgument implements _$ValueArgument {
  const factory ValueArgument({
    String name,
    @required Expression expression,
  }) = _ValueArgument;
  factory ValueArgument.fromJson(Map<String, dynamic> json) =>
      _$ValueArgumentFromJson(json);
  const ValueArgument._();
}
