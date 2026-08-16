import '../../hir/hir/hir.dart';
import '../../hir/hir/ids.dart';
import '../../hir/traits/projections.dart';
import '../mono/ids.dart';
import '../mono/mono.dart';

/// Turns [HirType]s into [MonoType]s, declaring each one it produces.
///
/// Every [MonoType] that appears anywhere in the result has an entry in
/// [declarations], including ones only reachable as a type argument.
class TypeMonomorphizer {
  TypeMonomorphizer(this.hir, this.normalizer);

  final Hir hir;
  final TypeNormalizer normalizer;
  final _declarations = <MonoType, MonoTypeDeclaration?>{};

  Map<MonoType, MonoTypeDeclaration> get declarations => {
    for (final MapEntry(:key, :value) in _declarations.entries) key: value!,
  };

  MonoTypeDeclaration declarationOf(MonoType type) =>
      _declarations[type] ??
      (throw StateError('`$type` was used before it was declared.'));

  MonoType monomorphize(
    HirType type,
    Map<TypeParameterId, HirType> environment,
  ) {
    // Substituting makes `Self` and the trait's type arguments concrete, which
    // is exactly what a projection needs to resolve.
    final result = _convert(
      normalizer.normalize(type.substitute(environment)),
    );
    _declare(result);
    return result;
  }

  MonoType _convert(HirType type) => switch (type) {
    HirType_Named(:final declaration, :final typeArguments) => .named(
      declaration: declaration,
      typeArguments: typeArguments.map(_convert).toList(),
    ),
    HirType_Function(:final parameterTypes, :final returnType) => .function(
      parameterTypes: parameterTypes.map(_convert).toList(),
      returnType: _convert(returnType),
    ),
    // An impl function's `Self` is owned by the implemented type rather than
    // the trait, so no substitution replaces it — but its base type is the
    // impl's type, which the environment has already made concrete.
    HirType_Self(:final baseType) => _convert(baseType),
    HirType_Parameter() ||
    HirType_Variable() ||
    HirType_Associated() => throw StateError(
      '`$type` should have been monomorphized away.',
    ),
    HirType_Error() => throw StateError('The HIR still contains errors.'),
  };

  void _declare(MonoType type) {
    if (_declarations.containsKey(type)) return;
    // Recursive types reach themselves again through their fields.
    _declarations[type] = null;
    _declarations[type] = _declarationOf(type);
  }

  MonoTypeDeclaration _declarationOf(MonoType type) {
    switch (type) {
      case MonoType_Function(:final parameterTypes, :final returnType):
        parameterTypes.forEach(_declare);
        _declare(returnType);
        return .function(
          parameterTypes: parameterTypes,
          returnType: returnType,
        );

      case MonoType_Named(:final declaration, :final typeArguments):
        typeArguments.forEach(_declare);

        final hirDeclaration = hir.typeDeclarations[declaration];
        if (hirDeclaration == null) {
          throw StateError('Unknown type declaration: `$declaration`.');
        }
        final environment = HirType.buildEnvironment(
          hirDeclaration.typeParameters,
          typeArguments.map(_hirTypeOf).toList(),
        );

        return switch (hirDeclaration.kind) {
          HirTypeDeclarationKind_Builtin(type: final builtin) => .builtin(
            type: builtin,
            typeArguments: typeArguments,
            span: hirDeclaration.span,
          ),
          HirTypeDeclarationKind_Struct(:final fields) => .struct(
            fields: [
              for (final field in fields)
                MonoStructField(
                  name: field.name,
                  type: monomorphize(field.type, environment),
                ),
            ],
            span: hirDeclaration.span,
          ),
          HirTypeDeclarationKind_Enum(:final variants) => .enum_(
            variants: [
              for (final variant in variants)
                MonoEnumVariant(
                  name: variant.name,
                  type: variant.type == null
                      ? null
                      : monomorphize(variant.type!, environment),
                ),
            ],
            span: hirDeclaration.span,
          ),
        };
    }
  }

  HirType _hirTypeOf(MonoType type) => switch (type) {
    MonoType_Named(:final declaration, :final typeArguments) => .named(
      declaration: declaration,
      typeArguments: typeArguments.map(_hirTypeOf).toList(),
    ),
    MonoType_Function(:final parameterTypes, :final returnType) => .function(
      parameterTypes: parameterTypes.map(_hirTypeOf).toList(),
      returnType: _hirTypeOf(returnType),
    ),
  };
}
