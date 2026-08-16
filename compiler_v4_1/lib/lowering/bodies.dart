import 'package:deranged/deranged.dart';
import 'package:supernova/supernova.dart';

import '../ast.dart';
import '../context.dart';
import '../error.dart';
import '../file.dart';
import '../hir/hir/builtins.dart';
import '../hir/hir/hir.dart';
import '../hir/hir/ids.dart';
import '../hir/inference/inference.dart';
import '../hir/inference/overloads.dart';
import '../lexer/token.dart';
import '../position.dart';
import 'declarations.dart';
import 'modules.dart';
import 'signatures.dart';
import 'types.dart';

/// What lowering an expression produced.
sealed class _Lowered {
  const _Lowered();
}

class _Value extends _Lowered {
  const _Value(this.id, this.type);
  final HirId id;
  final HirType type;
}

/// A bare type name, e.g. the `Maybe` in `Maybe.none()`. Only meaningful as
/// the receiver of a call or navigation.
class _TypeReference extends _Lowered {
  const _TypeReference(this.declaration);
  final DeclarationId declaration;
}

class _LoweringError extends _Lowered {
  const _LoweringError();
}

/// An expression whose callee isn't decided yet.
class _PendingCall {
  _PendingCall(this.candidate, this.arguments);
  OverloadCandidate candidate;
  final List<HirId> arguments;
}

class _Entry {
  _Entry({
    required this.id,
    required this.name,
    required this.type,
    required this.kind,
  });

  final HirId id;
  final String? name;
  HirType type;
  HirExpressionKind kind;
  _PendingCall? pendingCall;
}

class BodyLowerer {
  BodyLowerer({
    required this.context,
    required this.signatures,
    required this.declarations,
    required this.module,
    required this.file,
    required this.ids,
    required this.globalTypes,
  }) : variables = TypeVariables(),
       resolver = OverloadResolver(signatures) {
    unifier = Unifier(
      variables: variables,
      wellKnownTypes: signatures.wellKnownTypes,
      normalize: signatures.normalizer.normalize,
    );
    typeLowerer = TypeLowerer(
      context: context,
      declarations: declarations,
      scope: declarations.scopeOf(module),
    );
  }

  final Context context;
  final Signatures signatures;
  final CollectedDeclarations declarations;
  final ModuleId module;
  final SourceFile file;
  final HirIdGenerator ids;

  /// Used when an expression has no token at all, which means the parser
  /// already reported an error there.
  late final fallbackSpan = SourceSpan(
    file,
    const Offset(0).rangeTo(const Offset(0)),
  );

  /// Types of top-level assignments, filled in as they're lowered.
  final Map<DeclarationId, HirType> globalTypes;

  final TypeVariables variables;
  final OverloadResolver resolver;
  late final Unifier unifier;
  late final TypeLowerer typeLowerer;

  ImportScope get scope => declarations.scopeOf(module);
  WellKnownTypes get types => signatures.wellKnownTypes;

  late TypeScope typeScope;
  late SolverContext solverContext;

  final _locals = <(String, HirId, HirType)>[];
  final _retries = <void Function()>[];
  final _inferredTypeChecks = <(String, SourceSpan, HirType Function())>[];

  /// Reports, once the body has settled, that [name]'s type never got pinned
  /// down. Deferred because a lambda parameter is often bound by the overload
  /// resolution of the call the lambda is an argument to, which runs after the
  /// lambda's own body is finished.
  void checkInferred(String name, SourceSpan span, HirType Function() type) =>
      _inferredTypeChecks.add((name, span, type));

  void _error(SourceSpan span, String message) =>
      context.addError(CompilerError(span: span, message: message));

  /// Lowers one function or assignment body.
  HirBody lower(
    AstBody? ast, {
    required TypeScope typeScope,
    required List<HirParameter> parameters,
    required HirType? expectedType,
    required SourceSpan span,
    AstExpression? expression,
  }) {
    this.typeScope = typeScope;
    solverContext = resolver.solverContextFor(typeScope);
    _locals
      ..clear()
      ..addAll(parameters.map((it) => (it.name, it.id, it.type)));

    final builder = _Builder(this);
    if (expression != null) {
      builder.lowerExpression(expression, expectedType);
    } else if (ast != null) {
      builder.lowerStatements(ast.statements, expectedType);
    } else {
      _error(span, 'No function body provided.');
      builder.pushPanic('No function body provided');
    }

    _runRetries();

    for (final (name, span, type) in _inferredTypeChecks) {
      if (variables.isUnresolved(type())) {
        _error(
          span,
          "The type of `$name` can't be inferred. Add a type annotation.",
        );
      }
    }
    return builder.finish();
  }

