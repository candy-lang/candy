import 'package:supernova/supernova.dart' hide IterableSorted;

import '../../lowering/modules.dart';
import '../../lowering/signatures.dart';
import '../../lowering/types.dart';
import '../hir/hir.dart';
import '../hir/ids.dart';
import '../traits/dyn_compatibility.dart';
import '../traits/hir_bridge.dart';
import '../traits/solver.dart';
import '../traits/solver_values.dart';
import 'inference.dart';

/// The type parameters in scope where a call appears. Their bounds are what
/// the solver may assume; they're opaque so nothing else can be proven of them.
class SolverContext {
  const SolverContext(
    this.inScopeTypeParameters, {
    this.extraConditions,
    this.selfTrait,
    this.selfType,
  });

  final List<HirTypeParameter> inScopeTypeParameters;

  /// The bounds that in-scope associated types carry, which
  /// [OverloadResolver.solverContextFor] works out — `ToText($Assoc$Item(?T))`
  /// for `[T: Iterator]` and `trait Iterator { type Item: ToText }`.
  final List<SolverGoal>? extraConditions;

  /// Inside a trait declaration, that trait and the `Self` it bounds. Both
  /// `null` anywhere else, including in an impl.
  final HirTrait? selfTrait;
  final HirType? selfType;

  List<SolverGoal> get conditions => [
    ...solverGoalsOfBounds(inScopeTypeParameters),
    ?_selfGoal,
    ...?extraConditions,
  ];
  Set<SolverVariable> get opaqueVariables => {
    ...solverVariablesOf(inScopeTypeParameters),
    ?_selfVariable,
  };

  SolverGoal? get _selfGoal {
    if (selfTrait case final trait?) {
      if (selfType case final type?) return solverGoalOf(trait, type);
    }
    return null;
  }

  SolverVariable? get _selfVariable {
    if (selfTrait case final trait?) {
      return SolverVariable.parameter(TypeParameterId.self(trait.declaration));
    }
    return null;
  }
}

/// A function that could be called, before its type parameters are given
/// variables to stand in for them.
class CandidateSource {
  const CandidateSource({
    required this.function,
    required this.preset,
    required this.group,
    required this.goals,
  });

  final LoweredFunction function;

  /// Already-known substitutions: `Self`, the trait's type arguments, and the
  /// enclosing impl's type parameters.
  final Map<TypeParameterId, HirType> preset;

  /// Lower binds tighter. Only the lowest non-empty group is considered.
  final int group;

  /// Trait obligations this candidate brings, e.g. from a blanket impl.
  final List<SolverGoal> goals;

  HirFunctionSignature get signature => function.signature;

  OverloadCandidate instantiate(TypeVariables variables) {
    final substitutions = {
      ...preset,
      for (final parameter in signature.typeParameters)
        parameter.id: variables.fresh(),
    };
    return OverloadCandidate(
      source: this,
      signature: signature.copyWith(
        parameters: signature.parameters
            .map((it) => it.copyWith(type: it.type.substitute(substitutions)))
            .toList(),
        returnType: signature.returnType.substitute(substitutions),
      ),
      substitutions: substitutions,
    );
  }
}

class OverloadCandidate {
  const OverloadCandidate({
    required this.source,
    required this.signature,
    required this.substitutions,
  });

  final CandidateSource source;

  /// [CandidateSource.signature] with variables in place of type parameters.
  final HirFunctionSignature signature;
  final Map<TypeParameterId, HirType> substitutions;

  DeclarationId get id => source.signature.id;
  bool get isGeneric => source.signature.typeParameters.isNotEmpty;
}

sealed class OverloadResolution {
  const OverloadResolution();
}

class OverloadResolved extends OverloadResolution {
  const OverloadResolved(this.candidate);
  final OverloadCandidate candidate;
}

class OverloadNotFound extends OverloadResolution {
  const OverloadNotFound(this.rejections);

  /// Every candidate considered, with why it didn't apply.
  final List<(HirFunctionSignature, String)> rejections;
}

class OverloadAmbiguous extends OverloadResolution {
  const OverloadAmbiguous(this.candidates);
  final List<CandidateSource> candidates;
}

class OverloadResolver {
  OverloadResolver(this.signatures);

  final Signatures signatures;

