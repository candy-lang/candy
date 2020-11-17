import 'package:freezed_annotation/freezed_annotation.dart';

import '../../../compiler.dart';
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

  const factory Expression.functionCall(
    DeclarationLocalId id,
    Expression target,
    List<CandyType> typeArguments,
    Map<String, Expression> valueArguments,
    CandyType returnType,
  ) = FunctionCallExpression;
  const factory Expression.constructorCall(
    DeclarationLocalId id,
    ClassDeclaration class_,
    List<CandyType> typeArguments,
    Map<String, Expression> valueArguments,
    CandyType returnType,
  ) = ConstructorCallExpression;
  const factory Expression.expressionCall(
    DeclarationLocalId id,
    Expression target,
    List<Expression> valueArguments,
    CandyType returnType,
  ) = ExpressionCallExpression;
  // ignore: non_constant_identifier_names
  const factory Expression.if_(
    DeclarationLocalId id,
    Expression condition,
    List<Expression> thenBody,
    List<Expression> elseBody,
    CandyType type,
  ) = IfExpression;
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
  // ignore: non_constant_identifier_names
  const factory Expression.throw_(
    DeclarationLocalId id,
    Expression error,
  ) = ThrowExpression;
  const factory Expression.assignment(
    DeclarationLocalId id,
    IdentifierExpression left,
    Expression right,
  ) = AssignmentExpression;
  // ignore: non_constant_identifier_names
  const factory Expression.as_(
    DeclarationLocalId id,
    Expression instance,
    CandyType typeToCheck,
  ) = AsExpression;
  // ignore: non_constant_identifier_names
  const factory Expression.is_(
    DeclarationLocalId id,
    Expression instance,
    CandyType typeToCheck, {
    bool isNegated,
  }) = IsExpression;

  const Expression._();

  CandyType get type => map(
        identifier: (type) => type.identifier.type,
        literal: (lit) => lit.literal.type,
        property: (prop) => prop.type,
        navigation: (nav) => nav.type,
        functionCall: (fn) => fn.returnType,
        constructorCall: (constructor) => constructor.returnType,
        expressionCall: (call) => call.returnType,
        return_: (_) => CandyType.never,
        if_: (theIf) => theIf.type,
        loop: (loop) => loop.type,
        while_: (theWhile) => theWhile.type,
        break_: (_) => CandyType.never,
        continue_: (_) => CandyType.never,
        throw_: (_) => CandyType.never,
        assignment: (assignment) => assignment.right.type,
        as_: (theAs) => theAs.typeToCheck,
        is_: (_) => CandyType.bool,
      );

  T accept<T>(ExpressionVisitor<T> visitor) => map(
        identifier: (e) => visitor.visitIdentifierExpression(e),
        literal: (e) => visitor.visitLiteralExpression(e),
        property: (e) => visitor.visitPropertyExpression(e),
        navigation: (e) => visitor.visitNavigationExpression(e),
        functionCall: (e) => visitor.visitFunctionCallExpression(e),
        constructorCall: (e) => visitor.visitConstructorCallExpression(e),
        expressionCall: (e) => visitor.visitExpressionCallExpression(e),
        return_: (e) => visitor.visitReturnExpression(e),
        if_: (e) => visitor.visitIfExpression(e),
        loop: (e) => visitor.visitLoopExpression(e),
        while_: (e) => visitor.visitWhileExpression(e),
        break_: (e) => visitor.visitBreakExpression(e),
        continue_: (e) => visitor.visitContinueExpression(e),
        throw_: (e) => visitor.visitThrowExpression(e),
        assignment: (e) => visitor.visitAssignmentExpression(e),
        as_: (e) => visitor.visitAsExpression(e),
        is_: (e) => visitor.visitIsExpression(e),
      );
}

@freezed
abstract class Identifier implements _$Identifier {
  // ignore: non_constant_identifier_names
  const factory Identifier.this_(CandyType type) = ThisIdentifier;
  // ignore: non_constant_identifier_names
  const factory Identifier.super_(UserCandyType type) = SuperIdentifier;
  const factory Identifier.meta(
    CandyType referencedType, [
    IdentifierExpression base,
  ]) = MetaIdentifier;
  const factory Identifier.reflection(
    DeclarationId id, [
    IdentifierExpression base,
  ]) = ReflectionIdentifier;

  const factory Identifier.parameter(
    DeclarationLocalId id,
    String name,
    CandyType type,
  ) = ParameterIdentifier;

  /// A property or function.
  const factory Identifier.property(
    DeclarationId id,
    CandyType type, {
    bool isMutable,
    Expression base,
    Expression receiver,
  }) = PropertyIdentifier;
  const factory Identifier.localProperty(
    DeclarationLocalId id,
    String name,
    CandyType type,
    // ignore: avoid_positional_boolean_parameters
    bool isMutable,
  ) = LocalPropertyIdentifier;

  const Identifier._();

  bool get isMutableOrNull => maybeMap(
        property: (prop) => prop.isMutable,
        localProperty: (prop) => prop.isMutable,
        orElse: () => null,
      );

  CandyType get type => when(
        this_: (type) => type,
        super_: (type) => type,
        meta: (type, _) => CandyType.meta(type),
        reflection: (declarationId, _) => CandyType.reflection(declarationId),
        parameter: (_, __, type) => type,
        property: (_, type, __, ___, ____) => type,
        localProperty: (_, __, type, ___) => type,
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

  const StringLiteralPart._();
}

abstract class ExpressionVisitor<T> {
  const ExpressionVisitor();

  T visitIdentifierExpression(IdentifierExpression node);
  T visitLiteralExpression(LiteralExpression node);
  T visitPropertyExpression(PropertyExpression node);
  T visitNavigationExpression(NavigationExpression node);
  T visitFunctionCallExpression(FunctionCallExpression node);
  T visitConstructorCallExpression(ConstructorCallExpression node);
  T visitExpressionCallExpression(ExpressionCallExpression node);
  T visitReturnExpression(ReturnExpression node);
  T visitIfExpression(IfExpression node);
  T visitLoopExpression(LoopExpression node);
  T visitWhileExpression(WhileExpression node);
  T visitBreakExpression(BreakExpression node);
  T visitContinueExpression(ContinueExpression node);
  T visitThrowExpression(ThrowExpression node);
  T visitAssignmentExpression(AssignmentExpression node);
  T visitAsExpression(AsExpression node);
  T visitIsExpression(IsExpression node);
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
  void visitFunctionCallExpression(FunctionCallExpression node) {}
  @override
  void visitConstructorCallExpression(ConstructorCallExpression node) {}
  @override
  void visitExpressionCallExpression(ExpressionCallExpression node) {}
  @override
  void visitReturnExpression(ReturnExpression node) {}
  @override
  void visitIfExpression(IfExpression node) {}
  @override
  void visitLoopExpression(LoopExpression node) {}
  @override
  void visitWhileExpression(WhileExpression node) {}
  @override
  void visitBreakExpression(BreakExpression node) {}
  @override
  void visitContinueExpression(ContinueExpression node) {}
  @override
  void visitThrowExpression(ThrowExpression node) {}
  @override
  void visitAssignmentExpression(AssignmentExpression node) {}
  @override
  void visitAsExpression(AsExpression node) {}
  @override
  void visitIsExpression(IsExpression node) {}
}