  /// Retries calls that were blocked on a type that wasn't known yet, until
  /// nothing more becomes resolvable.
  void _runRetries() {
    while (_retries.isNotEmpty) {
      final pending = [..._retries];
      _retries.clear();
      final before = pending.length;
      for (final retry in pending) {
        retry();
      }
      if (_retries.length >= before) {
        // No progress: run each once more so it reports its own error.
        final stuck = [..._retries];
        _retries.clear();
        for (final retry in stuck) {
          retry();
        }
        return;
      }
    }
  }
}

class _Builder {
  _Builder(this.lowerer);

  final BodyLowerer lowerer;
  final _entries = <_Entry>[];

  TypeVariables get variables => lowerer.variables;
  Unifier get unifier => lowerer.unifier;
  WellKnownTypes get types => lowerer.types;

  void _error(SourceSpan span, String message) => lowerer._error(span, message);

  HirBody finish() => HirBody([
    for (final entry in _entries)
      HirBodyEntry(
        id: entry.id,
        name: entry.name,
        expression: HirExpression(
          kind: _finalKind(entry),
          type: variables.finalize(entry.type),
        ),
      ),
  ]);

  HirExpressionKind _finalKind(_Entry entry) {
    final pending = entry.pendingCall;
    if (pending == null) return entry.kind;
    return HirExpressionKind.call(
      function: pending.candidate.id,
      substitutions: {
        for (final MapEntry(:key, :value)
            in pending.candidate.substitutions.entries)
          key: variables.finalize(value),
      },
      arguments: pending.arguments,
    );
  }

  HirId push(HirExpressionKind kind, HirType type, {String? name}) {
    final id = lowerer.ids.generate();
    if (name != null) lowerer._locals.add((name, id, type));
    _entries.add(_Entry(id: id, name: name, type: type, kind: kind));
    return id;
  }

  HirId pushError() =>
      push(const HirExpressionKind.error(), const HirType.error());

  HirId pushNothing() =>
      push(const HirExpressionKind.createStruct(fields: []), types.nothingType);

  void pushPanic(String message) {
    final text = push(HirExpressionKind.text(message), types.textType);
    push(
      HirExpressionKind.call(
        function: BuiltinFunction.panic.id,
        substitutions: const {},
        arguments: [text],
      ),
      types.neverType,
    );
  }

  /// Runs [body] in a nested builder that sees this one's locals but doesn't
  /// leak its own.
  HirBody buildNested(void Function(_Builder) body) {
    final saved = [...lowerer._locals];
    final builder = _Builder(lowerer);
    body(builder);
    lowerer._locals
      ..clear()
      ..addAll(saved);
    return builder.finish();
  }

  (HirId, HirType) lowerStatements(
    List<AstStatement> statements,
    HirType? expectedType,
  ) {
    final savedLocals = [...lowerer._locals];
    (HirId, HirType)? last;

    for (final (index, statement) in statements.indexed) {
      final isLast = index == statements.length - 1;
      switch (statement) {
        case AstStatement_Assignment(:final assignment):
          final name = assignment.name.toNullable();
          final declared = assignment.type == null
              ? null
              : lowerer.typeLowerer.lowerType(
                  assignment.type!.toNullable(),
                  lowerer.typeScope,
                );
          final value = assignment.value.toNullable();
          final (id, type) = value == null
              ? (pushError(), const HirType.error())
              : lowerExpression(value, declared);
          if (name != null) {
            final bindingType = declared ?? type;
            push(
              HirExpressionKind.reference(id),
              bindingType,
              name: name.rawText,
            );
            if (declared == null) {
              lowerer.checkInferred(name.rawText, name.span, () => bindingType);
            }
          }
          last = null;

        case AstStatement_Expression(:final expression):
          last = lowerExpression(expression, isLast ? expectedType : null);
      }
    }

    lowerer._locals
      ..clear()
      ..addAll(savedLocals);

    if (last == null) {
      final id = pushNothing();
      last = (id, types.nothingType);
    }
    return (last.$1, expectedType ?? last.$2);
  }