  Unifier _unifier(TypeVariables variables) => Unifier(
    variables: variables,
    wellKnownTypes: signatures.wellKnownTypes,
    normalize: signatures.normalizer.normalize,
  );

  /// The solver context for a body written in [typeScope].
  ///
  /// Besides the bounds on the type parameters themselves, this assumes the
  /// bounds a trait puts on its associated types, so `T.Item` is usable as a
  /// `ToText` when `trait Iterator { type Item: ToText }`. Projections of
  /// projections are followed a few levels deep; a trait whose associated type
  /// is bounded by its own trait would otherwise never stop.
  SolverContext solverContextFor(TypeScope typeScope) {
    final selfType = typeScope.selfBaseType == null
        ? null
        : HirType.self(baseType: typeScope.selfBaseType!);
    // Only a trait's own `Self` is opaque and bounded by the trait; an impl's
    // is the concrete type it implements, which its own rule already covers.
    final isTraitSelf = selfType?.isTraitSelf ?? false;

    final conditions = <SolverGoal>[];
    var frontier = <(HirType, HirTrait)>[
      for (final parameter in typeScope.typeParameters)
        for (final bound in parameter.upperBounds) (parameter.type, bound),
      if (typeScope.selfTrait case final traitOfSelf?)
        if (selfType case final type?) (type, traitOfSelf),
    ];

    for (var depth = 0; depth < _maxProjectionDepth; depth++) {
      final next = <(HirType, HirTrait)>[];
      for (final (baseType, traitRef) in frontier) {
        final trait = signatures.traits[traitRef.declaration];
        if (trait == null) continue;

        final environment = {
          ...HirType.buildEnvironment(
            trait.typeParameters,
            traitRef.typeArguments,
          ),
          TypeParameterId.self(trait.id): baseType,
        };
        for (final associatedType in trait.associatedTypes) {
          for (final bound in associatedType.upperBounds) {
            final projection = HirType.associated(
              trait: traitRef,
              baseType: baseType,
              declaration: associatedType.id,
            );
            final substituted = bound.substitute(environment);
            final goal = solverGoalOf(
              substituted,
              projection,
              normalizer: signatures.normalizer,
            );
            if (goal != null) conditions.add(goal);
            next.add((projection, substituted));
          }
        }
      }
      if (next.isEmpty) break;
      frontier = next;
    }

    return SolverContext(
      typeScope.typeParameters,
      extraConditions: conditions,
      selfTrait: isTraitSelf ? typeScope.selfTrait : null,
      selfType: isTraitSelf ? selfType : null,
    );
  }

  static const _maxProjectionDepth = 3;

  /// The obligations an impl's own bounds impose, once its type parameters are
  /// pinned down.
  List<SolverGoal> _boundGoals(
    List<HirTypeParameter> typeParameters,
    Map<TypeParameterId, HirType> substitutions,
  ) => solverGoalsOfBounds(typeParameters, normalizer: signatures.normalizer)
      .map(
        (it) => substituteGoal(
          it,
          substitutions,
          normalizer: signatures.normalizer,
        ),
      )
      .toList();

  /// The associated type a rigid projection stands for, or `null` if it has no
  /// declared bound and so offers no methods at all.
  HirAssociatedType? _associatedTypeOf(HirType_Associated projection) =>
      signatures.traits[projection.trait.declaration]?.associatedType(
        projection.declaration,
      );

  /// Candidates for `receiver.name(…)`.
  ///
  /// UFCS is deliberately absent: top-level functions are never candidates
  /// here, so a method call only ever finds trait and impl members.
  List<CandidateSource> memberCandidates(
    HirType rawReceiver,
    String name,
    SolverContext solverContext,
  ) {
    final receiver = signatures.normalizer.normalize(rawReceiver);
    final candidates = _ownMemberCandidates(receiver, name, solverContext);

    // Every type implements `Any` without an impl. A bound on `Any`, or `Any`'s
    // own `Self`, may have added its functions already.
    final anyId = signatures.wellKnownTypes.anyId;
    if (anyId != null && !candidates.any((it) => it.function.owner == anyId)) {
      final isOpaque =
          receiver is HirType_Associated ||
          receiver.isTraitSelf ||
          receiver is HirType_Parameter ||
          receiver is HirType_Dyn;
      _addTraitFunctions(
        candidates,
        HirTrait(declaration: anyId),
        receiver,
        name,
        group: isOpaque ? 0 : _traitGroup,
        goals: const [],
      );
    }
    return candidates;
  }

