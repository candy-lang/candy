import 'package:supernova/supernova.dart';

import '../ast.dart';
import '../context.dart';
import '../error.dart';
import '../hir/hir/builtins.dart';
import '../hir/hir/hir.dart';
import '../hir/hir/ids.dart';
import '../hir/traits/hir_bridge.dart';
import '../hir/traits/projections.dart';
import '../hir/traits/solver.dart';
import '../hir/traits/solver_values.dart';
import '../position.dart';
import 'declarations.dart';
import 'types.dart';

/// A function together with everything a later pass needs to lower its body.
class LoweredFunction {
  const LoweredFunction({
    required this.signature,
    required this.ast,
    required this.typeScope,
    required this.owner,
  });

  final HirFunctionSignature signature;

  /// `null` for builtins, which have no source.
  final AstFunction? ast;

  /// Type parameters and `Self` from the enclosing trait or impl, plus the
  /// function's own.
  final TypeScope typeScope;

  /// The trait or impl this function belongs to.
  final DeclarationId? owner;

  DeclarationId get id => signature.id;
  bool get isBuiltin => ast == null;
}

class LoweredTrait {
  LoweredTrait({
    required this.id,
    required this.typeParameters,
    required this.solverGoal,
    required this.solverSubgoals,
    required this.associatedTypes,
    required this.functionIds,
    required this.selfBaseType,
    required this.selfTrait,
  });

  final DeclarationId id;
  final List<HirTypeParameter> typeParameters;
  final SolverGoal solverGoal;
  final List<SolverGoal> solverSubgoals;
  final List<HirAssociatedType> associatedTypes;

  /// Filled in once every impl's associated types are known.
  List<DeclarationId> functionIds;
  final HirType selfBaseType;

  /// `Self`'s own trait, i.e. this trait applied to its type parameters.
  final HirTrait selfTrait;

  HirAssociatedType? associatedType(DeclarationId id) =>
      associatedTypes.where((it) => it.id == id).firstOrNull;
}

class LoweredImpl {
  LoweredImpl({
    required this.id,
    required this.typeParameters,
    required this.type,
    required this.trait,
    required this.associatedTypes,
    required this.functionsByTraitFunction,
    required this.inherentFunctionIds,
  });

  final DeclarationId id;
  final List<HirTypeParameter> typeParameters;
  final HirType type;

  /// `null` for an inherent impl, which names no trait.
  final HirErrorOr<HirTrait>? trait;

  /// Keyed by the trait's associated type each one binds.
  final Map<DeclarationId, HirType> associatedTypes;

  /// Both are filled in once every impl's associated types are known.
  Map<DeclarationId, DeclarationId> functionsByTraitFunction;
  List<DeclarationId> inherentFunctionIds;

  bool get isInherent => trait == null;

  ProjectionImpl get forProjection => ProjectionImpl(
    typeParameters: typeParameters,
    type: type,
    trait: switch (trait) {
      null || None() => null,
      Some(:final some) => some,
    },
    associatedTypes: associatedTypes,
  );
}

class LoweredAssignment {
  const LoweredAssignment({
    required this.id,
    required this.name,
    required this.declaredType,
    required this.ast,
  });

  final DeclarationId id;
  final String name;

  /// `null` when the assignment has no annotation and its type is inferred.
  final HirType? declaredType;
  final AstAssignment ast;
}

class Signatures {
  Signatures({
    required this.typeDeclarations,
    required this.traits,
    required this.impls,
    required this.functions,
    required this.assignments,
    required this.solverEnvironment,
    required this.wellKnownTypes,
    required this.builtinIds,
  });

  final Map<DeclarationId, HirTypeDeclaration> typeDeclarations;
  final Map<DeclarationId, LoweredTrait> traits;
  final List<LoweredImpl> impls;
  final Map<DeclarationId, LoweredFunction> functions;
  final Map<DeclarationId, LoweredAssignment> assignments;
  final SolverEnvironment solverEnvironment;
  final WellKnownTypes wellKnownTypes;
  final Map<DeclarationId, BuiltinFunction> builtinIds;

  late final normalizer = TypeNormalizer(
    impls.map((it) => it.forProjection).toList(),
  );

  late final implsById = impls.associateBy((it) => it.id);

  LoweredTrait? traitOwning(DeclarationId functionId) {
    final owner = functions[functionId]?.owner;
    return owner == null ? null : traits[owner];
  }