  (HirId, HirType) lowerExpression(
    AstExpression expression,
    HirType? expectedType,
  ) {
    final span = _spanOf(expression);
    switch (lowerRaw(expression, expectedType)) {
      case _Value(:final id, :final type):
        if (expectedType != null && !unifier.assignable(type, expectedType)) {
          _error(
            span,
            'Expected type `${variables.resolve(expectedType)}`, '
            'got `${variables.resolve(type)}`.',
          );
          return (pushError(), const HirType.error());
        }
        return (id, type);
      case _TypeReference():
        _error(span, 'A type must be instantiated to be used as a value.');
        return (pushError(), const HirType.error());
      case _LoweringError():
        return (pushError(), const HirType.error());
    }
  }

  _Lowered lowerRaw(AstExpression expression, HirType? expectedType) {
    switch (expression) {
      case AstExpression_Int(:final value):
        return _Value(
          push(HirExpressionKind.int(value), types.intType),
          types.intType,
        );

      case AstExpression_Text(:final parts):
        return _Value(_lowerText(parts), types.textType);

      case AstExpression_Identifier(:final identifier):
        return _lowerIdentifier(identifier);

      case AstExpression_Parenthesized(:final parenthesized):
        final inner = parenthesized.inner.toNullable();
        return inner == null
            ? const _LoweringError()
            : lowerRaw(inner, expectedType);

      case AstExpression_Body(:final body):
        final (id, type) = lowerStatements(body.statements, expectedType);
        return _Value(id, type);

      case AstExpression_Navigation(:final navigation):
        return _lowerNavigation(navigation);

      case AstExpression_Call(:final call_):
        return _lowerCall(call_);

      case AstExpression_Lambda(:final lambda):
        return _lowerLambda(lambda, expectedType);

      case AstExpression_Switch(:final switch_):
        return _lowerSwitch(switch_, expectedType);
    }
  }

  HirId _lowerText(List<AstTextPart> parts) {
    final pieces = <HirId>[];
    for (final part in parts) {
      switch (part) {
        case AstTextPart_Text(:final value):
          pieces.add(push(HirExpressionKind.text(value), types.textType));
        case AstTextPart_Interpolation(:final expression):
          final inner = expression.toNullable();
          pieces.add(
            inner == null
                ? pushError()
                : lowerExpression(inner, types.textType).$1,
          );
      }
    }

    if (pieces.isEmpty) {
      return push(const HirExpressionKind.text(''), types.textType);
    }
    return pieces.reduce(
      (a, b) => push(
        HirExpressionKind.call(
          function: BuiltinFunction.textConcat.id,
          substitutions: const {},
          arguments: [a, b],
        ),
        types.textType,
      ),
    );
  }

  _Lowered _lowerIdentifier(Token identifier) {
    final name = identifier.rawText;

    for (final (localName, id, type) in lowerer._locals.reversed) {
      if (localName == name) {
        return _Value(push(HirExpressionKind.reference(id), type), type);
      }
    }

    switch (lowerer.scope.lookup(name)) {
      case NamedAssignment(:final id):
        final type = lowerer.globalTypes[id] ?? const HirType.error();
        return _Value(push(HirExpressionKind.globalReference(id), type), type);
      case NamedType(:final id) || NamedTrait(:final id):
        return _TypeReference(id);
      case NamedFunctions():
        _error(
          identifier.span,
          'Referencing a function as a value is not supported yet.',
        );
        return const _LoweringError();
      case NamedAmbiguous(:final originsText):
        _error(
          identifier.span,
          'Ambiguous name `$name`, imported from $originsText.',
        );
        return const _LoweringError();
      case null:
        if (lowerer.typeScope.find(name) != null) {
          _error(identifier.span, "A type parameter can't be used as a value.");
          return const _LoweringError();
        }
        _error(identifier.span, 'Unknown reference: `$name`.');
        return const _LoweringError();
    }
  }

