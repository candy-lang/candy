import 'package:supernova/supernova.dart';

import '../../ast.dart';
import '../traits/solver.dart';
import 'builtins.dart';
import 'ids.dart';

part 'hir.freezed.dart';

/// [None] means the element was required but missing or malformed, and an
/// error was already reported.
typedef HirErrorOr<T extends Object> = Option<T>;

class Hir {
  Hir({
    required this.solverEnvironment,
    required this.typeDeclarations,
    required this.traits,
    required this.impls,
    required this.assignments,
    required this.functions,
    required this.mainFunction,
  });

  final SolverEnvironment solverEnvironment;
  final Map<DeclarationId, HirTypeDeclaration> typeDeclarations;
  final Map<DeclarationId, HirTraitDefinition> traits;
  final List<HirImpl> impls;
  final Map<DeclarationId, HirAssignment> assignments;
  final Map<DeclarationId, HirFunction> functions;

  final DeclarationId? mainFunction;
}

// Types

@freezed
sealed class HirType with _$HirType {
  const factory HirType.named({
    required DeclarationId declaration,
    @Default([]) List<HirType> typeArguments,
  }) = HirType_Named;
  const factory HirType.parameter(TypeParameterId id) = HirType_Parameter;

  /// [baseType] is what `Self` stands for at the declaration site, used when
  /// no substitution applies.
  const factory HirType.self({required HirType baseType}) = HirType_Self;
  const factory HirType.function({
    required List<HirType> parameterTypes,
    required HirType returnType,
  }) = HirType_Function;

  /// An error was already reported, so anything involving this stays silent.
  const factory HirType.error() = HirType_Error;

  /// Only exists while a body is being inferred. The substitution pass
  /// replaces any that are left with [HirType.error].
  const factory HirType.variable(TypeVariableId id) = HirType_Variable;

  const HirType._();

  bool get containsError => switch (this) {
    HirType_Named(:final typeArguments) => typeArguments.any(
      (it) => it.containsError,
    ),
    HirType_Parameter() => false,
    HirType_Self(:final baseType) => baseType.containsError,
    HirType_Function(:final parameterTypes, :final returnType) =>
      parameterTypes.any((it) => it.containsError) || returnType.containsError,
    HirType_Error() => true,
    HirType_Variable() => false,
  };

  /// Parameters missing from [environment] are left alone, so partial
  /// substitution is safe. The Rust compiler panics instead.
  HirType substitute(Map<TypeParameterId, HirType> environment) =>
      switch (this) {
        HirType_Named(:final declaration, :final typeArguments) => .named(
          declaration: declaration,
          typeArguments: typeArguments
              .map((it) => it.substitute(environment))
              .toList(),
        ),
        HirType_Parameter(:final id) => environment[id] ?? this,
        HirType_Self(:final baseType) =>
          environment[TypeParameterId.self(baseType.selfOwner)] ??
              .self(baseType: baseType.substitute(environment)),
        HirType_Function(:final parameterTypes, :final returnType) => .function(
          parameterTypes: parameterTypes
              .map((it) => it.substitute(environment))
              .toList(),
          returnType: returnType.substitute(environment),
        ),
        HirType_Error() || HirType_Variable() => this,
      };

  DeclarationId get selfOwner => switch (this) {
    HirType_Named(:final declaration) => declaration,
    HirType_Parameter(:final id) => id.owner,
    HirType_Self(:final baseType) => baseType.selfOwner,
    HirType_Function() || HirType_Error() || HirType_Variable() =>
      throw StateError('This type can never be the base type of `Self`.'),
  };

  /// Extra parameters are left unsubstituted rather than throwing, since
  /// lowering produces mismatched arities after reporting an error.
  static Map<TypeParameterId, HirType> buildEnvironment(
    List<HirTypeParameter> typeParameters,
    List<HirType> typeArguments,
  ) => {
    for (final (index, parameter) in typeParameters.indexed)
      if (index < typeArguments.length) parameter.id: typeArguments[index],
  };

  @override
  String toString() => switch (this) {
    HirType_Named(:final declaration, :final typeArguments) =>
      typeArguments.isEmpty
          ? declaration.name
          : '${declaration.name}[${typeArguments.join(', ')}]',
    HirType_Parameter(:final id) => id.name,
    HirType_Self(:final baseType) => 'Self<$baseType>',
    HirType_Function(:final parameterTypes, :final returnType) =>
      '(${parameterTypes.join(', ')}) $returnType',
    HirType_Error() => '<error>',
    HirType_Variable(:final id) => '$id',
  };
}

