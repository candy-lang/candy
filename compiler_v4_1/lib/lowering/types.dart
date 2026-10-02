import 'package:supernova/supernova.dart';

import '../ast.dart';
import '../context.dart';
import '../error.dart';
import '../hir/hir/hir.dart';
import '../hir/hir/ids.dart';
import '../lexer/token.dart';
import '../position.dart';
import 'declarations.dart';
import 'modules.dart';

/// The type parameters and `Self` type visible at some point in the program.
class const TypeScope({
  final List<HirTypeParameter> typeParameters = const [],
  final HirType? selfBaseType,

  /// The trait `Self` gets its associated types from: the enclosing trait, or
  /// the trait an enclosing impl implements. `null` everywhere else.
  final HirTrait? selfTrait,
}) {
  TypeScope extend(List<HirTypeParameter> more) => TypeScope(
    typeParameters: [...typeParameters, ...more],
    selfBaseType: selfBaseType,
    selfTrait: selfTrait,
  );

  HirTypeParameter? find(String name) =>
      typeParameters.where((it) => it.name == name).firstOrNull;
}

/// Lowers syntax-level types to [HirType], resolving names through an
/// [ImportScope].
class TypeLowerer({
  required final Context context,
  required final CollectedDeclarations declarations,
  required final ImportScope scope,

  /// Called for every trait used as a type. Whether it can be one depends on
  /// its functions, which may not be lowered yet, so the caller decides when
  /// to check.
  required final void Function(HirTrait trait, SourceSpan span)
  checkTraitObject,
}) {
  void error(SourceSpan span, String message) =>
      context.addError(CompilerError(span: span, message: message));

  void _reference(SourceSpan span, SourceSpan? target) =>
      context.addReference(span, [target]);

  HirType lowerType(AstType? type, TypeScope typeScope) {
    switch (type) {
      case null:
        return const HirType.error();

      case AstType_Function(:final parameterTypes, :final returnType):
        return HirType.function(
          parameterTypes: parameterTypes
              .map((it) => lowerType(it, typeScope))
              .toList(),
          returnType: lowerType(returnType.toNullable(), typeScope),
        );

      case AstType_Named(:final name, :final typeArguments):
        return _lowerNamedType(name, typeArguments, typeScope);

      case AstType_Associated(:final baseType, :final name):
        return _lowerAssociatedType(baseType, name, typeScope);
    }
  }

  /// `Self.Item` / `T.Item`. Which trait the associated type comes from is
  /// read off the base's bound rather than searched for, so this doesn't
  /// depend on any other declaration having been lowered yet.
  HirType _lowerAssociatedType(
    AstType baseAst,
    Token name,
    TypeScope typeScope,
  ) {
    final base = lowerType(baseAst, typeScope);
    if (base is HirType_Error) return base;

    final List<HirTrait> traits;
    switch (base) {
      case HirType_Self():
        final selfTrait = typeScope.selfTrait;
        if (selfTrait == null) {
          error(
            name.span,
            '`Self` has no associated types here, because this impl '
            "doesn't implement a trait.",
          );
          return const HirType.error();
        }
        traits = [selfTrait];
      case HirType_Parameter(:final id):
        traits = typeScope.find(id.name)?.upperBounds ?? const [];
        if (traits.isEmpty) {
          error(
            name.span,
            '`${id.name}` has no associated types, because it has no bound.',
          );
          return const HirType.error();
        }
      case _:
        error(
          name.span,
          'Associated types can only be read off `Self` or a bounded type '
          'parameter.',
        );
        return const HirType.error();
    }

    final matches = [
      for (final trait in traits)
        if (declarations.associatedTypeOf(trait.declaration, name.rawText)
            case final declaration?)
          (trait, declaration),
    ];
    if (matches.isEmpty) {
      error(
        name.span,
        traits.length == 1
            ? 'Trait `${traits.single.declaration.name}` has no associated '
                  'type `${name.rawText}`.'
            : 'None of the bounds on `$base` has an associated type '
                  '`${name.rawText}`.',
      );
      return const HirType.error();
    }
    if (matches.length > 1) {
      final traitNames = matches
          .map((it) => '`${it.$1.declaration.name}`')
          .join(', ');
      error(
        name.span,
        'Ambiguous associated type `${name.rawText}`: declared by $traitNames.',
      );
      return const HirType.error();
    }

    final (trait, declaration) = matches.single;
    _reference(name.span, declarations.spanOf(declaration));
    return HirType.associated(
      trait: trait,
      baseType: base,
      declaration: declaration,
    );
  }

  HirType _lowerNamedType(
    Token name,
    AstTypeArguments? typeArguments,
    TypeScope typeScope,
  ) {
    final text = name.rawText;

    if (text == 'Self') {
      final selfBaseType = typeScope.selfBaseType;
      if (selfBaseType == null) {
        error(name.span, '`Self` can only be used in traits and impls.');
        return const HirType.error();
      }
      if (selfBaseType is HirType_Named) {
        _reference(name.span, declarations.spanOf(selfBaseType.declaration));
      }
      return HirType.self(baseType: selfBaseType);
    }

    final typeParameter = typeScope.find(text);
    if (typeParameter != null) {
      if (typeArguments != null) {
        error(name.span, "Type parameters can't have type arguments.");
      }
      _reference(name.span, declarations.typeParameterSpanOf(typeParameter.id));
      return typeParameter.type;
    }

    final arguments = (typeArguments?.arguments ?? const [])
        .map((it) => lowerType(it, typeScope))
        .toList();

    switch (scope.lookup(text)) {
      case NamedType(:final id):
        _reference(name.span, declarations.spanOf(id));
        return HirType.named(
          declaration: id,
          typeArguments: _checkArity(id, arguments, name.span),
        );
      case NamedTrait(:final id):
        _reference(name.span, declarations.spanOf(id));
        final trait = HirTrait(
          declaration: id,
          typeArguments: _checkArity(id, arguments, name.span),
        );
        checkTraitObject(trait, name.span);
        return HirType.dyn(trait);
      case NamedAmbiguous(:final originsText):
        error(name.span, 'Ambiguous name `$text`, imported from $originsText.');
        return const HirType.error();
      case _:
        error(name.span, 'Unknown type: `$text`.');
        return const HirType.error();
    }
  }

  HirErrorOr<HirTrait> lowerTrait(
    AstType? type,
    SourceSpan fallbackSpan,
    TypeScope typeScope,
  ) {
    switch (type) {
      case null:
        return const None();

      case AstType_Function():
        error(fallbackSpan, 'A function type is not a trait.');
        return const None();

      case AstType_Associated(:final name):
        error(name.span, 'An associated type is not a trait.');
        return const None();

      case AstType_Named(:final name, :final typeArguments):
        final text = name.rawText;
        if (text == 'Self') {
          error(name.span, '`Self` is not a trait.');
          return const None();
        }
        if (typeScope.find(text) != null) {
          error(name.span, '`$text` is a type parameter, not a trait.');
          return const None();
        }

        final arguments = (typeArguments?.arguments ?? const [])
            .map((it) => lowerType(it, typeScope))
            .toList();

        switch (scope.lookup(text)) {
          case NamedTrait(:final id):
            _reference(name.span, declarations.spanOf(id));
            return Some(
              HirTrait(
                declaration: id,
                typeArguments: _checkArity(id, arguments, name.span),
              ),
            );
          case NamedType():
            error(name.span, "Types can't be used as traits.");
            return const None();
          case NamedAmbiguous(:final originsText):
            error(
              name.span,
              'Ambiguous name `$text`, imported from $originsText.',
            );
            return const None();
          case _:
            error(name.span, 'Unknown trait: `$text`.');
            return const None();
        }
    }
  }

  List<HirTypeParameter> lowerTypeParameters(
    DeclarationId owner,
    AstTypeParameters? parameters,
    TypeScope typeScope,
  ) {
    if (parameters == null) return const [];

    final lowered = <HirTypeParameter>[];
    for (final parameter in parameters.parameters) {
      final name = parameter.name.toNullable();
      if (name == null) continue;

      final text = name.rawText;
      if (typeScope.find(text) != null ||
          lowered.any((it) => it.name == text)) {
        error(name.span, 'Duplicate type parameter name: `$text`.');
        continue;
      }

      final id = TypeParameterId.declared(
        owner: owner,
        index: lowered.length,
        name: text,
      );
      // The bounds are lowered against the parameters declared so far, matching
      // the Rust compiler: `[T, U: Compare[T]]` works, `[T: Compare[U], U]`
      // doesn't.
      lowered.add(
        HirTypeParameter(
          id: id,
          upperBounds: lowerBounds(
            parameter.upperBounds,
            name.span,
            typeScope.extend(lowered),
          ),
        ),
      );
    }
    return lowered;
  }

  /// Lowers `: Equal + Hash`, dropping bounds that don't resolve to a trait
  /// and ones that repeat an earlier bound.
  List<HirTrait> lowerBounds(
    List<AstErrorOr<AstType>> bounds,
    SourceSpan fallbackSpan,
    TypeScope typeScope,
  ) {
    final lowered = <HirTrait>[];
    for (final bound in bounds) {
      if (lowerTrait(bound.toNullable(), fallbackSpan, typeScope) case Some(
        some: final trait,
      )) {
        if (!lowered.contains(trait)) lowered.add(trait);
      }
    }
    return lowered;
  }

  /// Pads with [HirType.error] or truncates so later passes can rely on the
  /// arity being right.
  List<HirType> _checkArity(
    DeclarationId declaration,
    List<HirType> arguments,
    SourceSpan span,
  ) {
    final expected = declarations.typeParameterCountOf(declaration);
    if (arguments.length == expected) return arguments;

    error(
      span,
      'Expected $expected type ${expected == 1 ? 'argument' : 'arguments'}, '
      'got ${arguments.length}.',
    );
    return [
      ...arguments.take(expected),
      for (var i = arguments.length; i < expected; i++) const HirType.error(),
    ];
  }
}

extension TypeParameterCount on CollectedDeclarations {
  /// Read off the syntax tree rather than the lowered declarations, so that
  /// mutually referential types don't need an ordering between them.
  int typeParameterCountOf(DeclarationId declaration) {
    final parameters = switch (declarations[declaration]) {
      CollectedStruct(:final ast) => ast.typeParameters,
      CollectedEnum(:final ast) => ast.typeParameters,
      CollectedTrait(:final ast) => ast.typeParameters,
      _ => null,
    };
    return parameters?.parameters.length ?? 0;
  }

  /// The trait's associated type of that name, for the same reason.
  DeclarationId? associatedTypeOf(DeclarationId trait, String name) {
    final collected = declarations[trait];
    if (collected is! CollectedTrait) return null;
    return collected.associatedTypes
        .where((it) => it.id.name == name)
        .map((it) => it.id)
        .firstOrNull;
  }
}