  _Lowered _lowerNavigation(AstNavigation navigation) {
    final key = navigation.key.toNullable();
    if (key == null) return const _LoweringError();

    final receiver = lowerRaw(navigation.receiver, null);
    switch (receiver) {
      case _Value(:final id, :final type):
        return _lowerFieldAccess(id, type, key);
      case _TypeReference(:final declaration):
        final kind = lowerer.signatures.typeDeclarations[declaration]?.kind;
        if (kind is HirTypeDeclarationKind_Enum &&
            kind.variants.any((it) => it.name == key.rawText)) {
          _error(
            key.span,
            'Enum variant `${declaration.name}.${key.rawText}` must be called '
            'to create a value.',
          );
        } else {
          _error(
            key.span,
            '`${declaration.name}` has no member `${key.rawText}`.',
          );
        }
        return const _LoweringError();
      case _LoweringError():
        return const _LoweringError();
    }
  }

  _Lowered _lowerFieldAccess(HirId receiverId, HirType receiver, Token key) {
    final resolved = variables.resolve(receiver);
    if (resolved is HirType_Error) return const _LoweringError();
    if (resolved is! HirType_Named) {
      _error(
        key.span,
        variables.isUnresolved(resolved)
            ? 'The type of this value is not known here. Add a type annotation.'
            : 'A value of type `$resolved` has no field `${key.rawText}`.',
      );
      return const _LoweringError();
    }

    final declaration =
        lowerer.signatures.typeDeclarations[resolved.declaration];
    final kind = declaration?.kind;
    if (kind is HirTypeDeclarationKind_Struct) {
      final field = kind.fields
          .where((it) => it.name == key.rawText)
          .firstOrNull;
      if (field != null) {
        if (!_isFieldVisible(field, resolved.declaration)) {
          _error(key.span, _fieldVisibilityMessage(field, key.rawText));
          return const _LoweringError();
        }

        final type = field.type.substitute(
          HirType.buildEnvironment(
            declaration!.typeParameters,
            resolved.typeArguments,
          ),
        );
        return _Value(
          push(
            HirExpressionKind.structAccess(
              struct: receiverId,
              field: key.rawText,
            ),
            type,
          ),
          type,
        );
      }
    }

    _error(
      key.span,
      'A value of type `$resolved` has no field `${key.rawText}`.',
    );
    return const _LoweringError();
  }

  bool _isFieldVisible(HirStructField field, DeclarationId owner) =>
      switch (field.visibility) {
        .private => owner.module == lowerer.module,
        .package => owner.module.packageName == lowerer.module.packageName,
        .public => true,
      };

  String _fieldVisibilityMessage(HirStructField field, String name) =>
      field.visibility == .private
      ? 'The field `$name` is private.'
      : 'The field `$name` is not visible here.';

  _Lowered _lowerLambda(AstLambda lambda, HirType? expectedType) {
    final expected = expectedType == null
        ? null
        : variables.shallow(expectedType);
    final expectedFunction = expected is HirType_Function ? expected : null;

    final parameters = <HirParameter>[];
    for (final (index, parameter)
        in lambda.parameters.valueParameters.indexed) {
      final name = parameter.name.toNullable();
      if (name == null) continue;

      final annotation = parameter.type;
      final HirType type;
      if (annotation != null) {
        type = lowerer.typeLowerer.lowerType(
          annotation.toNullable(),
          lowerer.typeScope,
        );
      } else if (expectedFunction != null &&
          index < expectedFunction.parameterTypes.length) {
        // The expected type says what this parameter is.
        type = expectedFunction.parameterTypes[index];
      } else {
        // Nothing to go on yet. Overload resolution of the call this lambda is
        // an argument to usually binds this by unifying the whole lambda type.
        type = variables.fresh();
        lowerer.checkInferred(name.rawText, name.span, () => type);
      }
      parameters.add(
        HirParameter(
          id: lowerer.ids.generate(),
          name: name.rawText,
          type: type,
        ),
      );
    }

    var returnType = expectedFunction?.returnType ?? variables.fresh();
    final body = buildNested((builder) {
      lowerer._locals.addAll(parameters.map((it) => (it.name, it.id, it.type)));
      final (_, type) = builder.lowerStatements(
        lambda.body.statements,
        expectedFunction?.returnType,
      );
      if (expectedFunction == null) returnType = type;
    });

    final type = HirType.function(
      parameterTypes: parameters.map((it) => it.type).toList(),
      returnType: returnType,
    );
    return _Value(
      push(HirExpressionKind.lambda(parameters: parameters, body: body), type),
      type,
    );
  }

