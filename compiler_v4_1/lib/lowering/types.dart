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
class TypeScope {
  const TypeScope({this.typeParameters = const [], this.selfBaseType});

  final List<HirTypeParameter> typeParameters;
  final HirType? selfBaseType;

  TypeScope extend(List<HirTypeParameter> more) => TypeScope(
    typeParameters: [...typeParameters, ...more],
    selfBaseType: selfBaseType,
  );

  HirTypeParameter? find(String name) =>
      typeParameters.where((it) => it.name == name).firstOrNull;
}

/// Lowers syntax-level types to [HirType], resolving names through an
/// [ImportScope].
class TypeLowerer {
  TypeLowerer({
    required this.context,
    required this.declarations,
    required this.scope,
  });

  final Context context;
  final CollectedDeclarations declarations;
  final ImportScope scope;

  void error(SourceSpan span, String message) =>
      context.addError(CompilerError(span: span, message: message));

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
    }
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
      return HirType.self(baseType: selfBaseType);
    }

    final typeParameter = typeScope.find(text);
    if (typeParameter != null) {
      if (typeArguments != null) {
        error(name.span, "Type parameters can't have type arguments.");
      }
      return typeParameter.type;
    }

    final arguments = (typeArguments?.arguments ?? const [])
        .map((it) => lowerType(it, typeScope))
        .toList();

    switch (scope.lookup(text)) {
      case NamedType(:final id):
        return HirType.named(
          declaration: id,
          typeArguments: _checkArity(id, arguments, name.span),
        );
      case NamedTrait():
        error(name.span, "Traits can't be used as types.");
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
            return Some(
              HirTrait(
                declaration: id,
                typeArguments: _checkArity(id, arguments, name.span),
              ),
            );
          case NamedType():
            error(name.span, "Types can't be used as traits.");
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
      // The bound is lowered against the parameters declared so far, matching
      // the Rust compiler: `[T, U: Compare[T]]` works, `[T: Compare[U], U]`
      // doesn't.
      final upperBound = switch (parameter.upperBound) {
        null => null,
        final bound => lowerTrait(
          bound.toNullable(),
          name.span,
          typeScope.extend(lowered),
        ),
      };
      lowered.add(HirTypeParameter(id: id, upperBound: upperBound));
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
}
