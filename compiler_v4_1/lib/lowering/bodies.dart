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

/// A parameter or binding visible to the code being lowered.
class _Local {
  const _Local({
    required this.name,
    required this.id,
    required this.type,
    required this.span,
  });

  factory _Local.ofParameter(HirParameter parameter) => _Local(
    name: parameter.name,
    id: parameter.id,
    type: parameter.type,
    span: parameter.span,
  );

  final String name;
  final HirId id;
  final HirType type;

  /// Where the name is declared, or `null` for a builtin's parameter.
  final SourceSpan? span;
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
      // Signatures are complete by now, so this can be decided right away.
      checkTraitObject: (trait, span) =>
          checkTraitObject(context, signatures, trait, span),
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

  final _locals = <_Local>[];
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

  void _reference(SourceSpan span, Iterable<SourceSpan?> targets) =>
      context.addReference(span, targets);

  void _declare(SourceSpan span) => context.addDeclaration(span);

  /// Runs [check] against a copy of the type variables first, and against the
  /// real ones only if that succeeds, so a failed attempt binds nothing.
  bool tryUnify(bool Function(Unifier unifier) check) {
    final scratch = Unifier(
      variables: variables.speculate(),
      wellKnownTypes: types,
      normalize: signatures.normalizer.normalize,
    );
    return check(scratch) && check(unifier);
  }

  bool canCoerce(HirType from, HirType to) =>
      resolver.canCoerce(from, to, variables, solverContext);

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
      ..addAll(parameters.map(_Local.ofParameter));

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

  void _reference(SourceSpan span, Iterable<SourceSpan?> targets) =>
      lowerer._reference(span, targets);

  void _declare(SourceSpan span) => lowerer._declare(span);

  CollectedDeclarations get declarations => lowerer.declarations;

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
    if (pending == null) {
      return switch (entry.kind) {
        HirExpressionKind_ToDyn(:final value, :final sourceType) =>
          HirExpressionKind.toDyn(
            value: value,
            sourceType: variables.finalize(sourceType),
          ),
        final kind => kind,
      };
    }
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