  _Lowered _lowerSwitch(AstSwitch switch_, HirType? expectedType) {
    final value = switch_.value.toNullable();
    if (value == null) return const _LoweringError();

    final (valueId, valueType) = lowerExpression(value, null);
    final resolved = variables.resolve(valueType);
    final span = _spanOfSwitch(switch_);

    if (resolved is HirType_Error) return const _LoweringError();

    final declaration = resolved is HirType_Named
        ? lowerer.signatures.typeDeclarations[resolved.declaration]
        : null;
    final kind = declaration?.kind;
    if (kind is! HirTypeDeclarationKind_Enum) {
      _error(
        span,
        variables.isUnresolved(resolved)
            ? 'The type of this value is not known here. Add a type annotation.'
            : "Can't switch over a value of type `$resolved`.",
      );
      return const _LoweringError();
    }

    final environment = HirType.buildEnvironment(
      declaration!.typeParameters,
      (resolved as HirType_Named).typeArguments,
    );

    var returnType = expectedType;
    final seen = <String>{};
    final cases = <HirSwitchCase>[];

    for (final case_ in switch_.cases) {
      final variantToken = case_.variant.toNullable();
      if (variantToken == null) continue;
      final variantName = variantToken.rawText;

      final variant = kind.variants
          .where((it) => it.name == variantName)
          .firstOrNull;
      if (variant == null) {
        _error(variantToken.span, 'Unknown variant in switch: `$variantName`.');
        continue;
      }
      if (!seen.add(variantName)) {
        _error(
          variantToken.span,
          'Duplicate variant in switch: `$variantName`.',
        );
        continue;
      }

      final valueName = case_.valueName?.toNullable();
      if (variant.type == null && valueName != null) {
        _error(
          variantToken.span,
          'Variant `$variantName` has no value to bind.',
        );
        continue;
      }
      if (variant.type != null && valueName == null) {
        _error(
          variantToken.span,
          'Switch case for `$variantName` is missing a value name.',
        );
        continue;
      }

      HirId? bindingId;
      final body = buildNested((builder) {
        if (variant.type != null && valueName != null) {
          bindingId = lowerer.ids.generate();
          lowerer._locals.add((
            valueName.rawText,
            bindingId!,
            variant.type!.substitute(environment),
          ));
        }
        final expression = case_.expression.toNullable();
        if (expression == null) {
          builder.pushError();
          return;
        }
        final (_, type) = builder.lowerExpression(expression, returnType);
        returnType ??= type;
      });

      cases.add(
        HirSwitchCase(variant: variantName, valueId: bindingId, body: body),
      );
    }

    final missing = kind.variants
        .map((it) => it.name)
        .where((it) => !seen.contains(it))
        .toList();
    if (missing.isNotEmpty) {
      // The Rust compiler leaves this as a TODO, so a non-exhaustive switch
      // silently falls through there.
      _error(
        span,
        'Switch is missing ${missing.length == 1 ? 'variant' : 'variants'}: '
        '${missing.map((it) => '`$it`').join(', ')}.',
      );
    }

    final type = returnType ?? types.neverType;
    return _Value(
      push(
        HirExpressionKind.switch_(
          value: valueId,
          enumType: resolved,
          cases: cases,
        ),
        type,
      ),
      type,
    );
  }

