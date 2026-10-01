import '../mono/ids.dart';
import '../mono/mono.dart';

/// Decides which enum variants need to be stored behind a pointer (to break
/// reference cycles between C struct definitions, e.g. `List = Cons(Item,
/// List) | Nil`) and in which order type declarations can be emitted so that
/// every by-value member is already fully defined.
///
/// This mirrors the cycle-breaking part of the old compiler's
/// `memory_layout.rs`. That pass also computed byte-level field offsets, but
/// `mono_to_c` never used them — C's own compiler lays structs out — so this
/// version only computes what the backend actually needs.
class TypeLayouts {
  TypeLayouts._(this.boxedVariants, this.declarationOrder);

  factory TypeLayouts.compute(Map<MonoType, MonoTypeDeclaration> declarations) {
    final context = _Context(declarations);
    // ignore: prefer_foreach
    for (final type in declarations.keys) {
      context._layOut(type);
    }
    return TypeLayouts._(context.boxedVariants, context.declarationOrder);
  }

  /// For each enum type, the names of the variants that are stored as a
  /// pointer instead of by value.
  final Map<MonoType, Set<String>> boxedVariants;

  /// Every key of the input map, ordered so a struct/enum's by-value members
  /// are declared before it.
  final List<MonoType> declarationOrder;
}

class _Context {
  _Context(this.declarations);

  final Map<MonoType, MonoTypeDeclaration> declarations;

  /// `null` while a type is being laid out (used to detect cycles), then
  /// replaced with `true` once it's fully processed.
  final Map<MonoType, bool> _visited = {};
  final Map<MonoType, Set<String>> boxedVariants = {};
  final List<MonoType> declarationOrder = [];

  /// Returns `false` if [type] participates in a not-yet-broken cycle (so
  /// the caller must box it if it can, i.e. if it's an enum variant),
  /// otherwise lays it out and returns `true`. A cycle that never passes
  /// through an enum can't be broken and propagates all the way up; that
  /// only happens for invalid programs, since a plain struct can't have
  /// infinite size.
  bool _layOut(MonoType type) {
    final visited = _visited[type];
    if (visited != null) return visited;

    _visited[type] = false;

    var ok = true;
    switch (declarations[type]) {
      case null:
      case MonoTypeDeclaration_Builtin():
      case MonoTypeDeclaration_Function():
      // The value lives behind a pointer, so the object has no by-value
      // members to lay out first.
      case MonoTypeDeclaration_Dyn():
        break;
      case MonoTypeDeclaration_Struct(:final fields):
        for (final field in fields) {
          if (!_layOut(field.type)) {
            ok = false;
            break;
          }
        }
      case MonoTypeDeclaration_Enum(:final variants):
        final boxed = <String>{};
        for (final variant in variants) {
          final valueType = variant.type;
          if (valueType != null && !_layOut(valueType)) boxed.add(variant.name);
        }
        boxedVariants[type] = boxed;
    }

    if (!ok) {
      _visited.remove(type);
      return false;
    }

    declarationOrder.add(type);
    _visited[type] = true;
    return true;
  }
}
