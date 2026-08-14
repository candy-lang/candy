import 'package:deranged/deranged.dart';
import 'package:supernova/supernova.dart';

import '../ast.dart';
import '../context.dart';
import '../error.dart';
import '../lowering/bodies.dart';
import '../lowering/declarations.dart';
import '../lowering/modules.dart';
import '../lowering/signatures.dart';
import '../lowering/types.dart';
import '../position.dart';
import 'hir/hir.dart';
import 'hir/ids.dart';

Hir astToHir(Context context, List<Module> modules) =>
    _Lowerer(context, modules)._lower();

class _Lowerer {
  _Lowerer(this.context, this.modules);

  final Context context;
  final List<Module> modules;

  final _ids = HirIdGenerator();
  final _globalTypes = <DeclarationId, HirType>{};
  final _assignments = <DeclarationId, HirAssignment>{};
  final _functions = <DeclarationId, HirFunction>{};
  final _traitFunctions = <DeclarationId, HirTraitFunction>{};

  late final CollectedDeclarations _declarations;
  late final Signatures _signatures;

  void _error(SourceSpan span, String message) =>
      context.addError(CompilerError(span: span, message: message));

  Hir _lower() {
    _declarations = collectDeclarations(context, modules);
    _signatures = lowerSignatures(context, _declarations);

    _addBuiltinFunctions();
    _lowerAssignments();
    _lowerFunctions();

    return Hir(
      solverEnvironment: _signatures.solverEnvironment,
      typeDeclarations: _signatures.typeDeclarations,
      traits: _buildTraits(),
      impls: _buildImpls(),
      assignments: _assignments,
      functions: _functions,
      mainFunction: _findMainFunction(),
    );
  }

  void _addBuiltinFunctions() {
    for (final MapEntry(key: id, value: builtin)
        in _signatures.builtinIds.entries) {
      _functions[id] = HirFunction(
        signature: _signatures.functions[id]!.signature,
        body: HirBodyOrBuiltin.builtin(builtin),
      );
    }
  }

  BodyLowerer _bodyLowerer(ModuleId module) => BodyLowerer(
    context: context,
    signatures: _signatures,
    declarations: _declarations,
    module: module,
    file: modules.firstWhere((it) => it.id == module).file,
    ids: _ids,
    globalTypes: _globalTypes,
  );

  /// Lowers top-level assignments in dependency order, so an un-annotated one
  /// has a type before anything referring to it is lowered.
  ///
  /// The Rust compiler builds this graph while lowering, which is too late
  /// when the order decides what can be lowered at all — hence the pre-scan.
  void _lowerAssignments() {
    final order = _dependencyOrder();
    for (final id in order) {
      final assignment = _signatures.assignments[id]!;
      final lowerer = _bodyLowerer(id.module);
      final declared = assignment.declaredType;

      final body = lowerer.lower(
        null,
        typeScope: const TypeScope(),
        parameters: const [],
        expectedType: declared,
        span: assignment.ast.displaySpan,
        expression: assignment.ast.value.toNullable(),
      );

      final type = declared ?? body.returnType;
      _globalTypes[id] = type;
      _assignments[id] = HirAssignment(
        id: id,
        name: assignment.name,
        type: type,
        body: body,
        span: assignment.ast.displaySpan,
      );
    }
  }

  /// Topologically sorts assignments by which ones they reference. Cycles are
  /// reported and their members get the error type.
  List<DeclarationId> _dependencyOrder() {
    final dependencies = <DeclarationId, Set<DeclarationId>>{
      for (final assignment in _signatures.assignments.values)
        assignment.id: _referencedAssignments(assignment),
    };

    final order = <DeclarationId>[];
    final visited = <DeclarationId>{};
    final onStack = <DeclarationId>{};

    void visit(DeclarationId id) {
      if (visited.contains(id)) return;
      if (!onStack.add(id)) {
        _error(
          _signatures.assignments[id]!.ast.displaySpan,
          'Cycle in top-level assignments involving `${id.name}`.',
        );
        _globalTypes[id] = const HirType.error();
        return;
      }

      (dependencies[id] ?? const <DeclarationId>{}).forEach(visit);
      onStack.remove(id);
      if (visited.add(id)) order.add(id);
    }

    _signatures.assignments.keys.forEach(visit);
    return order;
  }