  _Lowered _lowerCall(AstCall call) {
    final typeArguments = call.typeArguments?.arguments
        .map((it) => lowerer.typeLowerer.lowerType(it, lowerer.typeScope))
        .toList();
    final argumentSpan =
        call.arguments.toNullable()?.openingParenthesisSpan ??
        _spanOf(call.receiver);

    switch (call.receiver) {
      case AstExpression_Navigation(:final navigation)
          when navigation.key.toNullable() != null:
        final key = navigation.key.toNullable()!;
        final receiver = lowerRaw(navigation.receiver, null);
        switch (receiver) {
          case _Value(:final id, :final type):
            final arguments = _lowerArguments(call.arguments);
            return _resolveCall(
              name: key.rawText,
              nameSpan: key.span,
              candidates: () => lowerer.resolver.memberCandidates(
                variables.resolve(type),
                key.rawText,
                lowerer.solverContext,
              ),
              driver: type,
              arguments: [(id, type), ...arguments],
              typeArguments: typeArguments,
              isMember: true,
            );
          case _TypeReference(:final declaration):
            return _lowerEnumCreation(
              declaration,
              key,
              call,
              typeArguments,
              argumentSpan,
            );
          case _LoweringError():
            return const _LoweringError();
        }

      case AstExpression_Identifier(:final identifier):
        final name = identifier.rawText;

        for (final (localName, id, type) in lowerer._locals.reversed) {
          if (localName != name) continue;
          return _lowerLambdaCall(id, type, call, identifier.span);
        }

        final found = lowerer.scope.lookup(name);
        if (found is NamedAmbiguous) {
          _error(
            identifier.span,
            'Ambiguous name `$name`, imported from ${found.originsText}.',
          );
          return const _LoweringError();
        }
        if (found is NamedType || found is NamedTrait) {
          return _lowerStructCreation(
            (found as dynamic).id as DeclarationId,
            call,
            typeArguments,
            identifier.span,
            argumentSpan,
          );
        }

        final arguments = _lowerArguments(call.arguments);
        return _resolveCall(
          name: name,
          nameSpan: identifier.span,
          candidates: () =>
              lowerer.resolver.freeCandidates(lowerer.scope, name),
          driver: null,
          arguments: arguments,
          typeArguments: typeArguments,
          isMember: false,
        );

      default:
        _error(
          _spanOf(call.receiver),
          'Only functions, methods, and types can be called.',
        );
        return const _LoweringError();
    }
  }

  _Lowered _lowerLambdaCall(
    HirId lambdaId,
    HirType lambdaType,
    AstCall call,
    SourceSpan span,
  ) {
    final resolved = variables.resolve(lambdaType);
    if (resolved is! HirType_Function) {
      _error(span, 'A value of type `$resolved` is not callable.');
      return const _LoweringError();
    }

    final arguments = _lowerArguments(call.arguments);
    if (arguments.length != resolved.parameterTypes.length) {
      _error(
        span,
        'Expected ${resolved.parameterTypes.length} arguments, '
        'got ${arguments.length}.',
      );
      return const _LoweringError();
    }

    for (final (index, (_, type)) in arguments.indexed) {
      if (!unifier.assignable(type, resolved.parameterTypes[index])) {
        _error(
          span,
          'Expected type `${resolved.parameterTypes[index]}`, got `$type`.',
        );
        return const _LoweringError();
      }
    }

    return _Value(
      push(
        HirExpressionKind.callLambda(
          lambda: lambdaId,
          arguments: arguments.map((it) => it.$1).toList(),
        ),
        resolved.returnType,
      ),
      resolved.returnType,
    );
  }

  /// Resolves an overloaded call, deferring while [driver] is still unknown.
  _Lowered _resolveCall({
    required String name,
    required SourceSpan nameSpan,
    required List<CandidateSource> Function() candidates,
    required HirType? driver,
    required List<(HirId, HirType)> arguments,
    required List<HirType>? typeArguments,
    required bool isMember,
  }) {
    final resultType = variables.fresh();
    final entryId = lowerer.ids.generate();
    final entry = _Entry(
      id: entryId,
      name: null,
      type: resultType,
      kind: const HirExpressionKind.error(),
    );
    _entries.add(entry);

    var attempts = 0;
    void attempt() {
      attempts++;
      // A member call can't be resolved until the receiver's type is known.
      if (isMember && driver != null && variables.isUnresolved(driver)) {
        if (attempts < 2) {
          lowerer._retries.add(attempt);
          return;
        }
        _error(
          nameSpan,
          'The type of the receiver is not known here. Add a type annotation.',
        );
        return;
      }

      final sources = candidates();
      final resolution = lowerer.resolver.resolve(
        candidates: sources,
        argumentTypes: arguments.map((it) => it.$2).toList(),
        typeArguments: typeArguments,
        variables: variables,
        solverContext: lowerer.solverContext,
      );

      switch (resolution) {
        case OverloadResolved(:final candidate):
          entry.pendingCall = _PendingCall(
            candidate,
            arguments.map((it) => it.$1).toList(),
          );
          entry.type = candidate.signature.returnType;
          unifier.unify(resultType, candidate.signature.returnType);

        case OverloadNotFound(:final rejections):
          _error(nameSpan, _notFoundMessage(name, arguments, rejections));

        case OverloadAmbiguous(:final candidates):
          _error(
            nameSpan,
            'Multiple functions match this call:\n'
            '  ${_callSignature(name, arguments)}\n'
            'These are the candidates:'
            '${candidates.map((it) => '\n• ${it.signature}').join()}',
          );
      }
    }

    attempt();
    return _Value(entryId, resultType);
  }