@freezed
sealed class HirTrait with _$HirTrait {
  const factory HirTrait({
    required DeclarationId declaration,
    @Default(<HirType>[]) List<HirType> typeArguments,
  }) = _HirTrait;
  const HirTrait._();

  HirTrait substitute(Map<TypeParameterId, HirType> environment) => HirTrait(
    declaration: declaration,
    typeArguments: typeArguments
        .map((it) => it.substitute(environment))
        .toList(),
  );

  @override
  String toString() => typeArguments.isEmpty
      ? declaration.name
      : '${declaration.name}[${typeArguments.join(', ')}]';
}

@freezed
sealed class HirTypeParameter with _$HirTypeParameter {
  const factory HirTypeParameter({
    required TypeParameterId id,

    /// `null` when there's no bound, [None] when it couldn't be lowered.
    required HirErrorOr<HirTrait>? upperBound,
  }) = _HirTypeParameter;
  const HirTypeParameter._();

  String get name => id.name;
  HirType get type => HirType.parameter(id);

  @override
  String toString() => switch (upperBound) {
    null => name,
    Some(:final some) => '$name: $some',
    None() => '$name: <error>',
  };
}

// Type declarations

@freezed
sealed class HirTypeDeclaration with _$HirTypeDeclaration {
  const factory HirTypeDeclaration({
    required DeclarationId id,
    required List<HirTypeParameter> typeParameters,
    required HirTypeDeclarationKind kind,
  }) = _HirTypeDeclaration;
  const HirTypeDeclaration._();

  /// `List[T]` for `struct List[T]`.
  HirType get selfType => HirType.named(
    declaration: id,
    typeArguments: typeParameters.map((it) => it.type).toList(),
  );
}

@freezed
sealed class HirTypeDeclarationKind with _$HirTypeDeclarationKind {
  const factory HirTypeDeclarationKind.builtin(BuiltinType type) =
      HirTypeDeclarationKind_Builtin;
  const factory HirTypeDeclarationKind.struct({
    required List<HirStructField> fields,
  }) = HirTypeDeclarationKind_Struct;
  const factory HirTypeDeclarationKind.enum_({
    required List<HirEnumVariant> variants,
  }) = HirTypeDeclarationKind_Enum;
}

@freezed
sealed class HirStructField with _$HirStructField {
  const factory HirStructField({
    required String name,
    required HirType type,
    @Default(AstVisibility.private) AstVisibility visibility,
  }) = _HirStructField;
}

@freezed
sealed class HirEnumVariant with _$HirEnumVariant {
  const factory HirEnumVariant({required String name, required HirType? type}) =
      _HirEnumVariant;
}

// Traits and impls

@freezed
sealed class HirTraitDefinition with _$HirTraitDefinition {
  const factory HirTraitDefinition({
    required DeclarationId id,
    required List<HirTypeParameter> typeParameters,
    required SolverGoal solverGoal,
    required List<SolverGoal> solverSubgoals,
    required Map<DeclarationId, HirTraitFunction> functions,
  }) = _HirTraitDefinition;
}

@freezed
sealed class HirTraitFunction with _$HirTraitFunction {
  const factory HirTraitFunction({
    required HirFunctionSignature signature,

    /// `null` without a default implementation.
    required HirBody? body,
  }) = _HirTraitFunction;
}

@freezed
sealed class HirImpl with _$HirImpl {
  const factory HirImpl({
    required DeclarationId id,
    required List<HirTypeParameter> typeParameters,
    required HirType type,
    required HirErrorOr<HirTrait> trait,

    /// Keyed by the trait function each one implements.
    required Map<DeclarationId, HirFunction> functions,
  }) = _HirImpl;
}

// Functions and assignments

@freezed
sealed class HirFunctionSignature with _$HirFunctionSignature {
  const factory HirFunctionSignature({
    required DeclarationId id,
    required String name,
    required List<HirTypeParameter> typeParameters,
    required List<HirParameter> parameters,
    required HirType returnType,
  }) = _HirFunctionSignature;
  const HirFunctionSignature._();

  List<HirType> get parameterTypes => parameters.map((it) => it.type).toList();

