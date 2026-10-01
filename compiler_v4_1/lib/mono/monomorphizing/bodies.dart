import '../../hir/hir/hir.dart';
import '../../hir/hir/ids.dart';
import '../hir_to_mono.dart';
import '../mono/ids.dart';
import '../mono/mono.dart';
import 'types.dart';

/// Lowers one body with the type arguments of the instantiation it belongs to.
///
/// Expression IDs carry over unchanged: HIR lowering draws them from a single
/// generator, so they're already unique within the function, nested lambdas
/// and switch bindings included.
class BodyMonomorphizer {
  BodyMonomorphizer(this.monomorphizer, this.environment);

  final Monomorphizer monomorphizer;
  final Map<TypeParameterId, HirType> environment;

  TypeMonomorphizer get _types => monomorphizer.types;

  MonoBody lower(HirBody body) =>
      MonoBody([for (final entry in body.expressions) _lowerEntry(entry)]);

  MonoBodyEntry _lowerEntry(HirBodyEntry entry) => MonoBodyEntry(
    id: entry.id,
    name: entry.name,
    expression: MonoExpression(
      kind: switch (entry.expression.kind) {
        // The only kind that needs the expression's type besides the kind.
        HirExpressionKind_ToDyn(:final value, :final sourceType) => _lowerToDyn(
          value,
          sourceType,
          entry.expression.type,
        ),
        final kind => _lowerKind(kind),
      },
      type: _type(entry.expression.type),
    ),
  );

  MonoType _type(HirType type) => _types.monomorphize(type, environment);

  MonoExpressionKind _lowerKind(HirExpressionKind kind) => switch (kind) {
    HirExpressionKind_Int(:final value) => .int(value),
    HirExpressionKind_Text(:final value) => .text(value),
    HirExpressionKind_CreateStruct(:final fields) => .createStruct(
      fields: fields,
    ),
    HirExpressionKind_StructAccess(:final struct, :final field) =>
      .structAccess(struct: struct, field: field),
    HirExpressionKind_CreateEnum(:final variant, :final value) => .createEnum(
      variant: variant,
      value: value,
    ),
    HirExpressionKind_Reference(:final id) => .localReference(id),
    HirExpressionKind_GlobalReference(:final id) => .globalAssignmentReference(
      monomorphizer.lowerAssignment(id),
    ),
    HirExpressionKind_Call(
      :final function,
      :final substitutions,
      :final arguments,
    ) =>
      .callFunction(
        function: monomorphizer.lowerFunction(function, _merge(substitutions)),
        arguments: arguments,
      ),
    HirExpressionKind_CallLambda(:final lambda, :final arguments) =>
      .callLambda(lambda: lambda, arguments: arguments),
    HirExpressionKind_Switch(:final value, :final enumType, :final cases) =>
      _lowerSwitch(value, enumType, cases),
    HirExpressionKind_Lambda(:final parameters, :final body) => .lambda(
      parameters: [
        for (final parameter in parameters)
          MonoParameter(
            id: parameter.id,
            name: parameter.name,
            type: _type(parameter.type),
          ),
      ],
      body: lower(body),
    ),
    HirExpressionKind_ToDyn() => throw StateError(
      'Boxing needs its target type, which `_lowerEntry` passes on.',
    ),
    HirExpressionKind_Error() => throw StateError(
      'The HIR still contains errors.',
    ),
  };

  /// Substitution can turn a coercion into a no-op, e.g. a `T: Widget` that
  /// turns out to be `Widget` itself.
  MonoExpressionKind _lowerToDyn(
    HirId value,
    HirType sourceType,
    HirType targetType,
  ) {
    final source = _concrete(sourceType);
    final target = _concrete(targetType);
    if (target is! HirType_Dyn) {
      throw StateError('Boxing into `$target`, which is not a trait object.');
    }
    if (source == target) return .localReference(value);
    // Only an upcast to `Any` gets here with an object as the source.
    if (source is HirType_Dyn) return .toDyn(value: value, vtable: null);
    return .toDyn(
      value: value,
      vtable: monomorphizer.vtableFor(source, target),
    );
  }

  HirType _concrete(HirType type) =>
      _types.normalizer.normalize(type.substitute(environment)).unwrapSelf;

  MonoExpressionKind _lowerSwitch(
    HirId value,
    HirType enumType,
    List<HirSwitchCase> cases,
  ) {
    final type = _type(enumType);
    final declaration = _types.declarationOf(type);
    if (declaration is! MonoTypeDeclaration_Enum) {
      throw StateError('Switching over `$type`, which is not an enum.');
    }

    MonoType payloadTypeOf(String variant) {
      final match = declaration.variants
          .where((it) => it.name == variant)
          .firstOrNull;
      if (match?.type == null) {
        throw StateError("`$type.$variant` doesn't carry a value.");
      }
      return match!.type!;
    }

    return MonoExpressionKind.switch_(
      value: value,
      enumType: type,
      cases: [
        for (final case_ in cases)
          MonoSwitchCase(
            variant: case_.variant,
            value: switch (case_.valueId) {
              null => null,
              final valueId => (valueId, payloadTypeOf(case_.variant)),
            },
            body: lower(case_.body),
          ),
      ],
    );
  }

  /// Resolves the call's type arguments in terms of this instantiation, so a
  /// generic function calling another one passes on concrete types.
  Map<TypeParameterId, HirType> _merge(
    Map<TypeParameterId, HirType> substitutions,
  ) => {
    for (final MapEntry(:key, :value) in substitutions.entries)
      key: value.substitute(environment),
  };
}