  String _callSignature(String name, List<(HirId, HirType)> arguments) =>
      '$name(${arguments.map((it) => variables.resolve(it.$2)).join(', ')})';

  String _notFoundMessage(
    String name,
    List<(HirId, HirType)> arguments,
    List<(HirFunctionSignature, String)> rejections,
  ) {
    if (rejections.isEmpty) {
      return 'No function `$name` matches this call:\n'
          '  ${_callSignature(name, arguments)}';
    }
    final heading = rejections.length == 1
        ? 'This is the candidate'
        : 'These are the candidates';
    return 'No function matches this call:\n'
        '  ${_callSignature(name, arguments)}\n'
        '$heading:'
        '${rejections.map((it) => '\n• ${it.$1}: ${it.$2}').join()}';
  }

  _Lowered _lowerStructCreation(
    DeclarationId declaration,
    AstCall call,
    List<HirType>? typeArguments,
    SourceSpan receiverSpan,
    SourceSpan argumentSpan,
  ) {
    final type = lowerer.signatures.typeDeclarations[declaration];
    if (type == null) {
      _error(receiverSpan, '`${declaration.name}` is not a type.');
      return const _LoweringError();
    }

    final kind = type.kind;
    if (kind is HirTypeDeclarationKind_Builtin) {
      _error(
        receiverSpan,
        "Can't instantiate builtin type `${declaration.name}` directly.",
      );
      return const _LoweringError();
    }
    if (kind is HirTypeDeclarationKind_Enum) {
      _error(receiverSpan, 'Enum variant is missing.');
      return const _LoweringError();
    }

    final fields = (kind as HirTypeDeclarationKind_Struct).fields;
    for (final field in fields) {
      if (_isFieldVisible(field, declaration)) continue;

      final reason = field.visibility == .private ? 'private' : 'not visible';
      _error(
        receiverSpan,
        'Cannot construct `${declaration.name}` here: '
        'the field `${field.name}` is $reason.',
      );
      return const _LoweringError();
    }

    final substitutions = _instantiate(
      type.typeParameters,
      typeArguments,
      argumentSpan,
    );
    if (substitutions == null) return const _LoweringError();

    final arguments = _lowerArguments(call.arguments);
    if (arguments.length != fields.length) {
      _error(
        argumentSpan,
        'Expected ${fields.length} '
        '${fields.length == 1 ? 'field' : 'fields'}, got ${arguments.length}.',
      );
      return const _LoweringError();
    }

    for (final (index, field) in fields.indexed) {
      final expected = field.type.substitute(substitutions);
      if (!unifier.assignable(arguments[index].$2, expected)) {
        _error(
          argumentSpan,
          'Expected type `${variables.resolve(expected)}` for field '
          '`${field.name}`, got `${variables.resolve(arguments[index].$2)}`.',
        );
        return const _LoweringError();
      }
    }

    final structType = HirType.named(
      declaration: declaration,
      typeArguments: type.typeParameters
          .map((it) => substitutions[it.id]!)
          .toList(),
    );
    return _Value(
      push(
        HirExpressionKind.createStruct(
          fields: arguments.map((it) => it.$1).toList(),
        ),
        structType,
      ),
      structType,
    );
  }

