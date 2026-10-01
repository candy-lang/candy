import 'package:supernova/supernova.dart';

import '../../hir/hir/ids.dart';

part 'ids.freezed.dart';

/// A type with every type parameter substituted away.
///
/// Freezed's structural equality is what interns these: two occurrences of
/// `Maybe[Int]` are the same key, so the type declarations hold one entry per
/// monomorphized type.
@freezed
sealed class MonoType with _$MonoType implements Comparable<MonoType> {
  const factory MonoType.named({
    required DeclarationId declaration,
    @Default(<MonoType>[]) List<MonoType> typeArguments,
  }) = MonoType_Named;
  const factory MonoType.function({
    required List<MonoType> parameterTypes,
    required MonoType returnType,
  }) = MonoType_Function;

  /// A trait object of [trait] instantiated with [typeArguments].
  const factory MonoType.dyn({
    required DeclarationId trait,
    @Default(<MonoType>[]) List<MonoType> typeArguments,
  }) = MonoType_Dyn;

  const MonoType._();

  @override
  int compareTo(MonoType other) => toString().compareTo(other.toString());

  /// A name that's unique across the whole program and usable as a C
  /// identifier.
  @override
  String toString() => switch (this) {
    MonoType_Named(:final declaration, :final typeArguments) =>
      '${declaration.mangled}${mangleArguments(typeArguments)}',
    MonoType_Function(:final parameterTypes, :final returnType) =>
      r'$Fun'
          '${mangleArguments(parameterTypes)}\$returns\$$returnType',
    MonoType_Dyn(:final trait, :final typeArguments) =>
      '\$Dyn\$${trait.mangled}${mangleArguments(typeArguments)}',
  };
}

/// One instantiation of a function.
///
/// [typeArguments] are the values of the function's relevant type parameters:
/// its owner's, then `Self` for a trait function, then its own. Call sites
/// carry substitution maps of varying shape, so they're canonicalized to this
/// list — otherwise one instantiation could end up under several keys.
@freezed
sealed class MonoFunctionId
    with _$MonoFunctionId
    implements Comparable<MonoFunctionId> {
  const factory MonoFunctionId({
    required DeclarationId declaration,
    @Default(<MonoType>[]) List<MonoType> typeArguments,
  }) = _MonoFunctionId;

  const MonoFunctionId._();

  @override
  int compareTo(MonoFunctionId other) => toString().compareTo(other.toString());

  @override
  String toString() =>
      '${declaration.mangled}${mangleArguments(typeArguments)}';
}

/// `$of$Int$Text$end`, or nothing for an empty list.
String mangleArguments(List<MonoType> types) =>
    types.isEmpty ? '' : '\$of\$${types.join(r'$')}\$end';

extension DeclarationIdMangling on DeclarationId {
  /// Candy identifiers can't contain `$`, so joining segments with it can only
  /// collide with the compiler's own reserved names, which are `$`-prefixed
  /// and therefore separated by a doubled `$`.
  String get mangled {
    final parent = this.parent;
    final prefix = parent == null
        ? [module.packageName, ...module.path].join(r'$')
        : parent.mangled;
    return '$prefix\$$name${disambiguator == 0 ? '' : '\$\$$disambiguator'}';
  }
}

/// The vtable that lets [concreteType] be used as the trait object [dynType].
@freezed
sealed class MonoVtableId
    with _$MonoVtableId
    implements Comparable<MonoVtableId> {
  const factory MonoVtableId({
    required MonoType concreteType,
    required MonoType_Dyn dynType,
  }) = _MonoVtableId;
  const MonoVtableId._();

  @override
  int compareTo(MonoVtableId other) => toString().compareTo(other.toString());

  @override
  String toString() => '$concreteType\$as\$$dynType';
}
