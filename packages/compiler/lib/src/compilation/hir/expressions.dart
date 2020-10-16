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
  // ignore: non_constant_identifier_names
  const factory Expression.return_(
    DeclarationLocalId id,
    Expression expression,
  ) = ReturnExpression;

  factory Expression.fromJson(Map<String, dynamic> json) =>
      _$ExpressionFromJson(json);
  const Expression._();

  CandyType get type => when(
        identifier: (_, identifier) => identifier.type,
        literal: (_, literal) => literal.type,
        call: (_, __, ___) => null,
        functionCall: (_, target, __) {
          final functionType = target.type as FunctionCandyType;
          return functionType.returnType;
        },
        return_: (_, __) => CandyType.never,
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
  const factory Identifier.module(ModuleId id) = ModuleIdentifier;
  const factory Identifier.trait(DeclarationId id) = TraitIdentifier;
  // ignore: non_constant_identifier_names
  const factory Identifier.class_(DeclarationId id) = ClassIdentifier;

  /// A property or function.
  const factory Identifier.property(
    Expression target,
    String name,
    CandyType type,
  ) = PropertyIdentifier;

  const factory Identifier.parameter(
    String name,
    int disambiguator,
    CandyType type,
  ) = ParameterIdentifier;

  factory Identifier.fromJson(Map<String, dynamic> json) =>
      _$IdentifierFromJson(json);
  const Identifier._();

  CandyType get type => when(
        this_: (type) => type,
        super_: (type) => type,
        it: (type) => type,
        field: (type) => type,
        trait: (_) => CandyType.declaration,
        class_: (_) => CandyType.declaration,
        module: (_) => CandyType.declaration,
        property: (_, __, type) => type,
        parameter: (_, __, type) => type,
      );
}

@freezed
abstract class Literal implements _$Literal {
  // ignore: avoid_positional_boolean_parameters
  const factory Literal.boolean(bool value) = BoolLiteral;
  const factory Literal.integer(int value) = IntLiteral;
  const factory Literal.string(List<StringLiteralPart> parts) = StringLiteral;

  factory Literal.fromJson(Map<String, dynamic> json) =>
      _$LiteralFromJson(json);
  const Literal._();

  CandyType get type => map(
        boolean: (_) => CandyType.bool,
        integer: (_) => CandyType.int,
        string: (_) => CandyType.string,
      );
}

@freezed
abstract class StringLiteralPart implements _$StringLiteralPart {
  const factory StringLiteralPart.literal(String value) =
      LiteralStringLiteralPart;
  const factory StringLiteralPart.interpolated(Expression value) =
      InterpolatedStringLiteralPart;

  factory StringLiteralPart.fromJson(Map<String, dynamic> json) =>
      _$StringLiteralPartFromJson(json);
  const StringLiteralPart._();
}