  _Lowered _lowerEnumCreation(
    DeclarationId declaration,
    Token variantToken,
    AstCall call,
    List<HirType>? typeArguments,
    SourceSpan argumentSpan,
  ) {
    final type = lowerer.signatures.typeDeclarations[declaration];
    final kind = type?.kind;
    if (kind is! HirTypeDeclarationKind_Enum) {
      _error(
        variantToken.span,
        '`${declaration.name}` has no member `${variantToken.rawText}`.',
      );
      return const _LoweringError();
    }

    final variant = kind.variants
        .where((it) => it.name == variantToken.rawText)
        .firstOrNull;
    if (variant == null) {
      _error(
        variantToken.span,
        'Enum `${declaration.name}` has no variant '
        '`${variantToken.rawText}`.',
      );
      return const _LoweringError();
    }

    final substitutions = _instantiate(
      type!.typeParameters,
      typeArguments,
      argumentSpan,
    );
    if (substitutions == null) return const _LoweringError();

    final arguments = _lowerArguments(call.arguments);
    final expectedCount = variant.type == null ? 0 : 1;
    if (arguments.length != expectedCount) {
      final noun = expectedCount == 1 ? 'argument' : 'arguments';
      _error(
        argumentSpan,
        'Expected $expectedCount $noun, got ${arguments.length}.',
      );
      return const _LoweringError();
    }

    if (variant.type != null) {
      final expected = variant.type!.substitute(substitutions);
      if (!unifier.assignable(arguments.single.$2, expected)) {
        _error(
          argumentSpan,
          'Expected type `${variables.resolve(expected)}`, '
          'got `${variables.resolve(arguments.single.$2)}`.',
        );
        return const _LoweringError();
      }
    }

    final enumType = HirType.named(
      declaration: declaration,
      typeArguments: type.typeParameters
          .map((it) => substitutions[it.id]!)
          .toList(),
    );
    return _Value(
      push(
        HirExpressionKind.createEnum(
          variant: variant.name,
          value: arguments.isEmpty ? null : arguments.single.$1,
        ),
        enumType,
      ),
      enumType,
    );
  }

  /// Type arguments if given, fresh variables otherwise. `null` on an arity
  /// mismatch, which is already reported.
  Map<TypeParameterId, HirType>? _instantiate(
    List<HirTypeParameter> typeParameters,
    List<HirType>? typeArguments,
    SourceSpan span,
  ) {
    if (typeArguments != null &&
        typeArguments.length != typeParameters.length) {
      _error(
        span,
        'Expected ${typeParameters.length} type '
        '${typeParameters.length == 1 ? 'argument' : 'arguments'}, '
        'got ${typeArguments.length}.',
      );
      return null;
    }
    return {
      for (final (index, parameter) in typeParameters.indexed)
        parameter.id: typeArguments?[index] ?? variables.fresh(),
    };
  }

  List<(HirId, HirType)> _lowerArguments(AstErrorOr<AstArguments> arguments) =>
      [
        for (final argument in arguments.argumentsOrDefault)
          lowerExpression(argument, null),
      ];
}

extension on _Builder {
  /// The AST carries spans only on tokens, so this digs out the nearest one.
  SourceSpan _spanOf(AstExpression expression) => switch (expression) {
    AstExpression_Int(:final token) => token.span,
    AstExpression_Identifier(:final identifier) => identifier.span,
    AstExpression_Text(:final parts) =>
      parts
              .whereType<AstTextPart_Text>()
              .map((it) => it.token.span)
              .firstOrNull ??
          lowerer.fallbackSpan,
    AstExpression_Parenthesized(:final parenthesized) =>
      parenthesized.inner.toNullable() == null
          ? lowerer.fallbackSpan
          : _spanOf(parenthesized.inner.toNullable()!),
    AstExpression_Call(:final call_) => _spanOf(call_.receiver),
    AstExpression_Navigation(:final navigation) =>
      navigation.key.toNullable()?.span ?? _spanOf(navigation.receiver),
    AstExpression_Lambda(:final lambda) => _spanOfBody(lambda.body),
    AstExpression_Body(:final body) => _spanOfBody(body),
    AstExpression_Switch(:final switch_) => _spanOfSwitch(switch_),
  };

  SourceSpan _spanOfSwitch(AstSwitch switch_) {
    final value = switch_.value.toNullable();
    if (value != null) return _spanOf(value);
    final variant = switch_.cases
        .map((it) => it.variant.toNullable())
        .nonNulls
        .firstOrNull;
    return variant?.span ?? lowerer.fallbackSpan;
  }

  SourceSpan _spanOfBody(AstBody body) {
    for (final statement in body.statements) {
      switch (statement) {
        case AstStatement_Expression(:final expression):
          return _spanOf(expression);
        case AstStatement_Assignment(:final assignment):
          return assignment.displaySpan;
      }
    }
    return lowerer.fallbackSpan;
  }
}