  List<CandidateSource> _ownMemberCandidates(
    HirType receiver,
    String name,
    SolverContext solverContext,
  ) {
    final candidates = <CandidateSource>[];

    // A trait object only offers what its vtable holds.
    if (receiver is HirType_Dyn) {
      _addTraitFunctions(
        candidates,
        receiver.trait,
        receiver,
        name,
        group: 0,
        goals: const [],
        onlyDispatchable: !signatures.wellKnownTypes.isAny(
          receiver.trait.declaration,
        ),
      );
      return candidates;
    }

    // A projection that no impl resolves stands for one unknown type, so like
    // a type parameter it only offers what its declared bound provides.
    if (receiver is HirType_Associated) {
      final associatedType = _associatedTypeOf(receiver);
      final trait = signatures.traits[receiver.trait.declaration];
      for (final bound in associatedType?.upperBounds ?? const <HirTrait>[]) {
        _addTraitFunctions(
          candidates,
          trait == null
              ? bound
              : bound.substitute({
                  ...HirType.buildEnvironment(
                    trait.typeParameters,
                    receiver.trait.typeArguments,
                  ),
                  TypeParameterId.self(trait.id): receiver.baseType,
                }),
          receiver,
          name,
          group: 0,
          goals: const [],
        );
      }
      return candidates;
    }

    // Inside the trait itself, `Self` is as opaque as a type parameter bounded
    // by that trait, so the trait's own functions are all it offers.
    if (receiver.isTraitSelf) {
      if (solverContext.selfTrait case final trait?) {
        _addTraitFunctions(
          candidates,
          trait,
          receiver,
          name,
          group: 0,
          goals: const [],
        );
      }
      return candidates;
    }

    // A type parameter is only usable through the traits it's bounded by.
    if (receiver is HirType_Parameter) {
      final parameter = solverContext.inScopeTypeParameters
          .where((it) => it.id == receiver.id)
          .firstOrNull;
      for (final bound in parameter?.upperBounds ?? const <HirTrait>[]) {
        _addTraitFunctions(
          candidates,
          bound,
          receiver,
          name,
          group: 0,
          goals: const [],
        );
      }
      return candidates;
    }

    for (final impl in signatures.impls) {
      if (impl.isInherent) {
        _addInherentFunctions(candidates, impl, receiver, name);
        continue;
      }

      if (impl.trait case Some(some: final traitRef)) {
        final match = _matchImpl(impl, receiver, traitRef);
        if (match == null) continue;

        final (implType, instantiatedTrait, goals, implSubstitutions) = match;
        final isBlanket = impl.type is HirType_Parameter;
        if (isBlanket) {
          final goal = solverGoalOf(
            instantiatedTrait,
            implType,
            normalizer: signatures.normalizer,
          );
          if (goal == null) continue;
          final solution = signatures.solverEnvironment.solve(
            goal,
            solverContext.conditions,
            opaqueVariables: solverContext.opaqueVariables,
          );
          if (!solution.isUnique) continue;
        }

        _addTraitFunctions(
          candidates,
          instantiatedTrait,
          implType,
          name,
          group: isBlanket ? _blanketTraitGroup : _traitGroup,
          goals: goals,
          impl: impl,
          implSubstitutions: implSubstitutions,
        );
      }
    }
    return candidates;
  }

  /// An inherent function shadows a trait one of the same name, as in Rust, so
  /// adding a trait impl can never change what an existing call means.
  static const _inherentGroup = 0;
  static const _traitGroup = 1;
  static const _blanketTraitGroup = 2;

  void _addInherentFunctions(
    List<CandidateSource> candidates,
    LoweredImpl impl,
    HirType receiver,
    String name,
  ) {
    final implSubstitutions = _matchImplType(impl, receiver);
    if (implSubstitutions == null) return;

    for (final id in impl.inherentFunctionIds) {
      final function = signatures.functions[id];
      if (function == null) continue;
      if (function.signature.name != name) continue;

      candidates.add(
        CandidateSource(
          function: function,
          preset: implSubstitutions,
          group: _inherentGroup,
          goals: _boundGoals(impl.typeParameters, implSubstitutions),
        ),
      );
    }
  }

