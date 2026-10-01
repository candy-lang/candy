import '../hir/hir/builtins.dart';
import '../hir/hir/hir.dart';
import '../hir/hir/ids.dart';
import '../hir/traits/dyn_compatibility.dart';
import '../hir/traits/projections.dart';
import 'mono/ids.dart';
import 'mono/mono.dart';
import 'monomorphizing/bodies.dart';
import 'monomorphizing/impls.dart';
import 'monomorphizing/types.dart';

/// Monomorphizes everything `main` reaches.
///
/// [hir] must be free of errors; anything left over from a failed lowering
/// throws a [StateError] rather than being reported. Returns `null` when there
/// is no `main` function; only the caller knows whether that is a problem.
Mono? hirToMono(Hir hir) {
  final mainFunction = hir.mainFunction;
  if (mainFunction == null) return null;
  return Monomorphizer(hir).run(mainFunction);
}

class Monomorphizer {
  factory Monomorphizer(Hir hir) {
    final normalizer = normalizerOf(hir);
    return Monomorphizer._(
      hir: hir,
      types: TypeMonomorphizer(hir, normalizer),
      impls: ImplResolver(hir, normalizer),
    );
  }
  Monomorphizer._({
    required this.hir,
    required this.types,
    required this.impls,
  });

  final Hir hir;
  final TypeMonomorphizer types;
  final ImplResolver impls;

  /// A `null` value marks an instantiation that is currently being lowered, so
  /// recursion doesn't lower it a second time.
  final _functions = <MonoFunctionId, MonoFunction?>{};
  final _assignments = <DeclarationId, MonoAssignment?>{};
  final _assignmentInitializationOrder = <DeclarationId>[];

  /// A `null` value marks a vtable whose entries are being lowered, since one
  /// of them may box a value into the same trait object again.
  final _vtables = <MonoVtableId, MonoVtable?>{};
  final _typeIds = <MonoType, int>{};

  Mono run(DeclarationId mainFunction) {
    final main = lowerFunction(mainFunction, const {});

    // The backend calls `panic` for cases the program itself never reaches, so
    // it has to exist even when nothing calls it.
    final panic = BuiltinFunction.panic.id;
    if (hir.functions.containsKey(panic)) lowerFunction(panic, const {});

    return Mono(
      typeDeclarations: types.declarations,
      assignments: {
        for (final MapEntry(:key, :value) in _assignments.entries) key: value!,
      },
      assignmentInitializationOrder: _assignmentInitializationOrder,
      functions: {
        for (final MapEntry(:key, :value) in _functions.entries) key: value!,
      },
      vtables: {
        for (final MapEntry(:key, :value) in _vtables.entries) key: value!,
      },
      typeIds: _typeIds,
      mainFunction: main,
    );
  }

  /// Lowers the instantiation of [id] given by [substitutions], which must
  /// already be concrete, and returns the instance it belongs to.
  MonoFunctionId lowerFunction(
    DeclarationId id,
    Map<TypeParameterId, HirType> substitutions,
  ) {
    final call = _resolve(id, substitutions);
    final monoId = _instanceOf(call);
    if (_functions.containsKey(monoId)) return monoId;
    _functions[monoId] = null;

    final environment = call.substitutions;
    _functions[monoId] = MonoFunction(
      id: monoId,
      parameters: [
        for (final parameter in call.signature.parameters)
          MonoParameter(
            id: parameter.id,
            name: parameter.name,
            type: types.monomorphize(parameter.type, environment),
          ),
      ],
      returnType: types.monomorphize(call.signature.returnType, environment),
      body: switch (call) {
        ResolvedCall(dynSlot: final slot?) => .dynDispatch(slot: slot),
        ResolvedCall(body: HirBodyOrBuiltin_Body(:final body)) => .body(
          BodyMonomorphizer(this, environment).lower(body),
        ),
        ResolvedCall(body: HirBodyOrBuiltin_Builtin(:final function)) =>
          _lowerBuiltin(function, call.signature, environment),
        _ => throw StateError('`$monoId` has no body.'),
      },
      span: call.signature.span,
    );
    return monoId;
  }

  MonoBodyOrBuiltin _lowerBuiltin(
    BuiltinFunction function,
    HirFunctionSignature signature,
    Map<TypeParameterId, HirType> environment,
  ) {
    final substitutions = {
      for (final parameter in signature.typeParameters)
        parameter.id: types.monomorphize(parameter.type, environment),
    };
    if (const <BuiltinFunction>{
      .typeId,
      .typeIdOf,
      .downcast,
    }.contains(function)) {
      substitutions.values.forEach(typeIdOf);
    }
    return .builtin(function: function, substitutions: substitutions);
  }

  /// IDs are handed out in the order types are first asked about, which is
  /// deterministic for a given program.
  int typeIdOf(MonoType type) =>
      _typeIds.putIfAbsent(type, () => _typeIds.length);

  /// The vtable that lets [concreteType] be used as the trait object
  /// [dynType]. Both must already be concrete.
  MonoVtableId vtableFor(HirType concreteType, HirType_Dyn dynType) {
    final concrete = types.monomorphize(concreteType, const {});
    final object = types.monomorphize(dynType, const {}) as MonoType_Dyn;
    final id = MonoVtableId(concreteType: concrete, dynType: object);
    if (_vtables.containsKey(id)) return id;
    _vtables[id] = null;

    final trait = dynType.trait;
    final definition = hir.traits[trait.declaration];
    if (definition == null) {
      throw StateError('Unknown trait: `${trait.declaration}`.');
    }
    final substitutions = {
      ...HirType.buildEnvironment(
        definition.typeParameters,
        trait.typeArguments,
      ),
      TypeParameterId.self(trait.declaration): concreteType,
    };
    _vtables[id] = MonoVtable(
      typeId: typeIdOf(concrete),
      entries: [
        for (final function in vtableSlotsOf(hir, trait.declaration))
          lowerFunction(function, substitutions),
      ],
    );
    return id;
  }

  ResolvedCall _resolve(
    DeclarationId id,
    Map<TypeParameterId, HirType> substitutions,
  ) {
    final function = hir.functions[id];
    if (function == null) return impls.resolveTraitFunction(id, substitutions);
    return ResolvedCall(
      signature: function.signature,
      body: function.body,
      substitutions: substitutions,
    );
  }

  MonoFunctionId _instanceOf(ResolvedCall call) => MonoFunctionId(
    declaration: call.signature.id,
    typeArguments: [
      for (final parameter in impls.relevantTypeParameters(call.signature))
        types.monomorphize(
          call.substitutions[parameter] ??
              (throw StateError(
                'No type argument for `$parameter` when instantiating '
                '`${call.signature.id}`.',
              )),
          const {},
        ),
    ],
  );

  /// Lowers the top-level assignment [id] if it isn't lowered yet, appending it
  /// to the initialization order after everything it reads.
  DeclarationId lowerAssignment(DeclarationId id) {
    if (_assignments.containsKey(id)) return id;
    _assignments[id] = null;

    final assignment = hir.assignments[id];
    if (assignment == null) throw StateError('Unknown assignment: `$id`.');

    final body = BodyMonomorphizer(this, const {}).lower(assignment.body);
    _assignments[id] = MonoAssignment(
      id: id,
      name: assignment.name,
      type: types.monomorphize(assignment.type, const {}),
      body: body,
      span: assignment.span,
    );
    _assignmentInitializationOrder.add(id);
    return id;
  }
}