  HirType get functionType =>
      HirType.function(parameterTypes: parameterTypes, returnType: returnType);

  @override
  String toString() {
    final typeParameterText = typeParameters.isEmpty
        ? ''
        : '[${typeParameters.join(', ')}]';
    final parameterText = parameters
        .map((it) => '${it.name}: ${it.type}')
        .join(', ');
    return '$name$typeParameterText($parameterText) $returnType';
  }
}

@freezed
sealed class HirParameter with _$HirParameter {
  const factory HirParameter({
    required HirId id,
    required String name,
    required HirType type,
  }) = _HirParameter;
}

@freezed
sealed class HirFunction with _$HirFunction {
  const factory HirFunction({
    required HirFunctionSignature signature,
    required HirBodyOrBuiltin body,
  }) = _HirFunction;
}

@freezed
sealed class HirBodyOrBuiltin with _$HirBodyOrBuiltin {
  const factory HirBodyOrBuiltin.body(HirBody body) = HirBodyOrBuiltin_Body;
  const factory HirBodyOrBuiltin.builtin(BuiltinFunction function) =
      HirBodyOrBuiltin_Builtin;
}

@freezed
sealed class HirAssignment with _$HirAssignment {
  const factory HirAssignment({
    required DeclarationId id,
    required String name,
    required HirType type,
    required HirBody body,
  }) = _HirAssignment;
}

// Bodies and expressions

/// The last expression is the body's result.
@freezed
sealed class HirBody with _$HirBody {
  const factory HirBody(List<HirBodyEntry> expressions) = _HirBody;
  const HirBody._();

  static const empty = HirBody([]);

  HirId get returnValueId => expressions.last.id;
  HirType get returnType => expressions.last.expression.type;
}

@freezed
sealed class HirBodyEntry with _$HirBodyEntry {
  const factory HirBodyEntry({
    required HirId id,

    /// Only used for diagnostics and debug output.
    required String? name,
    required HirExpression expression,
  }) = _HirBodyEntry;
}

@freezed
sealed class HirExpression with _$HirExpression {
  const factory HirExpression({
    required HirExpressionKind kind,
    required HirType type,
  }) = _HirExpression;
}

@freezed
sealed class HirExpressionKind with _$HirExpressionKind {
  const factory HirExpressionKind.int(int value) = HirExpressionKind_Int;
  const factory HirExpressionKind.text(String value) = HirExpressionKind_Text;

  /// The struct's type is the enclosing expression's type.
  const factory HirExpressionKind.createStruct({required List<HirId> fields}) =
      HirExpressionKind_CreateStruct;
  const factory HirExpressionKind.structAccess({
    required HirId struct,
    required String field,
  }) = HirExpressionKind_StructAccess;

  /// The enum's type is the enclosing expression's type.
  const factory HirExpressionKind.createEnum({
    required String variant,
    required HirId? value,
  }) = HirExpressionKind_CreateEnum;

  /// A parameter or earlier expression in the same body.
  const factory HirExpressionKind.reference(HirId id) =
      HirExpressionKind_Reference;

  /// A top-level assignment.
  const factory HirExpressionKind.globalReference(DeclarationId id) =
      HirExpressionKind_GlobalReference;

  const factory HirExpressionKind.call({
    required DeclarationId function,
    required Map<TypeParameterId, HirType> substitutions,
    required List<HirId> arguments,
  }) = HirExpressionKind_Call;

  /// The Rust compiler reuses [HirExpressionKind.call] for this by putting the
  /// local's ID in the function slot, leaving consumers to guess.
  const factory HirExpressionKind.callLambda({
    required HirId lambda,
    required List<HirId> arguments,
  }) = HirExpressionKind_CallLambda;

  const factory HirExpressionKind.switch_({
    required HirId value,
    required HirType enumType,
    required List<HirSwitchCase> cases,
  }) = HirExpressionKind_Switch;

  const factory HirExpressionKind.lambda({
    required List<HirParameter> parameters,
    required HirBody body,
  }) = HirExpressionKind_Lambda;

  const factory HirExpressionKind.error() = HirExpressionKind_Error;
}

@freezed
sealed class HirSwitchCase with _$HirSwitchCase {
  const factory HirSwitchCase({
    required String variant,
    required HirId? valueId,
    required HirBody body,
  }) = _HirSwitchCase;
}