  /// Candidates for `Type.name(…)`, i.e. the functions of every inherent impl
  /// on that type. Unlike [memberCandidates] no receiver is prepended, so
  /// `Int.divideTruncating(a, b)` and `a.divideTruncating(b)` both work.
  ///
  /// The impl's type parameters become variables in [variables] rather than
  /// being pinned down by a receiver, so `Maybe.none()` infers its element
  /// type from the call site.
  List<CandidateSource> staticCandidates(
    DeclarationId declaration,
    String name,
    TypeVariables variables,
  ) {
    final candidates = <CandidateSource>[];
    for (final impl in signatures.impls) {
      if (!impl.isInherent) continue;
      final type = impl.type;
      if (type is! HirType_Named || type.declaration != declaration) continue;

      final substitutions = {
        for (final parameter in impl.typeParameters)
          parameter.id: variables.fresh(),
      };
      for (final id in impl.inherentFunctionIds) {
        final function = signatures.functions[id];
        if (function == null) continue;
        if (function.signature.name != name) continue;

        candidates.add(
          CandidateSource(
            function: function,
            preset: substitutions,
            group: _inherentGroup,
            goals: _boundGoals(impl.typeParameters, substitutions),
          ),
        );
      }
    }
    return candidates;
  }

  (HirType, HirTrait, List<SolverGoal>, Map<TypeParameterId, HirType>)?
  _matchImpl(LoweredImpl impl, HirType receiver, HirTrait traitRef) {
    final resolved = _matchImplType(impl, receiver);
    if (resolved == null) return null;

    return (
      impl.type.substitute(resolved),
      traitRef.substitute(resolved),
      _boundGoals(impl.typeParameters, resolved),
      resolved,
    );
  }

  /// Unifies the impl's self type with [receiver] in a throwaway store, so a
  /// failed attempt leaves nothing behind. Returns what that pins the impl's
  /// type parameters down to, or `null` if the impl doesn't apply.
  Map<TypeParameterId, HirType>? _matchImplType(
    LoweredImpl impl,
    HirType receiver,
  ) {
    final scratch = TypeVariables();
    final substitutions = {
      for (final parameter in impl.typeParameters)
        parameter.id: scratch.fresh(),
    };

    final implType = impl.type.substitute(substitutions);
    if (!_unifier(scratch).unify(implType, receiver)) return null;

    return {
      for (final MapEntry(:key, :value) in substitutions.entries)
        key: scratch.finalize(value),
    };
  }

  void _addTraitFunctions(
    List<CandidateSource> candidates,
    HirTrait traitRef,
    HirType selfType,
    String name, {
    required int group,
    required List<SolverGoal> goals,
    LoweredImpl? impl,
    Map<TypeParameterId, HirType> implSubstitutions = const {},
    bool onlyDispatchable = false,
  }) {
    final trait = signatures.traits[traitRef.declaration];
    if (trait == null) return;

    final traitSubstitutions = {
      ...HirType.buildEnvironment(trait.typeParameters, traitRef.typeArguments),
      TypeParameterId.self(trait.id): selfType,
    };

    for (final traitFunctionId in trait.functionIds) {
      final traitFunction = signatures.functions[traitFunctionId];
      if (traitFunction == null) continue;
      if (traitFunction.signature.name != name) continue;
      if (onlyDispatchable &&
          whyNotDispatchable(traitFunction.signature) != null) {
        continue;
      }

      // An impl that doesn't override a defaulted trait function inherits it.
      // Without an impl the receiver is a type parameter, so the trait
      // function itself is the callee and needs no body.
      final implFunctionId = impl?.functionsByTraitFunction[traitFunctionId];
      if (impl != null &&
          implFunctionId == null &&
          traitFunction.ast?.body == null) {
        continue;
      }

      // An impl function's parameters are written in terms of the impl's own
      // type parameters, so those substitutions have to travel with it.
      candidates.add(
        CandidateSource(
          function: implFunctionId == null
              ? traitFunction
              : signatures.functions[implFunctionId]!,
          preset: {...traitSubstitutions, ...implSubstitutions},
          group: group,
          goals: goals,
        ),
      );
    }
  }

