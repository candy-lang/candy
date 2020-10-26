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

  const factory Expression.property(
    DeclarationLocalId id,
    String name,
    CandyType type,
    Expression initializer, {
    @required bool isMutable,
  }) = PropertyExpression;

  const factory Expression.navigation(
    DeclarationLocalId id,
    Expression target,
    DeclarationId property,
    CandyType type,
  ) = NavigationExpression;

  const factory Expression.call(
    DeclarationLocalId id,
    Expression target,
    List<Expression> valueArguments,
  ) = CallExpression;
  const factory Expression.functionCall(
    DeclarationLocalId id,
    Expression target,
    Map<String, Expression> valueArguments,
  ) = FunctionCallExpression;
  const factory Expression.loop(
    DeclarationLocalId id,
    List<Expression> body,
    CandyType type,
  ) = LoopExpression;
  // ignore: non_constant_identifier_names
  const factory Expression.while_(
    DeclarationLocalId id,
    Expression condition,
    List<Expression> body,
    CandyType type,
  ) = WhileExpression;
  // ignore: non_constant_identifier_names
  const factory Expression.return_(
    DeclarationLocalId id,
    DeclarationLocalId scopeId, [
    Expression expression,
  ]) = ReturnExpression;
  // ignore: non_constant_identifier_names
  const factory Expression.break_(
    DeclarationLocalId id,
    DeclarationLocalId scopeId, [
    Expression expression,
  ]) = BreakExpression;
  // ignore: non_constant_identifier_names
  const factory Expression.continue_(
    DeclarationLocalId id,
    DeclarationLocalId scopeId,
  ) = ContinueExpression;
  const factory Expression.assignment(
    DeclarationLocalId id,
    IdentifierExpression left,
    Expression right,
  ) = AssignmentExpression;

  factory Expression.fromJson(Map<String, dynamic> json) =>
      _$ExpressionFromJson(json);
  const Expression._();

  CandyType get type => when(
        identifier: (_, identifier) => identifier.type,
        literal: (_, literal) => literal.type,
        property: (_, __, type, ___, ____) => type,
        navigation: (_, __, ___, type) => type,
        call: (_, __, ___) => null,
        functionCall: (_, target, __) {
          final functionType = target.type as FunctionCandyType;
          return functionType.returnType;
        },
        return_: (_, __, ___) => CandyType.never,
        loop: (_, __, type) => type,
        while_: (_, __, ___, type) => type,
        break_: (_, __, ___) => CandyType.never,
        continue_: (_, __) => CandyType.never,
        assignment: (_, left, __) => left.type,
      );

  T accept<T>(ExpressionVisitor<T> visitor) => map(
        identifier: (e) => visitor.visitIdentifierExpression(e),
        literal: (e) => visitor.visitLiteralExpression(e),
        property: (e) => visitor.visitPropertyExpression(e),
        navigation: (e) => visitor.visitNavigationExpression(e),
        call: (e) => visitor.visitCallExpression(e),
        functionCall: (e) => visitor.visitFunctionCallExpression(e),
        return_: (e) => visitor.visitReturnExpression(e),
        loop: (e) => visitor.visitLoopExpression(e),
        while_: (e) => visitor.visitWhileExpression(e),
        break_: (e) => visitor.visitBreakExpression(e),
        continue_: (e) => visitor.visitContinueExpression(e),
        assignment: (e) => visitor.visitAssignmentExpression(e),
      );
}

@freezed
abstract class Identifier implements _$Identifier {
  // ignore: non_constant_identifier_names
  const factory Identifier.this_() = ThisIdentifier;
  // ignore: non_constant_identifier_names
  const factory Identifier.super_(UserCandyType type) = SuperIdentifier;
  const factory Identifier.module(ModuleId id) = ModuleIdentifier;
  const factory Identifier.trait(DeclarationId id) = TraitIdentifier;
  // ignore: non_constant_identifier_names
  const factory Identifier.class_(DeclarationId id) = ClassIdentifier;

