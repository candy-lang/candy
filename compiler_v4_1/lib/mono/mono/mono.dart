import 'package:supernova/supernova.dart';

import '../../hir/hir/builtins.dart';
import '../../hir/hir/ids.dart';
import '../../position.dart';
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
    required this.vtables,
    required this.typeIds,
    required this.mainFunction,
  });

  final Map<MonoType, MonoTypeDeclaration> typeDeclarations;
  final Map<DeclarationId, MonoAssignment> assignments;

  /// Every key of [assignments], with each one preceded by the assignments it
  /// reads.
  final List<DeclarationId> assignmentInitializationOrder;
  final Map<MonoFunctionId, MonoFunction> functions;
  final Map<MonoVtableId, MonoVtable> vtables;

  /// The runtime ID of every type that a vtable or a `TypeId` mentions.
  final Map<MonoType, int> typeIds;

  final MonoFunctionId mainFunction;
}

// Type declarations

@freezed
sealed class MonoTypeDeclaration with _$MonoTypeDeclaration {
  const factory MonoTypeDeclaration.builtin({
    required BuiltinType type,
    @Default(<MonoType>[]) List<MonoType> typeArguments,
    SourceSpan? span,
  }) = MonoTypeDeclaration_Builtin;
  const factory MonoTypeDeclaration.struct({
    required List<MonoStructField> fields,
    SourceSpan? span,
  }) = MonoTypeDeclaration_Struct;
  const factory MonoTypeDeclaration.enum_({
    required List<MonoEnumVariant> variants,
    SourceSpan? span,
  }) = MonoTypeDeclaration_Enum;

  /// A lambda type. It has no declaration in the source, but the backend needs
  /// one to emit a closure representation.
  const factory MonoTypeDeclaration.function({
    required List<MonoType> parameterTypes,
    required MonoType returnType,
  }) = MonoTypeDeclaration_Function;

  /// A trait object, which holds the value behind a pointer next to its
  /// vtable. [slots] are the trait's dispatchable functions in declaration
  /// order.
  const factory MonoTypeDeclaration.dyn({
    required List<MonoDynSlot> slots,
    SourceSpan? span,
  }) = MonoTypeDeclaration_Dyn;
  const MonoTypeDeclaration._();

  SourceSpan? get span => switch (this) {
    MonoTypeDeclaration_Builtin(:final span) ||
    MonoTypeDeclaration_Struct(:final span) ||
    MonoTypeDeclaration_Enum(:final span) ||
    MonoTypeDeclaration_Dyn(:final span) => span,
    MonoTypeDeclaration_Function() => null,
  };
}

/// One function of a trait object's vtable. The receiver isn't part of
/// [parameterTypes], since every slot takes it as an untyped pointer.
@freezed
sealed class MonoDynSlot with _$MonoDynSlot {
  const factory MonoDynSlot({
    required String name,
    required List<MonoType> parameterTypes,
    required MonoType returnType,
  }) = _MonoDynSlot;
}

/// [entries] implement the trait object's slots, in order, for one concrete
/// type.
@freezed
sealed class MonoVtable with _$MonoVtable {
  const factory MonoVtable({
    required int typeId,
    required List<MonoFunctionId> entries,
  }) = _MonoVtable;
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
    required SourceSpan span,
  }) = _MonoAssignment;
}

@freezed
sealed class MonoFunction with _$MonoFunction {
  const factory MonoFunction({
    required MonoFunctionId id,
    required List<MonoParameter> parameters,
    required MonoType returnType,
    required MonoBodyOrBuiltin body,

    /// The declaration's span in the Candy source, or `null` for builtins.
    SourceSpan? span,
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

  /// Calls the function in vtable slot [slot] of the trait object that is the
  /// first parameter.
  const factory MonoBodyOrBuiltin.dynDispatch({required int slot}) =
      MonoBodyOrBuiltin_DynDispatch;
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

  /// Boxes [value] into the trait object that is the enclosing expression's
  /// type, using [vtable]. Without one, [value] is already a trait object,
  /// which is reread as an `Any`.
  const factory MonoExpressionKind.toDyn({
    required HirId value,
    required MonoVtableId? vtable,
  }) = MonoExpressionKind_ToDyn;
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