  /// Candidates for a bare `name(…)` call. Group 0 is reserved for a local of
  /// function type, which body lowering handles before asking here.
  List<CandidateSource> freeCandidates(ImportScope scope, String name) {
    final found = scope.lookup(name);
    if (found is! NamedFunctions) return const [];

    return [
      for (final id in found.ids)
        if (signatures.functions[id] case final function?)
          CandidateSource(
            function: function,
            preset: const {},
            group: 1 + scope.layerOf(id),
            goals: const [],
          ),
    ];
  }

  /// Picks the best candidate for the given arguments.
  ///
  /// Only the lowest non-empty group is considered, so a member always wins
  /// over an import. Within it, applicable candidates are ranked by
  /// specificity, and a non-generic candidate beats a generic one on a tie.
  ///
  /// The winner is re-instantiated in [variables], committing its bindings;
  /// rejected candidates leave none.
  OverloadResolution resolve({
    required List<CandidateSource> candidates,
    required List<HirType> argumentTypes,
    required List<HirType>? typeArguments,
    required TypeVariables variables,
    required SolverContext solverContext,
  }) {
    if (candidates.isEmpty) return const OverloadNotFound([]);

    final rejections = <(HirFunctionSignature, String)>[];
    var applicable = <(CandidateSource, OverloadCandidate)>[];

    final groups = candidates.map((it) => it.group).toSet().sorted();
    for (final group in groups) {
      for (final source in candidates.where((it) => it.group == group)) {
        final scratch = variables.speculate();
        final candidate = source.instantiate(scratch);
        final rejection = _reject(
          candidate,
          argumentTypes,
          typeArguments,
          scratch,
          solverContext,
        );
        if (rejection == null) {
          applicable.add((source, candidate));
        } else {
          rejections.add((source.signature, rejection));
        }
      }
      if (applicable.isNotEmpty) break;
    }

    if (applicable.isEmpty) return OverloadNotFound(rejections);

    if (applicable.length > 1) {
      final best = _mostSpecific(
        applicable.map((it) => it.$2).toList(),
        solverContext,
      );
      if (best == null) {
        return OverloadAmbiguous(applicable.map((it) => it.$1).toList());
      }
      applicable = applicable.where((it) => identical(it.$2, best)).toList();
    }

    final winner = applicable.single.$1.instantiate(variables);
    _reject(winner, argumentTypes, typeArguments, variables, solverContext);
    return OverloadResolved(winner);
  }

  /// `null` when the candidate applies, otherwise why it doesn't.
  String? _reject(
    OverloadCandidate candidate,
    List<HirType> argumentTypes,
    List<HirType>? typeArguments,
    TypeVariables variables,
    SolverContext solverContext,
  ) {
    final original = candidate.source.signature;
    if (typeArguments != null &&
        typeArguments.length != original.typeParameters.length) {
      return 'wrong number of type arguments';
    }
    if (argumentTypes.length != candidate.signature.parameters.length) {
      return 'wrong number of arguments';
    }

    final unifier = _unifier(variables);

    if (typeArguments != null) {
      for (final (index, parameter) in original.typeParameters.indexed) {
        if (!unifier.unify(
          candidate.substitutions[parameter.id]!,
          typeArguments[index],
        )) {
          return 'type arguments are not assignable';
        }
      }
    }

    for (final (index, parameter) in candidate.signature.parameters.indexed) {
      final argumentType = argumentTypes[index];
      if (unifier.assignable(argumentType, parameter.type) ||
          canCoerce(argumentType, parameter.type, variables, solverContext)) {
        continue;
      }

      final expected = variables.resolve(parameter.type);
      final got = variables.resolve(argumentType);
      // A generic parameter already pinned to one type by another argument is
      // the usual way to end up here when mixing types.
      final hint = original.parameters[index].type is HirType_Parameter
          ? '; to mix types, annotate the expected type with a trait, e.g. '
                '`let items: List[MyTrait] = …`'
          : '';
      return 'expected `$expected` for `${parameter.name}`, got `$got`$hint';
    }

    final resolvedSubstitutions = {
      for (final MapEntry(:key, :value) in candidate.substitutions.entries)
        key: variables.resolve(value),
    };
    // Bounds over a type that isn't pinned down yet can't be decided. Treating
    // them as unproven would reject candidates that a later constraint makes
    // valid, and turn one earlier error into a cascade of them.
    final hasUnknown = resolvedSubstitutions.values.any(
      (it) => it.containsError || variables.isUnresolved(it),
    );
    for (final goal in [
      ...candidate.source.goals,
      ...solverGoalsOfBounds(
        original.typeParameters,
        normalizer: signatures.normalizer,
      ),
    ]) {
      if (hasUnknown) continue;

      final solution = signatures.solverEnvironment.solve(
        substituteGoal(
          goal,
          resolvedSubstitutions,
          normalizer: signatures.normalizer,
        ),
        solverContext.conditions,
        opaqueVariables: solverContext.opaqueVariables,
      );
      switch (solution) {
        case SolverSolution_Unique():
          continue;
        case SolverSolution_Ambiguous(:final cause):
          final lines = cause.describe();
          return [
            'the bound `$goal` ${lines.first}',
            ...lines.skip(1),
          ].join('\n');
        case SolverSolution_Impossible():
          return 'the bound `$goal` is not satisfied';
      }
    }

    return null;
  }