  /// Every top-level assignment mentioned by name anywhere in [assignment]'s
  /// initializer. Over-approximating is fine: a shadowed local name only adds
  /// an edge that can't create a false cycle on its own.
  Set<DeclarationId> _referencedAssignments(LoweredAssignment assignment) {
    final scope = _declarations.scopeOf(assignment.id.module);
    final found = <DeclarationId>{};

    void visitExpression(AstExpression? expression) {
      switch (expression) {
        case null:
          return;
        case AstExpression_Identifier(:final identifier):
          final found_ = scope.lookup(identifier.rawText);
          if (found_ is NamedAssignment) found.add(found_.id);
        case AstExpression_Text(:final parts):
          for (final part in parts) {
            if (part is AstTextPart_Interpolation) {
              visitExpression(part.expression.toNullable());
            }
          }
        case AstExpression_Parenthesized(:final parenthesized):
          visitExpression(parenthesized.inner.toNullable());
        case AstExpression_Call(:final call_):
          visitExpression(call_.receiver);
          call_.arguments.argumentsOrDefault.forEach(visitExpression);
        case AstExpression_Navigation(:final navigation):
          visitExpression(navigation.receiver);
        case AstExpression_Lambda(:final lambda):
          visitBody(lambda.body);
        case AstExpression_Body(:final body):
          visitBody(body);
        case AstExpression_Switch(:final switch_):
          visitExpression(switch_.value.toNullable());
          for (final case_ in switch_.cases) {
            visitExpression(case_.expression.toNullable());
          }
        case AstExpression_Int():
          return;
      }
    }

    void visitBodyStatements(AstBody body) {
      for (final statement in body.statements) {
        switch (statement) {
          case AstStatement_Expression(:final expression):
            visitExpression(expression);
          case AstStatement_Assignment(:final assignment):
            visitExpression(assignment.value.toNullable());
        }
      }
    }

    visitBody = visitBodyStatements;
    visitExpression(assignment.ast.value.toNullable());
    return found;
  }

  late void Function(AstBody) visitBody;

  void _lowerFunctions() {
    for (final function in _signatures.functions.values) {
      if (function.isBuiltin) continue;
      if (function.owner != null) continue;
      _functions[function.id] = _lowerFunction(function);
    }

    for (final trait in _signatures.traits.values) {
      for (final id in trait.functionIds) {
        final function = _signatures.functions[id]!;
        _traitFunctions[id] = HirTraitFunction(
          signature: function.signature,
          body: function.ast?.body == null
              ? null
              : switch (_lowerFunction(function).body) {
                  HirBodyOrBuiltin_Body(:final body) => body,
                  HirBodyOrBuiltin_Builtin() => null,
                },
        );
      }
    }

    for (final impl in _signatures.impls) {
      for (final id in [
        ...impl.functionsByTraitFunction.values,
        ...impl.inherentFunctionIds,
      ]) {
        _functions[id] = _lowerFunction(_signatures.functions[id]!);
      }
    }
  }

  HirFunction _lowerFunction(LoweredFunction function) {
    final lowerer = _bodyLowerer(function.id.module);
    final body = lowerer.lower(
      function.ast?.body,
      typeScope: function.typeScope,
      parameters: function.signature.parameters,
      expectedType: function.signature.returnType,
      span: function.ast?.displaySpan ?? lowerer.fallbackSpan,
    );
    return HirFunction(
      signature: function.signature,
      body: HirBodyOrBuiltin.body(body),
    );
  }

  Map<DeclarationId, HirTraitDefinition> _buildTraits() => {
    for (final trait in _signatures.traits.values)
      trait.id: HirTraitDefinition(
        id: trait.id,
        typeParameters: trait.typeParameters,
        solverGoal: trait.solverGoal,
        solverSubgoals: trait.solverSubgoals,
        functions: {
          for (final id in trait.functionIds)
            if (_traitFunctions.containsKey(id)) id: _traitFunctions[id]!,
        },
      ),
  };

  List<HirImpl> _buildImpls() => [
    for (final impl in _signatures.impls)
      HirImpl(
        id: impl.id,
        typeParameters: impl.typeParameters,
        type: impl.type,
        trait: impl.trait,
        functions: {
          for (final MapEntry(key: traitFunctionId, value: implFunctionId)
              in impl.functionsByTraitFunction.entries)
            if (_functions.containsKey(implFunctionId))
              traitFunctionId: _functions[implFunctionId]!,
        },
        inherentFunctions: {
          for (final id in impl.inherentFunctionIds) id: ?_functions[id],
        },
      ),
  ];

  DeclarationId? _findMainFunction() {
    final candidates = _signatures.functions.values
        .where(
          (it) =>
              !it.isBuiltin && it.owner == null && it.signature.name == 'main',
        )
        .toList();

    if (candidates.isEmpty) {
      final anyFile = modules.firstOrNull;
      if (anyFile != null) {
        context.addError(
          CompilerError(
            span: SourceSpan(
              anyFile.file,
              const Offset(0).rangeTo(const Offset(0)),
            ),
            message: 'The program is missing a `main` function.',
          ),
        );
      }
      return null;
    }

    final main = candidates.first;
    final span = main.ast!.displaySpan;
    if (candidates.length > 1) {
      _error(span, 'The `main` function may not be overloaded.');
      return null;
    }
    if (main.signature.parameters.isNotEmpty) {
      _error(span, 'The `main` function must not have parameters.');
      return null;
    }

    final returnType = main.signature.returnType;
    if (returnType is! HirType_Error &&
        returnType != _signatures.wellKnownTypes.intType) {
      _error(span, 'The `main` function must return an `Int`.');
      return null;
    }
    return main.id;
  }
}
