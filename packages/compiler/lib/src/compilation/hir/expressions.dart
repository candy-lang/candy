import 'package:freezed_annotation/freezed_annotation.dart';

import 'ids.dart';
import 'type.dart';

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
    List<Expression> valueArguments,
  ) = CallExpression;
  const factory Expression.functionCall(
    DeclarationLocalId id,
    IdentifierExpression target,
    Map<String, Expression> valueArguments,
  ) = FunctionCallExpression;

  factory Expression.fromJson(Map<String, dynamic> json) =>
      _$ExpressionFromJson(json);
  const Expression._();

  CandyType get type => map(
        identifier: (it) => it.type,
        literal: (it) => it.type,
        call: (it) => null,
        functionCall: (it) {
          final functionType = it.target.type as FunctionCandyType;
          return functionType.returnType;
        },
      );
}

@freezed
abstract class Identifier implements _$Identifier {
  // ignore: non_constant_identifier_names
  const factory Identifier.this_(CandyType type) = ThisIdentifier;
  // ignore: non_constant_identifier_names
  const factory Identifier.super_(CandyType type) = SuperIdentifier;
  const factory Identifier.it(CandyType type) = ItIdentifier;
  const factory Identifier.field(CandyType type) = FieldIdentifier;
  const factory Identifier.trait(DeclarationId id, CandyType type) =
      TraitIdentifier;
  // ignore: non_constant_identifier_names
  const factory Identifier.class_(DeclarationId id, CandyType type) =
      ClassIdentifier;

  /// A property or function.
  const factory Identifier.property(DeclarationId id, CandyType type) =
      PropertyIdentifier;

  const factory Identifier.parameter(
    String name,
    int disambiguator,
    CandyType type,
  ) = ParameterIdentifier;

  factory Identifier.fromJson(Map<String, dynamic> json) =>
      _$IdentifierFromJson(json);
  const Identifier._();
}

@freezed
abstract class Literal implements _$Literal {
  // ignore: avoid_positional_boolean_parameters
  const factory Literal.boolean(bool value) = BoolLiteral;
  const factory Literal.integer(int value) = IntLiteral;

  factory Literal.fromJson(Map<String, dynamic> json) =>
      _$LiteralFromJson(json);
  const Literal._();

  CandyType get type => map(
        boolean: (_) => CandyType.bool,
        integer: (_) => CandyType.int,
      );
}