  /// The single candidate that every other one is at least as general as.
  OverloadCandidate? _mostSpecific(
    List<OverloadCandidate> candidates,
    SolverContext solverContext,
  ) {
    bool isMoreSpecific(OverloadCandidate a, OverloadCandidate b) =>
        _isMoreSpecific(a, b, solverContext);

    OverloadCandidate? best;
    for (final candidate in candidates) {
      if (best == null || isMoreSpecific(candidate, best)) best = candidate;
    }
    if (best == null) return null;

    for (final candidate in candidates) {
      if (identical(candidate, best)) continue;
      if (!isMoreSpecific(best, candidate)) return null;
    }
    return best;
  }

  bool _isMoreSpecific(
    OverloadCandidate a,
    OverloadCandidate b,
    SolverContext solverContext,
  ) {
    if (!_acceptsAllOf(a, b, solverContext)) return false;
    // Interchangeable both ways: prefer the concrete signature, as Kotlin does
    // when a generic and a non-generic overload both apply.
    if (_acceptsAllOf(b, a, solverContext)) return !a.isGeneric && b.isGeneric;
    return true;
  }

  /// Whether every argument [a] accepts is also accepted by [b], so `f(Label)`
  /// is more specific than `f(Widget)` when `Label` implements `Widget`.
  bool _acceptsAllOf(
    OverloadCandidate a,
    OverloadCandidate b,
    SolverContext solverContext,
  ) {
    if (a.signature.parameters.length != b.signature.parameters.length) {
      return false;
    }
    final variables = TypeVariables();
    final unifier = _unifier(variables);
    for (final (index, parameter) in a.signature.parameters.indexed) {
      final other = b.signature.parameters[index].type;
      if (!unifier.assignable(parameter.type, other) &&
          !canCoerce(parameter.type, other, variables, solverContext)) {
        return false;
      }
    }
    return true;
  }

  /// Whether a value of type [from] can be boxed into the trait object [to].
  ///
  /// Only decidable once both are known: a type variable could still turn out
  /// to be the object type itself.
  bool canCoerce(
    HirType from,
    HirType to,
    TypeVariables variables,
    SolverContext solverContext,
  ) {
    final target = signatures.normalizer.normalize(variables.resolve(to));
    if (target is! HirType_Dyn || variables.isUnresolved(target)) return false;
    final source = signatures.normalizer.normalize(variables.resolve(from));
    if (variables.isUnresolved(source)) return false;

    // Every vtable starts with the concrete type's ID, so any object can be
    // reread as an `Any`. Other upcasts would need supertraits.
    if (source is HirType_Dyn) {
      return signatures.wellKnownTypes.isAny(target.trait.declaration);
    }

    final goal = solverGoalOf(
      target.trait,
      source,
      normalizer: signatures.normalizer,
    );
    if (goal == null) return false;
    return signatures.solverEnvironment
        .solve(
          goal,
          solverContext.conditions,
          opaqueVariables: solverContext.opaqueVariables,
        )
        .isUnique;
  }
}
