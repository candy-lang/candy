import 'package:supernova/supernova.dart';

import '../../hir/hir/builtins.dart';
import '../../hir/hir/ids.dart';
import 'ids.dart';

part 'mono.freezed.dart';

/// The program after monomorphization: no type parameters, no trait dispatch,
/// and only the declarations `main` transitively reaches.
class Mono {
  Mono({
    required this.typeDeclarations,
    required this.assignments,
    required this.assignmentInitializationOrder,
    required this.functions,
    required this.mainFunction,
  });

  final Map<MonoType, MonoTypeDeclaration> typeDeclarations;
  final Map<DeclarationId, MonoAssignment> assignments;

  /// Every key of [assignments], with each one preceded by the assignments it
  /// reads.
  final List<DeclarationId> assignmentInitializationOrder;
  final Map<MonoFunctionId, MonoFunction> functions;

  final MonoFunctionId mainFunction;
}

// Type declarations

@freezed
sealed class MonoTypeDeclaration with _$MonoTypeDeclaration {
  const factory MonoTypeDeclaration.builtin({
    required BuiltinType type,
    @Default(<MonoType>[]) List<MonoType> typeArguments,
  }) = MonoTypeDeclaration_Builtin;
  const factory MonoTypeDeclaration.struct({
    required List<MonoStructField> fields,
  }) = MonoTypeDeclaration_Struct;
  const factory MonoTypeDeclaration.enum_({
    required List<MonoEnumVariant> variants,
  }) = MonoTypeDeclaration_Enum;

  /// A lambda type. It has no declaration in the source, but the backend needs
  /// one to emit a closure representation.
  const factory MonoTypeDeclaration.function({
    required List<MonoType> parameterTypes,
    required MonoType returnType,
  }) = MonoTypeDeclaration_Function;
}

@freezed
sealed class MonoStructField with _$MonoStructField {
  const factory MonoStructField({
    required String name,
    required MonoType type,
  }) = _MonoStructField;
}

@freezed
sealed class MonoEnumVariant with _$MonoEnumVariant {
  const factory MonoEnumVariant({
    required String name,
    required MonoType? type,
  }) = _MonoEnumVariant;
}

// Functions and assignments

@freezed
sealed class MonoAssignment with _$MonoAssignment {
  const factory MonoAssignment({
    required DeclarationId id,
    required String name,
    required MonoType type,
    required MonoBody body,
  }) = _MonoAssignment;
}

@freezed
sealed class MonoFunction with _$MonoFunction {
  const factory MonoFunction({
    required MonoFunctionId id,
    required List<MonoParameter> parameters,
    required MonoType returnType,
    required MonoBodyOrBuiltin body,
  }) = _MonoFunction;
}

@freezed
sealed class MonoParameter with _$MonoParameter {
  const factory MonoParameter({
    required HirId id,
    required String name,
    required MonoType type,
  }) = _MonoParameter;
}

@freezed
sealed class MonoBodyOrBuiltin with _$MonoBodyOrBuiltin {
  const factory MonoBodyOrBuiltin.body(MonoBody body) = MonoBodyOrBuiltin_Body;

  /// [substitutions] cover the builtin's own type parameters, e.g. the item
  /// type of `builtinListOf`.
  const factory MonoBodyOrBuiltin.builtin({
    required BuiltinFunction function,
    required Map<TypeParameterId, MonoType> substitutions,
  }) = MonoBodyOrBuiltin_Builtin;
}

// Bodies and expressions

/// The last expression is the body's result.
///
/// IDs are those of the HIR expressions this was lowered from. They stay
/// unique within one [MonoFunction] because HIR lowering draws them from a
/// single generator, and monomorphization emits each HIR expression at most
/// once per instantiation.
@freezed
sealed class MonoBody with _$MonoBody {
  const factory MonoBody(List<MonoBodyEntry> expressions) = _MonoBody;
  const MonoBody._();

  static const empty = MonoBody([]);

  HirId get returnValueId => expressions.last.id;
  MonoType get returnType => expressions.last.expression.type;
}

@freezed
sealed class MonoBodyEntry with _$MonoBodyEntry {
  const factory MonoBodyEntry({
    required HirId id,

    /// Only used for diagnostics and debug output.
    required String? name,
    required MonoExpression expression,
  }) = _MonoBodyEntry;
}

@freezed
sealed class MonoExpression with _$MonoExpression {
  const factory MonoExpression({
    required MonoExpressionKind kind,
    required MonoType type,
  }) = _MonoExpression;
}

@freezed
sealed class MonoExpressionKind with _$MonoExpressionKind {
  const factory MonoExpressionKind.int(int value) = MonoExpressionKind_Int;
  const factory MonoExpressionKind.text(String value) = MonoExpressionKind_Text;

  /// The struct's type is the enclosing expression's type.
  const factory MonoExpressionKind.createStruct({required List<HirId> fields}) =
      MonoExpressionKind_CreateStruct;
  const factory MonoExpressionKind.structAccess({
    required HirId struct,
    required String field,
  }) = MonoExpressionKind_StructAccess;

  /// The enum's type is the enclosing expression's type.
  const factory MonoExpressionKind.createEnum({
    required String variant,
    required HirId? value,
  }) = MonoExpressionKind_CreateEnum;

  /// A parameter or earlier expression in the same body.
  const factory MonoExpressionKind.localReference(HirId id) =
      MonoExpressionKind_LocalReference;
  const factory MonoExpressionKind.globalAssignmentReference(DeclarationId id) =
      MonoExpressionKind_GlobalAssignmentReference;

  const factory MonoExpressionKind.callFunction({
    required MonoFunctionId function,
    required List<HirId> arguments,
  }) = MonoExpressionKind_CallFunction;
  const factory MonoExpressionKind.callLambda({
    required HirId lambda,
    required List<HirId> arguments,
  }) = MonoExpressionKind_CallLambda;

  const factory MonoExpressionKind.switch_({
    required HirId value,
    required MonoType enumType,
    required List<MonoSwitchCase> cases,
  }) = MonoExpressionKind_Switch;

  const factory MonoExpressionKind.lambda({
    required List<MonoParameter> parameters,
    required MonoBody body,
  }) = MonoExpressionKind_Lambda;
}

@freezed
sealed class MonoSwitchCase with _$MonoSwitchCase {
  const factory MonoSwitchCase({
    required String variant,

    /// The binding for the variant's payload, with its type — the backend
    /// needs the latter without looking the enum up again.
    required (HirId, MonoType)? value,
    required MonoBody body,
  }) = _MonoSwitchCase;
}
