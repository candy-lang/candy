import 'package:supernova/supernova.dart';

import '../../hir/hir/builtins.dart';
import '../../hir/hir/hir.dart';
import '../../hir/hir/ids.dart';
import '../../hir/inference/inference.dart';
import '../../hir/traits/hir_bridge.dart';

/// The function a call actually runs, and the substitutions to lower it with.
class ResolvedCall {
  const ResolvedCall({
    required this.signature,
    required this.body,
    required this.substitutions,
  });

  final HirFunctionSignature signature;
  final HirBodyOrBuiltin body;
  final Map<TypeParameterId, HirType> substitutions;
}

/// Answers what a call to a trait function dispatches to once `Self` is known.
class ImplResolver {
  ImplResolver(this.hir)
    : _implsById = {for (final impl in hir.impls) impl.id: impl};

  final Hir hir;
  final Map<DeclarationId, HirImpl> _implsById;

  /// The type parameters the function's signature and body can mention, in the
  /// order that identifies one of its instantiations.
  List<TypeParameterId> relevantTypeParameters(HirFunctionSignature signature) {
    final owner = signature.id.parent;
    final trait = owner == null ? null : hir.traits[owner];
    final impl = owner == null ? null : _implsById[owner];
    return [
      if (trait != null) ...[
        ...trait.typeParameters.map((it) => it.id),
        TypeParameterId.self(trait.id),
      ],
      if (impl != null) ...impl.typeParameters.map((it) => it.id),
      ...signature.typeParameters.map((it) => it.id),
    ];
  }

  /// [substitutions] must already be concrete, so `Self` pins down exactly one
  /// impl.
  ///
  /// A call only reaches here when its callee is a trait function, which body
  /// lowering leaves in place for a receiver whose type wasn't known yet, and
  /// for an impl that inherits a default implementation.
  ResolvedCall resolveTraitFunction(
    DeclarationId id,
    Map<TypeParameterId, HirType> substitutions,
  ) {
    final owner = id.parent;
    final trait = owner == null ? null : hir.traits[owner];
    final traitFunction = trait?.functions[id];
    if (trait == null || traitFunction == null) {
      throw StateError('Unknown function: `$id`.');
    }

    final selfType = substitutions[TypeParameterId.self(trait.id)];
    if (selfType == null) {
      throw StateError('`Self` is unknown when calling `$id`.');
    }
    final traitArguments = [
      for (final parameter in trait.typeParameters)
        substitutions[parameter.id] ??
            (throw StateError(
              '`${parameter.name}` is unknown when calling `$id`.',
            )),
    ];

    final (impl, implSubstitutions) = _findImpl(
      trait,
      selfType,
      traitArguments,
      id,
    );

    final implFunction = impl.functions[id];
    if (implFunction == null) {
      final defaultBody = traitFunction.body;
      if (defaultBody == null) {
        throw StateError("`${impl.id}` doesn't implement `$id`.");
      }
      return ResolvedCall(
        signature: traitFunction.signature,
        body: .body(defaultBody),
        substitutions: substitutions,
      );
    }

    return ResolvedCall(
      signature: implFunction.signature,
      body: implFunction.body,
      // The impl declares its own copies of the function's type parameters, so
      // the call's type arguments move over by position.
      substitutions: {
        ...implSubstitutions,
        for (final (index, parameter)
            in implFunction.signature.typeParameters.indexed)
          parameter.id:
              substitutions[traitFunction.signature.typeParameters[index].id]!,
      },
    );
  }

  (HirImpl, Map<TypeParameterId, HirType>) _findImpl(
    HirTraitDefinition trait,
    HirType selfType,
    List<HirType> traitArguments,
    DeclarationId functionId,
  ) {
    final matches = <(HirImpl, Map<TypeParameterId, HirType>)>[];
    final blanketMatches = <(HirImpl, Map<TypeParameterId, HirType>)>[];
    for (final impl in hir.impls) {
      if (impl.trait case Some(
        some: final traitRef,
      ) when traitRef.declaration == trait.id) {
        final match = _match(impl, traitRef, selfType, traitArguments);
        if (match == null) continue;
        // A blanket impl only applies where no impl for the concrete type
        // does, mirroring how overload resolution ranks the two.
        (impl.type is HirType_Parameter ? blanketMatches : matches).add((
          impl,
          match,
        ));
      }
    }

    final candidates = matches.isEmpty ? blanketMatches : matches;
    if (candidates.length == 1) return candidates.single;
    throw StateError(
      candidates.isEmpty
          ? 'No impl of `${trait.id.name}` for `$selfType` when calling '
                '`$functionId`.'
          : 'Multiple impls of `${trait.id.name}` for `$selfType` when calling '
                '`$functionId`: '
                '${candidates.map((it) => it.$1.id).join(', ')}.',
    );
  }

  /// The impl's type parameters as unification pins them down, or `null` if it
  /// doesn't apply.
  Map<TypeParameterId, HirType>? _match(
    HirImpl impl,
    HirTrait traitRef,
    HirType selfType,
    List<HirType> traitArguments,
  ) {
    final scratch = TypeVariables();
    final substitutions = {
      for (final parameter in impl.typeParameters)
        parameter.id: scratch.fresh(),
    };
    // Only `unify` is used, which never consults the well-known types.
    final unifier = Unifier(
      variables: scratch,
      wellKnownTypes: const WellKnownTypes(),
    );

    if (!unifier.unify(impl.type.substitute(substitutions), selfType)) {
      return null;
    }

    final arguments = traitRef.substitute(substitutions).typeArguments;
    if (arguments.length != traitArguments.length) return null;
    for (final (index, argument) in arguments.indexed) {
      if (!unifier.unify(argument, traitArguments[index])) return null;
    }

    final resolved = {
      for (final MapEntry(:key, :value) in substitutions.entries)
        key: scratch.finalize(value),
    };
    // Everything is concrete by now, so the bounds hold or they don't; there
    // are no conditions to assume.
    for (final goal in solverGoalsOfBounds(impl.typeParameters)) {
      if (!hir.solverEnvironment
          .solve(substituteGoal(goal, resolved), const [])
          .isUnique) {
        return null;
      }
    }
    return resolved;
  }
}
