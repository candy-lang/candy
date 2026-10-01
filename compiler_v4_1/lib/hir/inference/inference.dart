import '../../position.dart';
import '../hir/builtins.dart';
import '../hir/hir.dart';
import '../hir/ids.dart';

/// Bindings for the type variables introduced while inferring one body.
class TypeVariables {
  TypeVariables();
  TypeVariables._(this._bindings, this._next);

  Map<TypeVariableId, HirType> _bindings = {};
  var _next = 0;

  HirType fresh() => HirType.variable(TypeVariableId(_next++));

  /// A copy that sees the current bindings but keeps new ones to itself, for
  /// trying something out. Fresh variables it hands out can't collide with
  /// this store's, so the attempt can be replayed here if it works out.
  TypeVariables speculate() => TypeVariables._({..._bindings}, _next);

  /// Follows bindings at the head only.
  HirType shallow(HirType type) {
    var current = type;
    while (current is HirType_Variable) {
      final bound = _bindings[current.id];
      if (bound == null) return current;
      current = bound;
    }
    return current;
  }

  /// Follows bindings everywhere, so the result is variable-free wherever it
  /// could be.
  HirType resolve(HirType type) {
    final head = shallow(type);
    return switch (head) {
      HirType_Named(:final declaration, :final typeArguments) => HirType.named(
        declaration: declaration,
        typeArguments: typeArguments.map(resolve).toList(),
      ),
      HirType_Function(:final parameterTypes, :final returnType) =>
        HirType.function(
          parameterTypes: parameterTypes.map(resolve).toList(),
          returnType: resolve(returnType),
        ),
      HirType_Self(:final baseType) => HirType.self(
        baseType: resolve(baseType),
      ),
      HirType_Associated(:final trait, :final baseType, :final declaration) =>
        HirType.associated(
          trait: trait.copyWith(
            typeArguments: trait.typeArguments.map(resolve).toList(),
          ),
          baseType: resolve(baseType),
          declaration: declaration,
        ),
      HirType_Dyn(:final trait) => HirType.dyn(
        trait.copyWith(
          typeArguments: trait.typeArguments.map(resolve).toList(),
        ),
      ),
      _ => head,
    };
  }

  /// Replaces variables that are still unbound with [HirType.error].
  HirType finalize(HirType type) {
    final resolved = resolve(type);
    return switch (resolved) {
      HirType_Variable() => const HirType.error(),
      HirType_Named(:final declaration, :final typeArguments) => HirType.named(
        declaration: declaration,
        typeArguments: typeArguments.map(finalize).toList(),
      ),
      HirType_Function(:final parameterTypes, :final returnType) =>
        HirType.function(
          parameterTypes: parameterTypes.map(finalize).toList(),
          returnType: finalize(returnType),
        ),
      HirType_Self(:final baseType) => HirType.self(
        baseType: finalize(baseType),
      ),
      HirType_Associated(:final trait, :final baseType, :final declaration) =>
        HirType.associated(
          trait: trait.copyWith(
            typeArguments: trait.typeArguments.map(finalize).toList(),
          ),
          baseType: finalize(baseType),
          declaration: declaration,
        ),
      HirType_Dyn(:final trait) => HirType.dyn(
        trait.copyWith(
          typeArguments: trait.typeArguments.map(finalize).toList(),
        ),
      ),
      _ => resolved,
    };
  }

  bool isUnresolved(HirType type) => resolve(type).containsVariable;

  bool bind(TypeVariableId id, HirType type) {
    final target = shallow(type);
    if (target is HirType_Variable && target.id == id) return true;
    // Occurs check: binding `?0 := List[?0]` would build an infinite type.
    if (_occurs(id, target)) return false;
    _bindings[id] = target;
    return true;
  }

  bool _occurs(TypeVariableId id, HirType type) {
    final head = shallow(type);
    return switch (head) {
      HirType_Variable(id: final other) => other == id,
      HirType_Named(:final typeArguments) => typeArguments.any(
        (it) => _occurs(id, it),
      ),
      HirType_Function(:final parameterTypes, :final returnType) =>
        parameterTypes.any((it) => _occurs(id, it)) || _occurs(id, returnType),
      HirType_Self(:final baseType) => _occurs(id, baseType),
      HirType_Associated(:final trait, :final baseType) =>
        trait.typeArguments.any((it) => _occurs(id, it)) ||
            _occurs(id, baseType),
      HirType_Dyn(:final trait) => trait.typeArguments.any(
        (it) => _occurs(id, it),
      ),
      _ => false,
    };
  }
}

extension on HirType {
  bool get containsVariable => switch (this) {
    HirType_Variable() => true,
    HirType_Named(:final typeArguments) => typeArguments.any(
      (it) => it.containsVariable,
    ),
    HirType_Function(:final parameterTypes, :final returnType) =>
      parameterTypes.any((it) => it.containsVariable) ||
          returnType.containsVariable,
    HirType_Self(:final baseType) => baseType.containsVariable,
    HirType_Associated(:final trait, :final baseType) =>
      trait.typeArguments.any((it) => it.containsVariable) ||
          baseType.containsVariable,
    HirType_Dyn(:final trait) => trait.typeArguments.any(
      (it) => it.containsVariable,
    ),
    _ => false,
  };
}

/// Unifies and checks assignability against a set of [TypeVariables].
///
/// Candy has no subtyping between named types, so assignability is
/// unification plus escapes for `Never`, error types, and `Self`. Coercing a
/// value into a trait object needs the trait solver, so body lowering does that
/// on top.
class Unifier {
  Unifier({
    required this.variables,
    required this.wellKnownTypes,
    this.normalize,
  });

  final TypeVariables variables;
  final WellKnownTypes wellKnownTypes;

