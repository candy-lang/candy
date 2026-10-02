import 'package:supernova/supernova.dart';

import '../../ast.dart';
import '../../position.dart';
import '../traits/solver.dart';
import 'builtins.dart';
import 'ids.dart';

part 'hir.freezed.dart';

/// [None] means the element was required but missing or malformed, and an
/// error was already reported.
typedef HirErrorOr<T extends Object> = Option<T>;

class Hir({
  required final SolverEnvironment solverEnvironment,
  required final Map<DeclarationId, HirTypeDeclaration> typeDeclarations,
  required final Map<DeclarationId, HirTraitDefinition> traits,
  required final List<HirImpl> impls,
  required final Map<DeclarationId, HirAssignment> assignments,
  required final Map<DeclarationId, HirFunction> functions,

  required final DeclarationId? mainFunction,

  /// The `Any` trait, which every type implements without an impl.
  final DeclarationId? anyTrait,
});

// Types

@freezed
sealed class const HirType._() with _$HirType {
  const factory named({
    required DeclarationId declaration,
    @Default([]) List<HirType> typeArguments,
  }) = HirType_Named;
  const factory parameter(TypeParameterId id) = HirType_Parameter;

  /// [baseType] is what `Self` stands for at the declaration site, used when
  /// no substitution applies.
  const factory self({required HirType baseType}) = HirType_Self;
  const factory function({
    required List<HirType> parameterTypes,
    required HirType returnType,
  }) = HirType_Function;

  /// A projection like `T.Item`, before an impl is known to resolve it.
  ///
  /// [trait] pins down the trait's own type arguments, so `Self.Output` is
  /// unambiguous even for a generic trait. [declaration] is the associated
  /// type; its parent is the trait. Resolving one needs the impls in scope, so
  /// it happens in `TypeNormalizer` rather than in [substitute].
  const factory associated({
    required HirTrait trait,
    required HirType baseType,
    required DeclarationId declaration,
  }) = HirType_Associated;

  /// A trait object: some value whose type implements [trait], with the
  /// concrete type only known at runtime. Written as the bare trait name.
  const factory dyn(HirTrait trait) = HirType_Dyn;

  /// An error was already reported, so anything involving this stays silent.
  const factory error() = HirType_Error;

  /// Only exists while a body is being inferred. The substitution pass
  /// replaces any that are left with [HirType.error].
  const factory variable(TypeVariableId id) = HirType_Variable;

  bool get containsError => switch (this) {
    HirType_Named(:final typeArguments) => typeArguments.any(
      (it) => it.containsError,
    ),
    HirType_Parameter() => false,
    HirType_Self(:final baseType) => baseType.containsError,
    HirType_Function(:final parameterTypes, :final returnType) =>
      parameterTypes.any((it) => it.containsError) || returnType.containsError,
    HirType_Associated(:final trait, :final baseType) =>
      trait.typeArguments.any((it) => it.containsError) ||
          baseType.containsError,
    HirType_Dyn(:final trait) => trait.typeArguments.any(
      (it) => it.containsError,
    ),
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
        HirType_Associated(:final trait, :final baseType, :final declaration) =>
          .associated(
            trait: trait.substitute(environment),
            baseType: baseType.substitute(environment),
            declaration: declaration,
          ),
        HirType_Dyn(:final trait) => .dyn(trait.substitute(environment)),
        HirType_Error() || HirType_Variable() => this,
      };

  DeclarationId get selfOwner => switch (this) {
    HirType_Named(:final declaration) => declaration,
    HirType_Parameter(:final id) => id.owner,
    HirType_Self(:final baseType) => baseType.selfOwner,
    HirType_Function() ||
    HirType_Associated() ||
    HirType_Dyn() ||
    HirType_Error() ||
    HirType_Variable() => throw StateError(
      'This type can never be the base type of `Self`.',
    ),
  };

  /// A trait's own `Self`, which stands for an unknown type. An impl's `Self`
  /// is only another spelling of the type the impl is for.
  bool get isTraitSelf => switch (this) {
    HirType_Self(baseType: HirType_Parameter(id: TypeParameterId_Self())) =>
      true,
    _ => false,
  };