  LoweredImpl? implOwning(DeclarationId functionId) {
    final owner = functions[functionId]?.owner;
    return owner == null ? null : implsById[owner];
  }
}

Signatures lowerSignatures(
  Context context,
  CollectedDeclarations declarations,
) => _SignatureLowerer(context, declarations)._lower();

class _SignatureLowerer {
  _SignatureLowerer(this.context, this.declarations);

  final Context context;
  final CollectedDeclarations declarations;

  final _typeDeclarations = <DeclarationId, HirTypeDeclaration>{};
  final _traits = <DeclarationId, LoweredTrait>{};
  final _impls = <LoweredImpl>[];
  final _implsById = <DeclarationId, LoweredImpl>{};

  /// Each trait's and impl's type scope, so the later function-lowering phase
  /// doesn't have to rebuild it.
  final _scopes = <DeclarationId, TypeScope>{};
  final _functions = <DeclarationId, LoweredFunction>{};
  final _assignments = <DeclarationId, LoweredAssignment>{};
  final _builtinIds = <DeclarationId, BuiltinFunction>{};
  final _solverEnvironment = SolverEnvironment();
  final _ids = HirIdGenerator();

  void _error(SourceSpan span, String message) =>
      context.addError(CompilerError(span: span, message: message));

  TypeLowerer _lowererFor(ModuleId module) => TypeLowerer(
    context: context,
    declarations: declarations,
    scope: declarations.scopeOf(module),
  );

  /// Only valid once every impl head is lowered, which is why it's lazy.
  late final TypeNormalizer _normalizer = TypeNormalizer([
    for (final impl in _impls) impl.forProjection,
  ]);

  Signatures _lower() {
    _addBuiltins();

    declarations.ofKind<CollectedStruct>().forEach(_lowerStruct);
    declarations.ofKind<CollectedEnum>().forEach(_lowerEnum);

    // Heads first: a function signature anywhere may project an associated
    // type off any impl, so no signature can be lowered until every impl has
    // bound its associated types.
    declarations.ofKind<CollectedTrait>().forEach(_lowerTraitHead);
    declarations.ofKind<CollectedImpl>().forEach(_lowerImplHead);

    declarations.ofKind<CollectedTrait>().forEach(_lowerTraitFunctions);
    declarations.ofKind<CollectedImpl>().forEach(_lowerImplFunctions);
    declarations
        .ofKind<CollectedFunction>()
        .where((it) => it.id.parent == null)
        .forEach(_lowerTopLevelFunction);
    declarations.ofKind<CollectedAssignment>().forEach(_lowerAssignment);

    declarations.ofKind<CollectedImpl>().forEach(_checkImpl);
    _checkOverloads();

    return Signatures(
      typeDeclarations: _typeDeclarations,
      traits: _traits,
      impls: _impls,
      functions: _functions,
      assignments: _assignments,
      solverEnvironment: _solverEnvironment,
      wellKnownTypes: declarations.wellKnownTypes,
      builtinIds: _builtinIds,
    );
  }

  void _addBuiltins() {
    for (final builtin in BuiltinFunction.values) {
      final signature = builtin.signature(declarations.wellKnownTypes, _ids);
      _builtinIds[builtin.id] = builtin;
      _functions[builtin.id] = LoweredFunction(
        signature: signature,
        ast: null,
        typeScope: TypeScope(typeParameters: signature.typeParameters),
        owner: null,
      );
    }
  }

  void _lowerStruct(CollectedStruct struct) {
    final lowerer = _lowererFor(struct.module);
    final nameSpan = struct.ast.name.toNullable()!.span;
    final typeParameters = lowerer.lowerTypeParameters(
      struct.id,
      struct.ast.typeParameters,
      const TypeScope(),
    );
    final selfBaseType = HirType.named(
      declaration: struct.id,
      typeArguments: typeParameters.map((it) => it.type).toList(),
    );
    final typeScope = TypeScope(
      typeParameters: typeParameters,
      selfBaseType: selfBaseType,
    );

    final kind = switch (struct.ast.kind) {
      AstStructKind_Builtin() => () {
        final builtin = BuiltinType.fromName(struct.id.name);
        if (builtin == null) {
          _error(nameSpan, 'Unknown builtin type: `${struct.id.name}`.');
          return const HirTypeDeclarationKind.struct(fields: []);
        }
        return HirTypeDeclarationKind.builtin(builtin);
      }(),
      AstStructKind_UserDefined(:final fields) => HirTypeDeclarationKind.struct(
        fields: [
          for (final field in fields)
            if (field.name.toNullable() case final name?)
              HirStructField(
                name: name.rawText,
                type: lowerer.lowerType(field.type.toNullable(), typeScope),
                visibility: field.visibility,
              ),
        ],
      ),
    };

    _typeDeclarations[struct.id] = HirTypeDeclaration(
      id: struct.id,
      typeParameters: typeParameters,
      kind: kind,
      span: nameSpan,
    );
  }