  HirId push(
    HirExpressionKind kind,
    HirType type, {
    String? name,
    SourceSpan? nameSpan,
  }) {
    final id = lowerer.ids.generate();
    if (name != null) {
      if (nameSpan != null) _declare(nameSpan);
      lowerer._locals.add(
        _Local(name: name, id: id, type: type, span: nameSpan),
      );
    }
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
              nameSpan: name.span,
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
        if (expectedType == null || unifier.assignable(type, expectedType)) {
          return (id, type);
        }
        if (_coerce(id, type, expectedType) case final coerced?) {
          return (coerced, expectedType);
        }
        _error(
          span,
          'Expected type `${variables.resolve(expectedType)}`, '
          'got `${variables.resolve(type)}`.',
        );
        return (pushError(), const HirType.error());
      case _TypeReference():
        _error(span, 'A type must be instantiated to be used as a value.');
        return (pushError(), const HirType.error());
      case _LoweringError():
        return (pushError(), const HirType.error());
    }
  }

  /// Boxes [id] into the trait object [to], or returns `null` if [from] can't
  /// be used as one.
  HirId? _coerce(HirId id, HirType from, HirType to) {
    if (!lowerer.canCoerce(from, to)) return null;
    return push(HirExpressionKind.toDyn(value: id, sourceType: from), to);
  }

  /// Lowers a call argument against the type the callee expects, boxing it
  /// into a trait object if that's what makes it fit. A mismatch is left for
  /// overload resolution to report, since it can describe the whole call.
  (HirId, HirType) _lowerArgument(
    AstExpression argument,
    HirType? expectedType,
  ) {
    switch (lowerRaw(argument, expectedType)) {
      case _Value(:final id, :final type):
        if (expectedType == null ||
            lowerer.tryUnify((it) => it.assignable(type, expectedType))) {
          return (id, type);
        }
        if (_coerce(id, type, expectedType) case final coerced?) {
          return (coerced, expectedType);
        }
        return (id, type);
      case _TypeReference():
        _error(
          _spanOf(argument),
          'A type must be instantiated to be used as a value.',
        );
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
        return _lowerCall(call_, expectedType);

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

    for (final local in lowerer._locals.reversed) {
      if (local.name == name) {
        _reference(identifier.span, [local.span]);
        return _Value(
          push(HirExpressionKind.reference(local.id), local.type),
          local.type,
        );
      }
    }

    if (name == 'Self') {
      final declaration = _selfDeclaration(identifier.span);
      return declaration == null
          ? const _LoweringError()
          : _TypeReference(declaration);
    }

    switch (lowerer.scope.lookup(name)) {
      case NamedAssignment(:final id):
        _reference(identifier.span, [declarations.spanOf(id)]);
        final type = lowerer.globalTypes[id] ?? const HirType.error();
        return _Value(push(HirExpressionKind.globalReference(id), type), type);
      case NamedType(:final id) || NamedTrait(:final id):
        _reference(identifier.span, [declarations.spanOf(id)]);
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

  DeclarationId? _selfDeclaration(SourceSpan span) {
    final baseType = lowerer.typeScope.selfBaseType;
    if (baseType == null) {
      _error(span, '`Self` can only be used in traits and impls.');
      return null;
    }
    if (baseType is! HirType_Named) {
      _error(span, '`Self` is not a named type here.');
      return null;
    }

    _reference(span, [declarations.spanOf(baseType.declaration)]);
    return baseType.declaration;
  }

  _Lowered _lowerNavigation(AstNavigation navigation) {
    final key = navigation.key.toNullable();
    if (key == null) return const _LoweringError();

    final receiver = lowerRaw(navigation.receiver, null);
    switch (receiver) {
      case _Value(:final id, :final type):
        return _lowerFieldAccess(id, type, key);
      case _TypeReference(:final declaration):
        if (_isVariant(declaration, key.rawText)) {
          _error(
            key.span,
            'Enum variant `${declaration.name}.${key.rawText}` must be called '
            'to create a value.',
          );
        } else if (_staticCandidates(declaration, key.rawText).isNotEmpty) {
          _error(
            key.span,
            'Referencing a function as a value is not supported yet.',
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

  List<CandidateSource> _staticCandidates(
    DeclarationId declaration,
    String name,
  ) => lowerer.resolver.staticCandidates(declaration, name, variables);

  bool _isVariant(DeclarationId declaration, String name) {
    final kind = lowerer.signatures.typeDeclarations[declaration]?.kind;
    return kind is HirTypeDeclarationKind_Enum &&
        kind.variants.any((it) => it.name == name);
  }

  _Lowered _lowerFieldAccess(HirId receiverId, HirType receiver, Token key) {
    final resolved = variables.resolve(receiver);
    if (resolved is HirType_Error) return const _LoweringError();
    final unwrapped = resolved.unwrapSelf;
    if (unwrapped is! HirType_Named) {
      _error(
        key.span,
        variables.isUnresolved(resolved)
            ? 'The type of this value is not known here. Add a type annotation.'
            : 'A value of type `$resolved` has no field `${key.rawText}`.',
      );
      return const _LoweringError();
    }

    final declaration =
        lowerer.signatures.typeDeclarations[unwrapped.declaration];
    final kind = declaration?.kind;
    if (kind is HirTypeDeclarationKind_Struct) {
      final field = kind.fields
          .where((it) => it.name == key.rawText)
          .firstOrNull;
      if (field != null) {
        _reference(key.span, [
          declarations.fieldSpanOf(unwrapped.declaration, key.rawText),
        ]);
        if (!_isFieldVisible(field, unwrapped.declaration)) {
          _error(key.span, _fieldVisibilityMessage(field, key.rawText));
          return const _LoweringError();
        }

        final type = field.type.substitute(
          HirType.buildEnvironment(
            declaration!.typeParameters,
            unwrapped.typeArguments,
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
        // The expected type says what this parameter is, though maybe only as
        // a variable the enclosing call has yet to pin down.
        type = expectedFunction.parameterTypes[index];
        if (variables.isUnresolved(type)) {
          lowerer.checkInferred(name.rawText, name.span, () => type);
        }
      } else {
        // Nothing to go on yet. Overload resolution of the call this lambda is
        // an argument to usually binds this by unifying the whole lambda type.
        type = variables.fresh();
        lowerer.checkInferred(name.rawText, name.span, () => type);
      }
      _declare(name.span);
      parameters.add(
        HirParameter(
          id: lowerer.ids.generate(),
          name: name.rawText,
          type: type,
          span: name.span,
        ),
      );
    }

    var returnType = expectedFunction?.returnType ?? variables.fresh();
    final body = buildNested((builder) {
      lowerer._locals.addAll(parameters.map(_Local.ofParameter));
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

    final unwrapped = resolved.unwrapSelf;
    final declaration = unwrapped is HirType_Named
        ? lowerer.signatures.typeDeclarations[unwrapped.declaration]
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
      (unwrapped as HirType_Named).typeArguments,
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
      _reference(variantToken.span, [
        declarations.variantSpanOf(declaration.id, variantName),
      ]);
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
          _declare(valueName.span);
          lowerer._locals.add(
            _Local(
              name: valueName.rawText,
              id: bindingId!,
              type: variant.type!.substitute(environment),
              span: valueName.span,
            ),
          );
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

  _Lowered _lowerCall(AstCall call, HirType? expectedType) {
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
            return _resolveCall(
              name: key.rawText,
              nameSpan: key.span,
              candidates: () => lowerer.resolver.memberCandidates(
                variables.resolve(type),
                key.rawText,
                lowerer.solverContext,
              ),
              receiver: (id, type),
              arguments: call.arguments,
              typeArguments: typeArguments,
              expectedType: expectedType,
            );
          case _TypeReference(:final declaration):
            if (_isVariant(declaration, key.rawText) ||
                _staticCandidates(declaration, key.rawText).isEmpty) {
              return _lowerEnumCreation(
                declaration,
                key,
                call,
                typeArguments,
                argumentSpan,
                expectedType,
              );
            }

            return _resolveCall(
              name: key.rawText,
              nameSpan: key.span,
              candidates: () => _staticCandidates(declaration, key.rawText),
              receiver: null,
              arguments: call.arguments,
              typeArguments: typeArguments,
              expectedType: expectedType,
            );
          case _LoweringError():
            return const _LoweringError();
        }

      case AstExpression_Identifier(:final identifier):
        final name = identifier.rawText;

        for (final local in lowerer._locals.reversed) {
          if (local.name != name) continue;
          _reference(identifier.span, [local.span]);
          return _lowerLambdaCall(local.id, local.type, call, identifier.span);
        }

        if (name == 'Self') {
          final declaration = _selfDeclaration(identifier.span);
          if (declaration == null) return const _LoweringError();
          return _lowerStructCreation(
            declaration,
            call,
            typeArguments,
            identifier.span,
            argumentSpan,
            expectedType,
          );
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
          _reference(identifier.span, [
            declarations.spanOf((found as dynamic).id as DeclarationId),
          ]);
          return _lowerStructCreation(
            (found as dynamic).id as DeclarationId,
            call,
            typeArguments,
            identifier.span,
            argumentSpan,
            expectedType,
          );
        }

        return _resolveCall(
          name: name,
          nameSpan: identifier.span,
          candidates: () =>
              lowerer.resolver.freeCandidates(lowerer.scope, name),
          receiver: null,
          arguments: call.arguments,
          typeArguments: typeArguments,
          expectedType: expectedType,
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

    final parameterTypes = resolved.parameterTypes;
    final arguments = _lowerArgumentsAgainst(call.arguments, parameterTypes);
    if (arguments.length != parameterTypes.length) {
      _error(
        span,
        'Expected ${parameterTypes.length} arguments, '
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

  /// Resolves an overloaded call, deferring while the [receiver]'s type is
  /// still unknown.
  _Lowered _resolveCall({
    required String name,
    required SourceSpan nameSpan,
    required List<CandidateSource> Function() candidates,
    required (HirId, HirType)? receiver,
    required AstErrorOr<AstArguments> arguments,
    required List<HirType>? typeArguments,
    required HirType? expectedType,
  }) {
    final argumentAsts = arguments.argumentsOrDefault;
    final expectedArgumentTypes = _expectedArgumentTypes(
      candidates: candidates,
      receiver: receiver,
      argumentCount: argumentAsts.length,
      typeArguments: typeArguments,
      expectedType: expectedType,
    );
    final loweredArguments = [
      ?receiver,
      for (final (index, argument) in argumentAsts.indexed)
        _lowerArgument(argument, expectedArgumentTypes?[index]),
    ];
    final driver = receiver?.$2;

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
      if (driver != null && variables.isUnresolved(driver)) {
        if (attempts < 2) {
          lowerer._retries.add(attempt);
          return;
        }
        _referenceCandidates(nameSpan, candidates());
        _error(
          nameSpan,
          'The type of the receiver is not known here. Add a type annotation.',
        );
        return;
      }

      final sources = candidates();
      final resolution = lowerer.resolver.resolve(
        candidates: sources,
        argumentTypes: loweredArguments.map((it) => it.$2).toList(),
        typeArguments: typeArguments,
        variables: variables,
        solverContext: lowerer.solverContext,
      );

      switch (resolution) {
        case OverloadResolved(:final candidate):
          _reference(nameSpan, [declarations.spanOf(candidate.id)]);
          final invisible = _methodVisibilityMessage(candidate.id, name);
          if (invisible != null) {
            _error(nameSpan, invisible);
            return;
          }

          // Only an overloaded call gets here with arguments that still need
          // boxing; a single candidate's are boxed as they're lowered.
          entry.pendingCall = _PendingCall(candidate, [
            for (final (index, (id, type)) in loweredArguments.indexed)
              _coerceBefore(
                    entry,
                    id,
                    type,
                    candidate.signature.parameters[index].type,
                  ) ??
                  id,
          ]);
          entry.type = candidate.signature.returnType;
          unifier.unify(resultType, candidate.signature.returnType);

        case OverloadNotFound(:final rejections):
          _referenceCandidates(nameSpan, sources);
          _error(
            nameSpan,
            _notFoundMessage(name, loweredArguments, rejections),
          );

        case OverloadAmbiguous(:final candidates):
          _referenceCandidates(nameSpan, candidates);
          _error(
            nameSpan,
            'Multiple functions match this call:\n'
            '  ${_callSignature(name, loweredArguments)}\n'
            'These are the candidates:'
            '${candidates.map((it) => '\n• ${it.signature}').join()}',
          );
      }
    }

    attempt();
    return _Value(entryId, resultType);
  }

  /// The parameter types of the only function a call can resolve to, so its
  /// arguments are lowered knowing what they should be: a lambda learns its
  /// parameter types, and a value can be boxed into the trait object the
  /// function takes. The call's own expected type flows in first, which is
  /// what lets `List.of(label, counter)` build a `List[Widget]`.
  ///
  /// `null` while several candidates are in the running, since binding one's
  /// types could contradict the one that wins.
  List<HirType>? _expectedArgumentTypes({
    required List<CandidateSource> Function() candidates,
    required (HirId, HirType)? receiver,
    required int argumentCount,
    required List<HirType>? typeArguments,
    required HirType? expectedType,
  }) {
    if (receiver != null && variables.isUnresolved(receiver.$2)) return null;

    final receiverCount = receiver == null ? 0 : 1;
    final matching = candidates()
        .where(
          (it) =>
              it.signature.parameters.length == receiverCount + argumentCount &&
              (typeArguments == null ||
                  it.signature.typeParameters.length == typeArguments.length),
        )
        .toList();
    if (matching.length != 1) return null;

    final candidate = matching.single.instantiate(variables);
    if (typeArguments != null) {
      for (final (index, parameter)
          in matching.single.signature.typeParameters.indexed) {
        unifier.unify(
          candidate.substitutions[parameter.id]!,
          typeArguments[index],
        );
      }
    }
    if (expectedType != null) {
      lowerer.tryUnify(
        (it) => it.unify(candidate.signature.returnType, expectedType),
      );
    }
    return candidate.signature.parameters
        .skip(receiverCount)
        .map((it) => it.type)
        .toList();
  }

  /// Boxes [id] into the trait object [to] just before [anchor], for a call
  /// whose callee was picked after its arguments were lowered. `null` if it
  /// needs no boxing.
  HirId? _coerceBefore(_Entry anchor, HirId id, HirType from, HirType to) {
    if (!lowerer.canCoerce(from, to)) return null;

    final coerced = _Entry(
      id: lowerer.ids.generate(),
      name: null,
      type: to,
      kind: HirExpressionKind.toDyn(value: id, sourceType: from),
    );
    _entries.insert(_entries.indexOf(anchor), coerced);
    return coerced.id;
  }

  /// A call that inference couldn't narrow down still points somewhere useful:
  /// every function that was in the running.
  void _referenceCandidates(SourceSpan span, List<CandidateSource> sources) =>
      _reference(
        span,
        sources.map((it) => declarations.spanOf(it.signature.id)),
      );

  /// Inherent methods carry their own visibility; trait members never do and
  /// are always callable wherever their receiver is.
  String? _methodVisibilityMessage(DeclarationId id, String name) {
    final impl = lowerer.signatures.implOwning(id);
    if (impl == null || !impl.isInherent) return null;

    final visibility =
        lowerer.signatures.functions[id]?.ast?.visibility ?? .private;
    final isVisible = switch (visibility) {
      .private => id.module == lowerer.module,
      .package => id.module.packageName == lowerer.module.packageName,
      .public => true,
    };
    if (isVisible) return null;

    return visibility == .private
        ? 'The function `$name` is private.'
        : 'The function `$name` is not visible here.';
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
    HirType? expectedType,
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

    final structType = HirType.named(
      declaration: declaration,
      typeArguments: type.typeParameters
          .map((it) => substitutions[it.id]!)
          .toList(),
    );
    _expect(structType, expectedType);

    final fieldTypes = [
      for (final field in fields) field.type.substitute(substitutions),
    ];
    final arguments = _lowerArgumentsAgainst(call.arguments, fieldTypes);
    if (arguments.length != fields.length) {
      _error(
        argumentSpan,
        'Expected ${fields.length} '
        '${fields.length == 1 ? 'field' : 'fields'}, got ${arguments.length}.',
      );
      return const _LoweringError();
    }

    for (final (index, field) in fields.indexed) {
      final expected = fieldTypes[index];
      if (!unifier.assignable(arguments[index].$2, expected)) {
        _error(
          argumentSpan,
          'Expected type `${variables.resolve(expected)}` for field '
          '`${field.name}`, got `${variables.resolve(arguments[index].$2)}`.',
        );
        return const _LoweringError();
      }
    }

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
    HirType? expectedType,
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
    _reference(variantToken.span, [
      declarations.variantSpanOf(declaration, variant.name),
    ]);

    final substitutions = _instantiate(
      type!.typeParameters,
      typeArguments,
      argumentSpan,
    );
    if (substitutions == null) return const _LoweringError();

    final enumType = HirType.named(
      declaration: declaration,
      typeArguments: type.typeParameters
          .map((it) => substitutions[it.id]!)
          .toList(),
    );
    _expect(enumType, expectedType);

    final arguments = _lowerArgumentsAgainst(call.arguments, [
      if (variant.type case final payload?) payload.substitute(substitutions),
    ]);
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

  /// Lowers each argument against its parameter type, or against nothing if
  /// the counts don't match, which the caller reports.
  List<(HirId, HirType)> _lowerArgumentsAgainst(
    AstErrorOr<AstArguments> arguments,
    List<HirType> parameterTypes,
  ) {
    final asts = arguments.argumentsOrDefault;
    final matches = asts.length == parameterTypes.length;
    return [
      for (final (index, argument) in asts.indexed)
        _lowerArgument(argument, matches ? parameterTypes[index] : null),
    ];
  }

  /// Lets a value's expected type pin down its type arguments before its
  /// fields are lowered, so `Maybe.some(label)` builds a `Maybe[Widget]` where
  /// one is expected. Ignored if it doesn't fit, since the value may still be
  /// boxed into a trait object afterwards.
  void _expect(HirType type, HirType? expectedType) {
    if (expectedType == null) return;
    lowerer.tryUnify((it) => it.unify(type, expectedType));
  }
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