  /// `Self` stands for its base type wherever the concrete type matters.
  HirType get unwrapSelf => switch (this) {
    HirType_Self(:final baseType) => baseType.unwrapSelf,
    _ => this,
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
    HirType_Self(:final baseType) => isTraitSelf ? 'Self' : 'Self<$baseType>',
    HirType_Function(:final parameterTypes, :final returnType) =>
      '(${parameterTypes.join(', ')}) $returnType',
    HirType_Associated(:final baseType, :final declaration) =>
      '$baseType.${declaration.name}',
    HirType_Dyn(:final trait) => '$trait',
    HirType_Error() => '<error>',
    HirType_Variable(:final id) => '$id',
  };
}

@freezed
sealed class const HirTrait._() with _$HirTrait {
  const factory({
    required DeclarationId declaration,
    @Default(<HirType>[]) List<HirType> typeArguments,
  }) = _HirTrait;

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
sealed class const HirTypeParameter._() with _$HirTypeParameter {
  const factory({
    required TypeParameterId id,

    /// Bounds that couldn't be lowered are dropped, so this is empty for both
    /// `[T]` and `[T: Unknown]`.
    @Default(<HirTrait>[]) List<HirTrait> upperBounds,
  }) = _HirTypeParameter;

  String get name => id.name;
  HirType get type => .parameter(id);

  @override
  String toString() =>
      upperBounds.isEmpty ? name : '$name: ${upperBounds.join(' + ')}';
}

// Type declarations

@freezed
sealed class const HirTypeDeclaration._() with _$HirTypeDeclaration {
  const factory({
    required DeclarationId id,
    required List<HirTypeParameter> typeParameters,
    required HirTypeDeclarationKind kind,

    /// The declaration's name in the Candy source, or `null` for
    /// declarations without one (e.g. malformed code).
    SourceSpan? span,
  }) = _HirTypeDeclaration;

  /// `List[T]` for `struct List[T]`.
  HirType get selfType => .named(
    declaration: id,
    typeArguments: typeParameters.map((it) => it.type).toList(),
  );
}

@freezed
sealed class const HirTypeDeclarationKind._() with _$HirTypeDeclarationKind {
  const factory builtin(BuiltinType type) = HirTypeDeclarationKind_Builtin;
  const factory struct({required List<HirStructField> fields}) =
      HirTypeDeclarationKind_Struct;
  const factory enum_({required List<HirEnumVariant> variants}) =
      HirTypeDeclarationKind_Enum;
}

@freezed
sealed class const HirStructField._() with _$HirStructField {
  const factory({
    required String name,
    required HirType type,
    @Default(AstVisibility.private) AstVisibility visibility,
  }) = _HirStructField;
}

@freezed
sealed class const HirEnumVariant._() with _$HirEnumVariant {
  const factory({required String name, required HirType? type}) =
      _HirEnumVariant;
}

// Traits and impls

@freezed
sealed class const HirTraitDefinition._() with _$HirTraitDefinition {
  const factory({
    required DeclarationId id,
    required List<HirTypeParameter> typeParameters,
    required SolverGoal solverGoal,
    required List<SolverGoal> solverSubgoals,
    required List<HirAssociatedType> associatedTypes,
    required Map<DeclarationId, HirTraitFunction> functions,
  }) = _HirTraitDefinition;
}

@freezed
sealed class const HirAssociatedType._() with _$HirAssociatedType {
  const factory({
    required DeclarationId id,

    /// Bounds that couldn't be lowered are dropped, so this is empty for both
    /// `type Item` and `type Item: Unknown`.
    @Default(<HirTrait>[]) List<HirTrait> upperBounds,
  }) = _HirAssociatedType;

  String get name => id.name;

  @override
  String toString() =>
      upperBounds.isEmpty ? name : '$name: ${upperBounds.join(' + ')}';
}

@freezed
sealed class const HirTraitFunction._() with _$HirTraitFunction {
  const factory({
    required HirFunctionSignature signature,

    /// `null` without a default implementation.
    required HirBody? body,
  }) = _HirTraitFunction;
}

@freezed
sealed class const HirImpl._() with _$HirImpl {
  const factory({
    required DeclarationId id,
    required List<HirTypeParameter> typeParameters,
    required HirType type,

    /// `null` for an inherent impl, which names no trait.
    required HirErrorOr<HirTrait>? trait,

    /// Keyed by the trait's associated type each one binds.
    required Map<DeclarationId, HirType> associatedTypes,

    /// Keyed by the trait function each one implements.
    required Map<DeclarationId, HirFunction> functions,

    /// The impl's own functions, empty unless this is an inherent impl.
    required Map<DeclarationId, HirFunction> inherentFunctions,
  }) = _HirImpl;
}

// Functions and assignments

@freezed
sealed class const HirFunctionSignature._() with _$HirFunctionSignature {
  const factory({
    required DeclarationId id,
    required String name,
    required List<HirTypeParameter> typeParameters,
    required List<HirParameter> parameters,
    required HirType returnType,

    /// The declaration's span in the Candy source, or `null` for builtins.
    SourceSpan? span,
  }) = _HirFunctionSignature;