  void _lowerEnum(CollectedEnum enum_) {
    final lowerer = _lowererFor(enum_.module);
    final nameSpan = enum_.ast.name.toNullable()!.span;
    final typeParameters = lowerer.lowerTypeParameters(
      enum_.id,
      enum_.ast.typeParameters,
      const TypeScope(),
    );
    final typeScope = TypeScope(
      typeParameters: typeParameters,
      selfBaseType: HirType.named(
        declaration: enum_.id,
        typeArguments: typeParameters.map((it) => it.type).toList(),
      ),
    );

    _typeDeclarations[enum_.id] = HirTypeDeclaration(
      id: enum_.id,
      typeParameters: typeParameters,
      span: nameSpan,
      kind: HirTypeDeclarationKind.enum_(
        variants: [
          for (final variant in enum_.ast.variants)
            if (variant.name.toNullable() case final name?)
              HirEnumVariant(
                name: name.rawText,
                type: variant.type == null
                    ? null
                    : lowerer.lowerType(variant.type!.toNullable(), typeScope),
              ),
        ],
      ),
    );
  }

  void _lowerTraitHead(CollectedTrait trait) {
    final lowerer = _lowererFor(trait.module);
    final typeParameters = lowerer.lowerTypeParameters(
      trait.id,
      trait.ast.typeParameters,
      const TypeScope(),
    );
    // A trait's `Self` is an opaque type parameter of the trait, unlike an
    // impl's, which stands for the concrete type being implemented.
    final selfBaseType = HirType.parameter(TypeParameterId.self(trait.id));
    final selfTrait = HirTrait(
      declaration: trait.id,
      typeArguments: typeParameters.map((it) => it.type).toList(),
    );
    final typeScope = TypeScope(
      typeParameters: typeParameters,
      selfBaseType: selfBaseType,
      selfTrait: selfTrait,
    );
    _scopes[trait.id] = typeScope;

    // The Rust compiler drops the trait's own type parameters here, so a
    // generic trait's goal never has the same arity as the rule an impl
    // generates for it and the two can't unify.
    final solverGoal = SolverGoal(
      trait: trait.id,
      parameters: [
        ...typeParameters.map((it) => SolverVariable.parameter(it.id).type),
        SolverVariable.parameter(TypeParameterId.self(trait.id)).type,
      ],
    );

    final associatedTypes = <HirAssociatedType>[];
    for (final associatedType in trait.associatedTypes) {
      if (associatedType.ast.value != null) {
        _error(
          associatedType.ast.displaySpan,
          "A trait's associated type can't have a value; only an impl gives "
          'it one.',
        );
      }
      associatedTypes.add(
        HirAssociatedType(
          id: associatedType.id,
          upperBounds: lowerer.lowerBounds(
            associatedType.ast.upperBounds,
            associatedType.ast.displaySpan,
            typeScope,
          ),
        ),
      );
    }

    _traits[trait.id] = LoweredTrait(
      id: trait.id,
      typeParameters: typeParameters,
      solverGoal: solverGoal,
      solverSubgoals: solverGoalsOfBounds(typeParameters),
      associatedTypes: associatedTypes,
      functionIds: const [],
      selfBaseType: selfBaseType,
      selfTrait: selfTrait,
    );
  }