  /// Resolves projections before they're compared, so `Self.Item` and the
  /// `Int` an impl binds it to are the same type. `null` inside the normalizer
  /// itself, which would otherwise call back into itself.
  final HirType Function(HirType)? normalize;

  HirType _head(HirType type) {
    final resolved = variables.shallow(type);
    if (resolved is! HirType_Associated) return resolved;

    final normalized = normalize?.call(resolved) ?? resolved;
    return normalized == resolved ? resolved : variables.shallow(normalized);
  }

  bool unify(HirType a, HirType b) {
    final left = _head(a);
    final right = _head(b);

    if (left is HirType_Error || right is HirType_Error) return true;
    if (left is HirType_Variable) return variables.bind(left.id, right);
    if (right is HirType_Variable) return variables.bind(right.id, left);

    switch ((left, right)) {
      case (HirType_Named(), HirType_Named()):
        final l = left as HirType_Named;
        final r = right as HirType_Named;
        if (l.declaration != r.declaration) return false;
        if (l.typeArguments.length != r.typeArguments.length) return false;
        for (var i = 0; i < l.typeArguments.length; i++) {
          if (!unify(l.typeArguments[i], r.typeArguments[i])) return false;
        }
        return true;

      case (HirType_Function(), HirType_Function()):
        final l = left as HirType_Function;
        final r = right as HirType_Function;
        if (l.parameterTypes.length != r.parameterTypes.length) return false;
        for (var i = 0; i < l.parameterTypes.length; i++) {
          if (!unify(l.parameterTypes[i], r.parameterTypes[i])) return false;
        }
        return unify(l.returnType, r.returnType);

      case (HirType_Parameter(), HirType_Parameter()):
        return (left as HirType_Parameter).id ==
            (right as HirType_Parameter).id;

      // A projection that survived normalization is rigid: it stands for one
      // unknown type, so it only ever equals the same projection.
      case (HirType_Associated(), HirType_Associated()):
        final l = left as HirType_Associated;
        final r = right as HirType_Associated;
        if (l.declaration != r.declaration) return false;
        if (l.trait.typeArguments.length != r.trait.typeArguments.length) {
          return false;
        }
        for (var i = 0; i < l.trait.typeArguments.length; i++) {
          if (!unify(l.trait.typeArguments[i], r.trait.typeArguments[i])) {
            return false;
          }
        }
        return unify(l.baseType, r.baseType);

      case (HirType_Dyn(), HirType_Dyn()):
        final l = (left as HirType_Dyn).trait;
        final r = (right as HirType_Dyn).trait;
        if (l.declaration != r.declaration) return false;
        if (l.typeArguments.length != r.typeArguments.length) return false;
        for (var i = 0; i < l.typeArguments.length; i++) {
          if (!unify(l.typeArguments[i], r.typeArguments[i])) return false;
        }
        return true;

      // `Self` unifies with whatever it stands for.
      case (HirType_Self(:final baseType), _):
        return unify(baseType, right);
      case (_, HirType_Self(:final baseType)):
        return unify(left, baseType);

      default:
        return false;
    }
  }

  /// Whether a value of type [sub] can be used where [sup] is expected.
  bool assignable(HirType sub, HirType sup) {
    final from = _head(sub);
    final to = _head(sup);

    if (from is HirType_Error || to is HirType_Error) return true;
    // `Never` has no values, so it stands in for anything. Allowed at every
    // level, matching the Rust compiler's `equals_lenient`.
    if (wellKnownTypes.isNever(from)) return true;
    if (from is HirType_Variable || to is HirType_Variable) {
      return unify(from, to);
    }

    switch ((from, to)) {
      case (HirType_Named(), HirType_Named()):
        final l = from as HirType_Named;
        final r = to as HirType_Named;
        if (l.declaration != r.declaration) return false;
        if (l.typeArguments.length != r.typeArguments.length) return false;
        for (var i = 0; i < l.typeArguments.length; i++) {
          if (!assignable(l.typeArguments[i], r.typeArguments[i])) return false;
        }
        return true;

      case (HirType_Function(), HirType_Function()):
        final l = from as HirType_Function;
        final r = to as HirType_Function;
        if (l.parameterTypes.length != r.parameterTypes.length) return false;
        for (var i = 0; i < l.parameterTypes.length; i++) {
          if (!assignable(r.parameterTypes[i], l.parameterTypes[i])) {
            return false;
          }
        }
        return assignable(l.returnType, r.returnType);

      default:
        return unify(from, to);
    }
  }
}

/// A constraint that can't be decided until more types are known.
sealed class DeferredConstraint {
  const DeferredConstraint();

  SourceSpan get span;

  /// The type whose head has to be known before this can be decided.
  HirType get driver;
}

class HasFieldConstraint extends DeferredConstraint {
  const HasFieldConstraint({
    required this.receiver,
    required this.field,
    required this.result,
    required this.span,
  });

  final HirType receiver;
  final String field;
  final HirType result;
  @override
  final SourceSpan span;
  @override
  HirType get driver => receiver;
}

class HasVariantConstraint extends DeferredConstraint {
  const HasVariantConstraint({
    required this.enumType,
    required this.variant,
    required this.payload,
    required this.span,
  });

  final HirType enumType;
  final String variant;

  /// `null` when the use site provides no payload.
  final HirType? payload;
  @override
  final SourceSpan span;
  @override
  HirType get driver => enumType;
}

class ImplementsConstraint extends DeferredConstraint {
  const ImplementsConstraint({
    required this.type,
    required this.trait,
    required this.span,
  });

  final HirType type;
  final HirTrait trait;
  @override
  final SourceSpan span;
  @override
  HirType get driver => type;
}
