import 'package:supernova/supernova.dart';

import '../hir/builtins.dart';
import '../hir/hir.dart';
import '../hir/ids.dart';
import '../inference/inference.dart';

TypeNormalizer normalizerOf(Hir hir) => TypeNormalizer([
  for (final impl in hir.impls)
    ProjectionImpl(
      typeParameters: impl.typeParameters,
      type: impl.type,
      trait: switch (impl.trait) {
        null || None() => null,
        Some(:final some) => some,
      },
      associatedTypes: impl.associatedTypes,
    ),
]);

/// Everything resolving a projection needs to know about one impl.
///
/// `LoweredImpl` and [HirImpl] both reduce to this, so signature lowering,
/// body lowering and monomorphization can all share the resolution logic.
class ProjectionImpl {
  const ProjectionImpl({
    required this.typeParameters,
    required this.type,
    required this.trait,
    required this.associatedTypes,
  });

  final List<HirTypeParameter> typeParameters;
  final HirType type;

  /// `null` for an inherent impl, which binds no associated types.
  final HirTrait? trait;
  final Map<DeclarationId, HirType> associatedTypes;

  bool get isBlanket => type is HirType_Parameter;
}

/// Replaces projections like `T.Item` with what the matching impl binds them
/// to.
///
/// A projection whose base type isn't known concretely stays as it is — a
/// *rigid* projection, which only unifies with itself and only offers the
/// methods of its declared bound.
class TypeNormalizer {
  TypeNormalizer(this.impls);

  final List<ProjectionImpl> impls;
  final _cache = <HirType, HirType>{};

  /// `type Item = Self.Item` is legal to write and would otherwise never stop
  /// unfolding.
  static const _maxDepth = 16;

  HirType normalize(HirType type) => _cache[type] ??= _normalize(type, 0);

  HirTrait normalizeTrait(HirTrait trait) => HirTrait(
    declaration: trait.declaration,
    typeArguments: trait.typeArguments.map(normalize).toList(),
  );

  HirType _normalize(HirType type, int depth) => switch (type) {
    HirType_Named(:final declaration, :final typeArguments) => .named(
      declaration: declaration,
      typeArguments: typeArguments.map((it) => _normalize(it, depth)).toList(),
    ),
    HirType_Function(:final parameterTypes, :final returnType) => .function(
      parameterTypes: parameterTypes
          .map((it) => _normalize(it, depth))
          .toList(),
      returnType: _normalize(returnType, depth),
    ),
    // Only a trait's `Self` stands for an unknown type; an impl's is just
    // another spelling of the type the impl is for.
    HirType_Self() when type.isTraitSelf => type,
    HirType_Self(:final baseType) => _normalize(baseType, depth),
    HirType_Associated(:final trait, :final baseType, :final declaration) =>
      _resolve(
        HirTrait(
          declaration: trait.declaration,
          typeArguments: trait.typeArguments
              .map((it) => _normalize(it, depth))
              .toList(),
        ),
        _normalize(baseType, depth),
        declaration,
        depth,
      ),
    HirType_Parameter() || HirType_Error() || HirType_Variable() => type,
  };

  HirType _resolve(
    HirTrait trait,
    HirType baseType,
    DeclarationId declaration,
    int depth,
  ) {
    final rigid = HirType.associated(
      trait: trait,
      baseType: baseType,
      declaration: declaration,
    );
    if (depth >= _maxDepth) return rigid;

    // `Self` stands for the impl's type, which is what an impl is matched
    // against — the same unwrapping `Unifier` does.
    final concrete = baseType.unwrapSelf;
    if (concrete is HirType_Parameter ||
        concrete is HirType_Associated ||
        concrete is HirType_Variable ||
        concrete is HirType_Error) {
      return rigid;
    }

    final matches = <HirType>[];
    final blanketMatches = <HirType>[];
    for (final impl in impls) {
      final binding = impl.associatedTypes[declaration];
      if (binding == null) continue;

      final traitRef = impl.trait;
      if (traitRef == null || traitRef.declaration != trait.declaration) {
        continue;
      }

      final substitutions = _match(impl, traitRef, concrete, trait);
      if (substitutions == null) continue;

      (impl.isBlanket ? blanketMatches : matches).add(
        binding.substitute(substitutions),
      );
    }

    // A blanket impl only applies where no impl for the concrete type does,
    // mirroring how overload resolution and `ImplResolver` rank the two.
    final candidates = matches.isEmpty ? blanketMatches : matches;
    if (candidates.length != 1) return rigid;

    return _normalize(candidates.single, depth + 1);
  }

  /// What unification pins the impl's type parameters down to, or `null` if
  /// the impl doesn't apply.
  ///
  /// Unlike `ImplResolver._match` this doesn't check the impl's bounds, which
  /// would need the solver — and the solver needs this. Two impls that only a
  /// bound tells apart therefore leave the projection rigid rather than
  /// resolving it.
  Map<TypeParameterId, HirType>? _match(
    ProjectionImpl impl,
    HirTrait traitRef,
    HirType concrete,
    HirTrait trait,
  ) {
    final scratch = TypeVariables();
    final substitutions = {
      for (final parameter in impl.typeParameters)
        parameter.id: scratch.fresh(),
    };
    // Normalizing here would be circular; an impl's own type is written in
    // terms of its parameters and never contains a resolvable projection.
    final unifier = Unifier(
      variables: scratch,
      wellKnownTypes: const WellKnownTypes(),
    );

    if (!unifier.unify(impl.type.substitute(substitutions), concrete)) {
      return null;
    }

    final arguments = traitRef.substitute(substitutions).typeArguments;
    if (arguments.length != trait.typeArguments.length) return null;
    for (final (index, argument) in arguments.indexed) {
      if (!unifier.unify(argument, trait.typeArguments[index])) return null;
    }

    return {
      for (final MapEntry(:key, :value) in substitutions.entries)
        key: scratch.finalize(value),
    };
  }
}