  void _lowerImplHead(CollectedImpl impl) {
    final lowerer = _lowererFor(impl.module);
    final implSpan = impl.ast.implKeyword.span;
    final typeParameters = lowerer.lowerTypeParameters(
      impl.id,
      impl.ast.typeParameters,
      const TypeScope(),
    );
    final outerScope = TypeScope(typeParameters: typeParameters);

    final type = lowerer.lowerType(impl.ast.type.toNullable(), outerScope);
    final astTrait = impl.ast.trait;
    final trait = astTrait == null
        ? null
        : lowerer.lowerTrait(astTrait.toNullable(), implSpan, outerScope);
    final traitRef = switch (trait) {
      null || None() => null,
      Some(:final some) => some,
    };
    final typeScope = TypeScope(
      typeParameters: typeParameters,
      selfBaseType: type,
      selfTrait: traitRef,
    );
    _scopes[impl.id] = typeScope;

    if (trait == null) {
      // A blanket inherent impl would add its functions to every type, which
      // no bound could then constrain. Rust rules them out the same way.
      if (type is HirType_Parameter) {
        _error(implSpan, 'An impl without a trait needs a concrete type.');
      }
    } else if (traitRef != null) {
      final goal = solverGoalOf(traitRef, type);
      if (goal != null) {
        _solverEnvironment.addRule(
          SolverRule(
            goal: goal,
            subgoals: solverGoalsOfBounds(typeParameters),
            origin: impl.id,
          ),
        );
      }
    }

    final lowered = LoweredImpl(
      id: impl.id,
      typeParameters: typeParameters,
      type: type,
      trait: trait,
      associatedTypes: _lowerImplAssociatedTypes(
        impl,
        traitRef,
        lowerer,
        typeScope,
      ),
      functionsByTraitFunction: {},
      inherentFunctionIds: const [],
    );
    _impls.add(lowered);
    _implsById[impl.id] = lowered;
  }

  Map<DeclarationId, HirType> _lowerImplAssociatedTypes(
    CollectedImpl impl,
    HirTrait? traitRef,
    TypeLowerer lowerer,
    TypeScope typeScope,
  ) {
    final bindings = <DeclarationId, HirType>{};
    for (final associatedType in impl.associatedTypes) {
      final span = associatedType.ast.displaySpan;
      if (associatedType.ast.upperBounds.isNotEmpty) {
        _error(
          span,
          "An impl's associated type can't have a bound; only the trait "
          'declares one.',
        );
      }

      if (traitRef == null) {
        _error(span, "An impl without a trait can't bind associated types.");
        continue;
      }

      final traitAssociatedType = declarations.associatedTypeOf(
        traitRef.declaration,
        associatedType.id.name,
      );
      if (traitAssociatedType == null) {
        _error(
          span,
          '`${associatedType.id.name}` is not an associated type of trait '
          '`${traitRef.declaration.name}`.',
        );
        continue;
      }

      final value = associatedType.ast.value;
      if (value == null) {
        _error(span, 'This associated type is missing a value.');
        continue;
      }
      bindings[traitAssociatedType] = lowerer.lowerType(
        value.toNullable(),
        typeScope,
      );
    }
    return bindings;
  }

  void _lowerTraitFunctions(CollectedTrait trait) {
    final typeScope = _scopes[trait.id]!;
    for (final function in trait.functions) {
      _lowerFunctionSignature(function, typeScope, owner: trait.id);
    }
    _traits[trait.id]?.functionIds = trait.functions
        .map((it) => it.id)
        .toList();
  }

  void _lowerImplFunctions(CollectedImpl impl) {
    final typeScope = _scopes[impl.id]!;
    for (final function in impl.functions) {
      _lowerFunctionSignature(function, typeScope, owner: impl.id);
    }

    final lowered = _implsById[impl.id];
    if (lowered != null && lowered.isInherent) {
      lowered.inherentFunctionIds = impl.functions.map((it) => it.id).toList();
    }
  }

  void _checkImpl(CollectedImpl impl) {
    final lowered = _implsById[impl.id];
    if (lowered == null) return;
    if (lowered.trait case Some(some: final traitRef)) {
      _matchImplFunctions(
        impl,
        traitRef,
        lowered.type,
        impl.ast.implKeyword.span,
        lowered.functionsByTraitFunction,
      );
      _checkImplAssociatedTypes(lowered, traitRef, impl.ast.implKeyword.span);
    }
  }

