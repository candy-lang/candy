import '../hir/hir/builtins.dart';
import '../hir/hir/hir.dart';
import '../hir/hir/ids.dart';
import 'mono/ids.dart';
import 'mono/mono.dart';
import 'monomorphizing/bodies.dart';
import 'monomorphizing/impls.dart';
import 'monomorphizing/types.dart';

/// Monomorphizes everything `main` reaches.
///
/// [hir] must be free of errors; anything left over from a failed lowering
/// throws a [StateError] rather than being reported. Returns `null` when there
/// is no `main` function, which HIR lowering already reported.
Mono? hirToMono(Hir hir) {
  final mainFunction = hir.mainFunction;
  if (mainFunction == null) return null;
  return Monomorphizer(hir).run(mainFunction);
}

class Monomorphizer {
  Monomorphizer(this.hir)
    : types = TypeMonomorphizer(hir),
      impls = ImplResolver(hir);

  final Hir hir;
  final TypeMonomorphizer types;
  final ImplResolver impls;

  /// A `null` value marks an instantiation that is currently being lowered, so
  /// recursion doesn't lower it a second time.
  final _functions = <MonoFunctionId, MonoFunction?>{};
  final _assignments = <DeclarationId, MonoAssignment?>{};
  final _assignmentInitializationOrder = <DeclarationId>[];

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
      body: switch (call.body) {
        HirBodyOrBuiltin_Body(:final body) => .body(
          BodyMonomorphizer(this, environment).lower(body),
        ),
        HirBodyOrBuiltin_Builtin(:final function) => .builtin(
          function: function,
          substitutions: {
            for (final parameter in call.signature.typeParameters)
              parameter.id: types.monomorphize(parameter.type, environment),
          },
        ),
      },
    );
    return monoId;
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
    );
    _assignmentInitializationOrder.add(id);
    return id;
  }
}