  const factory Identifier.parameter(
    DeclarationLocalId id,
    String name,
    CandyType type,
  ) = ParameterIdentifier;

  /// A property or function.
  const factory Identifier.property(
    DeclarationId id,
    CandyType type, [
    Expression target,
  ]) = PropertyIdentifier;
  const factory Identifier.localProperty(
    DeclarationLocalId id,
    String name,
    CandyType type,
  ) = LocalPropertyIdentifier;

  factory Identifier.fromJson(Map<String, dynamic> json) =>
      _$IdentifierFromJson(json);
  const Identifier._();

  CandyType get type => when(
        this_: () => CandyType.this_(),
        super_: (type) => type,
        trait: (_) => CandyType.declaration,
        class_: (_) => CandyType.declaration,
        module: (_) => CandyType.declaration,
        parameter: (_, __, type) => type,
        property: (_, type, __) => type,
        localProperty: (_, __, type) => type,
      );
}

@freezed
abstract class Literal implements _$Literal {
  // ignore: avoid_positional_boolean_parameters
  const factory Literal.boolean(bool value) = BoolLiteral;
  const factory Literal.integer(int value) = IntLiteral;
  const factory Literal.string(List<StringLiteralPart> parts) = StringLiteral;
  const factory Literal.lambda(
    List<LambdaLiteralParameter> parameters,
    List<Expression> expressions,
    CandyType returnType, [
    CandyType receiverType,
  ]) = LambdaLiteral;

  factory Literal.fromJson(Map<String, dynamic> json) =>
      _$LiteralFromJson(json);
  const Literal._();

  CandyType get type => when(
        boolean: (_) => CandyType.bool,
        integer: (_) => CandyType.int,
        string: (_) => CandyType.string,
        lambda: (parameters, _, returnType, receiverType) => CandyType.function(
          receiverType: receiverType,
          parameterTypes: parameters.map((p) => p.type).toList(),
          returnType: returnType,
        ),
      );
}

@freezed
abstract class LambdaLiteralParameter implements _$LambdaLiteralParameter {
  const factory LambdaLiteralParameter(String name, CandyType type) =
      _LambdaLiteralParameter;
  factory LambdaLiteralParameter.fromJson(Map<String, dynamic> json) =>
      _$LambdaLiteralParameterFromJson(json);
  const LambdaLiteralParameter._();
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

abstract class ExpressionVisitor<T> {
  const ExpressionVisitor();

  T visitIdentifierExpression(IdentifierExpression node);
  T visitLiteralExpression(LiteralExpression node);
  T visitPropertyExpression(PropertyExpression node);
  T visitNavigationExpression(NavigationExpression node);
  T visitCallExpression(CallExpression node);
  T visitFunctionCallExpression(FunctionCallExpression node);
  T visitReturnExpression(ReturnExpression node);
  T visitLoopExpression(LoopExpression node);
  T visitWhileExpression(WhileExpression node);
  T visitBreakExpression(BreakExpression node);
  T visitContinueExpression(ContinueExpression node);
  T visitAssignmentExpression(AssignmentExpression node);
}

abstract class DoNothingExpressionVisitor extends ExpressionVisitor<void> {
  const DoNothingExpressionVisitor();

  @override
  void visitIdentifierExpression(IdentifierExpression node) {}
  @override
  void visitLiteralExpression(LiteralExpression node) {}
  @override
  void visitPropertyExpression(PropertyExpression node) {}
  @override
  void visitNavigationExpression(NavigationExpression node) {}
  @override
  void visitCallExpression(CallExpression node) {}
  @override
  void visitFunctionCallExpression(FunctionCallExpression node) {}
  @override
  void visitReturnExpression(ReturnExpression node) {}
  @override
  void visitLoopExpression(LoopExpression node) {}
  @override
  void visitWhileExpression(WhileExpression node) {}
  @override
  void visitBreakExpression(BreakExpression node) {}
  @override
  void visitContinueExpression(ContinueExpression node) {}
  @override
  void visitAssignmentExpression(AssignmentExpression node) {}
}