  /// Reports associated types the impl left unbound, and bindings that don't
  /// satisfy the bound the trait declares.
  void _checkImplAssociatedTypes(
    LoweredImpl impl,
    HirTrait traitRef,
    SourceSpan implSpan,
  ) {
    final trait = _traits[traitRef.declaration];
    if (trait == null) return;

    for (final associatedType in trait.associatedTypes) {
      final binding = impl.associatedTypes[associatedType.id];
      if (binding == null) {
        _error(
          implSpan,
          'Missing binding for associated type `${associatedType.name}`.',
        );
        continue;
      }
      if (binding.containsError) continue;

      for (final bound in associatedType.upperBounds) {
        final goal = solverGoalOf(
          bound.substitute(
            HirType.buildEnvironment(
              trait.typeParameters,
              traitRef.typeArguments,
            ),
          ),
          binding,
          normalizer: _normalizer,
        );
        if (goal == null) continue;

        final solution = _solverEnvironment.solve(
          goal,
          solverGoalsOfBounds(impl.typeParameters, normalizer: _normalizer),
          opaqueVariables: solverVariablesOf(impl.typeParameters),
        );
        if (!solution.isUnique) {
          _error(
            implSpan,
            '`$binding` does not satisfy the bound `$bound` on associated '
            'type `${associatedType.name}`.',
          );
        }
      }
    }
  }

  void _matchImplFunctions(
    CollectedImpl impl,
    HirTrait traitRef,
    HirType type,
    SourceSpan implSpan,
    Map<DeclarationId, DeclarationId> functionsByTraitFunction,
  ) {
    final trait = _traits[traitRef.declaration];
    if (trait == null) return;

    // The impl's associated types aren't in here: a projection resolves
    // through the impl once `Self` is substituted, which is what normalizing
    // both signatures below does.
    final substitutions = {
      ...HirType.buildEnvironment(trait.typeParameters, traitRef.typeArguments),
      TypeParameterId.self(trait.id): type,
    };

    final attemptedNames = <String>{};
    for (final function in impl.functions) {
      final implSignature = _functions[function.id]?.signature;
      if (implSignature == null) continue;

      final parentId = trait.functionIds
          .where(
            (it) => _signaturesMatch(
              implSignature,
              _functions[it]!.signature,
              substitutions,
            ),
          )
          .firstOrNull;

      if (parentId == null) {
        attemptedNames.add(implSignature.name);
        final expected = trait.functionIds
            .map((it) => _functions[it]!.signature)
            .where((it) => it.name == implSignature.name)
            .map((it) => _substituteSignature(it, substitutions))
            .toList();
        _error(
          function.ast.displaySpan,
          expected.isEmpty
              ? '`${implSignature.name}` is not part of trait '
                    '`${traitRef.declaration.name}`.'
              : "This function doesn't match the trait's signature.\n"
                    'Expected:${expected.map((it) => '\n• $it').join()}',
        );
        continue;
      }
      functionsByTraitFunction[parentId] = function.id;
    }

    for (final traitFunctionId in trait.functionIds) {
      if (functionsByTraitFunction.containsKey(traitFunctionId)) continue;
      if (_functions[traitFunctionId]!.ast?.body != null) continue;
      // A mismatched signature was already reported; don't pile on.
      if (attemptedNames.contains(traitFunctionId.name)) continue;

      final expected = _substituteSignature(
        _functions[traitFunctionId]!.signature,
        substitutions,
      );
      _error(implSpan, 'Missing implementation of function $expected.');
    }
  }

  bool _signaturesMatch(
    HirFunctionSignature impl,
    HirFunctionSignature trait,
    Map<TypeParameterId, HirType> substitutions,
  ) {
    if (impl.name != trait.name) return false;
    if (impl.typeParameters.length != trait.typeParameters.length) return false;
    if (impl.parameters.length != trait.parameters.length) return false;

    // A trait function's own type parameters stand for themselves in the impl.
    final all = {
      ...substitutions,
      for (final (index, parameter) in trait.typeParameters.indexed)
        parameter.id: impl.typeParameters[index].type,
    };

    for (final (index, parameter) in trait.typeParameters.indexed) {
      final implParameter = impl.typeParameters[index];
      if (implParameter.name != parameter.name) return false;
      final expectedBounds = parameter.upperBounds
          .map((it) => it.substitute(all))
          .toList();
      if (!const ListEquality<HirTrait>().equals(
        implParameter.upperBounds,
        expectedBounds,
      )) {
        return false;
      }
    }

    // Normalized on both sides, so the trait's `Self.Item` and the `Int` the
    // impl writes out are recognized as the same type.
    for (final (index, parameter) in trait.parameters.indexed) {
      final implParameter = impl.parameters[index];
      if (implParameter.name != parameter.name) return false;
      if (_normalizer.normalize(implParameter.type) !=
          _normalizer.normalize(parameter.type.substitute(all))) {
        return false;
      }
    }

    return _normalizer.normalize(impl.returnType) ==
        _normalizer.normalize(trait.returnType.substitute(all));
  }