  List<HirType> get parameterTypes => parameters.map((it) => it.type).toList();

  HirType get functionType =>
      .function(parameterTypes: parameterTypes, returnType: returnType);

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
sealed class const HirParameter._() with _$HirParameter {
  const factory({
    required HirId id,
    required String name,
    required HirType type,

    /// Where the parameter is declared, or `null` for builtins.
    SourceSpan? span,
  }) = _HirParameter;
}

@freezed
sealed class const HirFunction._() with _$HirFunction {
  const factory({
    required HirFunctionSignature signature,
    required HirBodyOrBuiltin body,
  }) = _HirFunction;
}

@freezed
sealed class const HirBodyOrBuiltin._() with _$HirBodyOrBuiltin {
  const factory body(HirBody body) = HirBodyOrBuiltin_Body;
  const factory builtin(BuiltinFunction function) = HirBodyOrBuiltin_Builtin;
}

@freezed
sealed class const HirAssignment._() with _$HirAssignment {
  const factory({
    required DeclarationId id,
    required String name,
    required HirType type,
    required HirBody body,
    required SourceSpan span,
  }) = _HirAssignment;
}

// Bodies and expressions

/// The last expression is the body's result.
@freezed
sealed class const HirBody._() with _$HirBody {
  const factory(List<HirBodyEntry> expressions) = _HirBody;

  static const empty = HirBody([]);

  HirId get returnValueId => expressions.last.id;
  HirType get returnType => expressions.last.expression.type;
}

@freezed
sealed class const HirBodyEntry._() with _$HirBodyEntry {
  const factory({
    required HirId id,

    /// Only used for diagnostics and debug output.
    required String? name,
    required HirExpression expression,
  }) = _HirBodyEntry;
}

@freezed
sealed class const HirExpression._() with _$HirExpression {
  const factory({required HirExpressionKind kind, required HirType type}) =
      _HirExpression;
}

@freezed
sealed class const HirExpressionKind._() with _$HirExpressionKind {
  const factory int(int value) = HirExpressionKind_Int;
  const factory text(String value) = HirExpressionKind_Text;

  /// The struct's type is the enclosing expression's type.
  const factory createStruct({required List<HirId> fields}) =
      HirExpressionKind_CreateStruct;
  const factory structAccess({required HirId struct, required String field}) =
      HirExpressionKind_StructAccess;

  /// The enum's type is the enclosing expression's type.
  const factory createEnum({required String variant, required HirId? value}) =
      HirExpressionKind_CreateEnum;

  /// A parameter or earlier expression in the same body.
  const factory reference(HirId id) = HirExpressionKind_Reference;

  /// A top-level assignment.
  const factory globalReference(DeclarationId id) =
      HirExpressionKind_GlobalReference;

  const factory call({
    required DeclarationId function,
    required Map<TypeParameterId, HirType> substitutions,
    required List<HirId> arguments,
  }) = HirExpressionKind_Call;

  /// The Rust compiler reuses [HirExpressionKind.call] for this by putting the
  /// local's ID in the function slot, leaving consumers to guess.
  const factory callLambda({
    required HirId lambda,
    required List<HirId> arguments,
  }) = HirExpressionKind_CallLambda;

  const factory switch_({
    required HirId value,
    required HirType enumType,
    required List<HirSwitchCase> cases,
  }) = HirExpressionKind_Switch;

  const factory lambda({
    required List<HirParameter> parameters,
    required HirBody body,
  }) = HirExpressionKind_Lambda;

  /// Boxes [value] into the trait object that is the enclosing expression's
  /// type. [sourceType] is [value]'s type, which monomorphization needs to
  /// pick the vtable.
  const factory toDyn({required HirId value, required HirType sourceType}) =
      HirExpressionKind_ToDyn;

  const factory error() = HirExpressionKind_Error;
}

@freezed
sealed class const HirSwitchCase._() with _$HirSwitchCase {
  const factory({
    required String variant,
    required HirId? valueId,
    required HirBody body,
  }) = _HirSwitchCase;
}
