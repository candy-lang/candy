import '../hir/hir.dart';
import '../hir/ids.dart';

/// The functions in a vtable for [trait], in slot order.
///
/// `Any`'s vtable is only the type ID every vtable starts with: its functions
/// can't be overridden, so they run their default bodies even on an object.
List<DeclarationId> vtableSlotsOf(Hir hir, DeclarationId trait) {
  if (trait == hir.anyTrait) return const [];
  return [
    for (final MapEntry(key: id, value: function)
        in hir.traits[trait]?.functions.entries ??
            const <MapEntry<DeclarationId, HirTraitFunction>>[])
      if (whyNotDispatchable(function.signature) == null) id,
  ];
}

/// Why [signature], a function of a trait, can't be called through a trait
/// object, or `null` if it can.
///
/// A dispatchable function becomes one vtable slot, compiled once per concrete
/// type. That rules out anything whose shape depends on more than the receiver:
/// its own type parameters, a second `Self` the object can't vouch for, or a
/// `Self.Item` that differs per impl.
String? whyNotDispatchable(HirFunctionSignature signature) {
  final parameters = signature.parameters;
  if (parameters.isEmpty || !parameters.first.type.isTraitSelf) {
    return "doesn't take `self: Self` as its first parameter";
  }
  if (signature.typeParameters.isNotEmpty) {
    return 'has type parameters of its own';
  }
  if ([
    for (final parameter in parameters) parameter.type,
    signature.returnType,
  ].any(_mentionsSelfProjection)) {
    return 'uses an associated type of `Self`';
  }
  if (parameters.skip(1).any((it) => _mentionsSelf(it.type))) {
    return 'takes another `Self` besides the receiver';
  }
  // A bare `Self` result is boxed back into the object, but there's no
  // general way to do that for one nested inside another type.
  final returnType = signature.returnType;
  if (!returnType.isTraitSelf && _mentionsSelf(returnType)) {
    return 'returns `Self` nested inside another type';
  }
  return null;
}

bool _mentionsSelf(HirType type) => switch (type) {
  HirType_Self() when type.isTraitSelf => true,
  HirType_Self(:final baseType) => _mentionsSelf(baseType),
  HirType_Named(:final typeArguments) => typeArguments.any(_mentionsSelf),
  HirType_Function(:final parameterTypes, :final returnType) =>
    parameterTypes.any(_mentionsSelf) || _mentionsSelf(returnType),
  HirType_Associated(:final trait, :final baseType) =>
    trait.typeArguments.any(_mentionsSelf) || _mentionsSelf(baseType),
  HirType_Dyn(:final trait) => trait.typeArguments.any(_mentionsSelf),
  HirType_Parameter() || HirType_Error() || HirType_Variable() => false,
};

bool _mentionsSelfProjection(HirType type) => switch (type) {
  HirType_Associated(:final baseType) when _mentionsSelf(baseType) => true,
  HirType_Associated(:final trait, :final baseType) =>
    trait.typeArguments.any(_mentionsSelfProjection) ||
        _mentionsSelfProjection(baseType),
  HirType_Self(:final baseType) => _mentionsSelfProjection(baseType),
  HirType_Named(:final typeArguments) => typeArguments.any(
    _mentionsSelfProjection,
  ),
  HirType_Function(:final parameterTypes, :final returnType) =>
    parameterTypes.any(_mentionsSelfProjection) ||
        _mentionsSelfProjection(returnType),
  HirType_Dyn(:final trait) => trait.typeArguments.any(_mentionsSelfProjection),
  HirType_Parameter() || HirType_Error() || HirType_Variable() => false,
};