  HirFunctionSignature _substituteSignature(
    HirFunctionSignature signature,
    Map<TypeParameterId, HirType> substitutions,
  ) => signature.copyWith(
    parameters: signature.parameters
        .map(
          (it) => it.copyWith(
            type: _normalizer.normalize(it.type.substitute(substitutions)),
          ),
        )
        .toList(),
    returnType: _normalizer.normalize(
      signature.returnType.substitute(substitutions),
    ),
  );

  void _lowerTopLevelFunction(CollectedFunction function) =>
      _lowerFunctionSignature(function, const TypeScope(), owner: null);

  void _lowerFunctionSignature(
    CollectedFunction function,
    TypeScope outerScope, {
    required DeclarationId? owner,
  }) {
    final lowerer = _lowererFor(function.module);

    final typeParameters = lowerer.lowerTypeParameters(
      function.id,
      function.ast.typeParameters,
      outerScope,
    );
    final typeScope = outerScope.extend(typeParameters);

    final parameters = <HirParameter>[];
    final seenNames = <String>{};
    for (final parameter
        in function.ast.valueParameters.toNullable()?.valueParameters ??
            const <AstValueParameter>[]) {
      final name = parameter.name.toNullable();
      if (name == null) continue;

      if (!seenNames.add(name.rawText)) {
        _error(name.span, 'Duplicate parameter name: `${name.rawText}`.');
        continue;
      }
      parameters.add(
        HirParameter(
          id: _ids.generate(),
          name: name.rawText,
          type: lowerer.lowerType(parameter.type?.toNullable(), typeScope),
          span: name.span,
        ),
      );
    }

    _functions[function.id] = LoweredFunction(
      signature: HirFunctionSignature(
        id: function.id,
        name: function.id.name,
        typeParameters: typeParameters,
        parameters: parameters,
        returnType: function.ast.returnType == null
            ? declarations.wellKnownTypes.nothingType
            : lowerer.lowerType(function.ast.returnType, typeScope),
        span: function.ast.displaySpan,
      ),
      ast: function.ast,
      typeScope: typeScope,
      owner: owner,
    );
  }

  void _lowerAssignment(CollectedAssignment assignment) {
    final lowerer = _lowererFor(assignment.module);
    final ast = assignment.ast;
    final declaredType = ast.type == null
        ? null
        : lowerer.lowerType(ast.type!.toNullable(), const TypeScope());

    _assignments[assignment.id] = LoweredAssignment(
      id: assignment.id,
      name: assignment.id.name,
      declaredType: declaredType,
      ast: ast,
    );
  }

  static bool _sameTypes(List<HirType> a, List<HirType> b) {
    if (a.length != b.length) return false;
    for (var i = 0; i < a.length; i++) {
      if (a[i] != b[i]) return false;
    }
    return true;
  }

  /// The Rust compiler leaves this as a TODO, so `fun f(a: Int)` twice is
  /// silently accepted there and the call is then reported as ambiguous.
  void _checkOverloads() {
    final byScope = <(DeclarationId?, ModuleId, String), List<DeclarationId>>{};
    for (final function in _functions.values) {
      if (function.isBuiltin) continue;
      final id = function.id;
      byScope.putIfAbsent((id.parent, id.module, id.name), () => []).add(id);
    }

    for (final ids in byScope.values) {
      if (ids.length < 2) continue;

      for (var i = 1; i < ids.length; i++) {
        final signature = _functions[ids[i]]!.signature;
        final clashes = ids
            .take(i)
            .any(
              (it) => _sameTypes(
                _functions[it]!.signature.parameterTypes,
                signature.parameterTypes,
              ),
            );
        if (clashes) {
          _error(
            _functions[ids[i]]!.ast!.displaySpan,
            'A function named `${signature.name}` with the same parameter '
            'types already exists.',
          );
        }
      }
    }
  }
}
