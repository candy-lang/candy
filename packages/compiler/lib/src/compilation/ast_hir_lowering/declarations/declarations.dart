import 'package:parser/parser.dart' as ast;

import '../../../errors.dart';
import '../../../query.dart';
import '../../../utils.dart';
import '../../ast.dart';
import '../../hir/ids.dart';

final doesDeclarationExist = Query<DeclarationId, bool>(
  'doesDeclarationExist',
  provider: (context, declarationId) {
    if (declarationId.path.isEmpty) {
      return context.callQuery(doesResourceExist, declarationId.resourceId);
    }

    final ast = context.callQuery(getAst, declarationId.resourceId);
    final declaration =
        _findDeclarationAst(ast.declaration, declarationId.path);
    return declaration.isSome;
  },
);
final getDeclarationAst = Query<DeclarationId, ast.Declaration>(
  'getDeclarationAst',
  provider: (context, declarationId) {
    final ast = context.callQuery(getAst, declarationId.resourceId);
    final declaration =
        _findDeclarationAst(ast.declaration, declarationId.path);
    if (declaration.isNone) {
      throw CompilerError.internalError(
        "Couldn't find declaration `$declarationId`. Maybe call `doesDeclarationExist` first.\n\n${StackTrace.current}",
      );
    }
    return declaration.value;
  },
);

Option<ast.Declaration> _findDeclarationAst(
  ast.Declaration declaration,
  List<DisambiguatedDeclarationPathData> path,
) {
  if (path.isEmpty) return Option.some(declaration);

  final dPathData = path.first;
  final pathData = dPathData.data;
  final disambiguator = dPathData.disambiguator;

  var index = 0;
  for (final declaration in declaration.innerDeclarations) {
    if (declaration.declarationType != pathData.declarationType) {
      continue;
    }
    if (declaration.nameOrNull != pathData.nameOrNull) continue;
    if (index != disambiguator) {
      index++;
      continue;
    }

    return _findDeclarationAst(declaration, path.sublist(1));
  }
  return Option.none();
}

final getInnerDeclarationIds = Query<DeclarationId, List<DeclarationId>>(
  'getInnerDeclarationIds',
  provider: (context, declarationId) {
    final declarationAst = context.callQuery(getDeclarationAst, declarationId);

    final declarationIds = <DeclarationId>[];
    final moduleDisambiguator = <String, int>{};
    final traitDisambiguator = <String, int>{};
    final implDisambiguator = <String, int>{};
    final classDisambiguator = <String, int>{};
    final functionDisambiguator = <String, int>{};
    final propertyDisambiguator = <String, int>{};
    var propertyGetterDisambiguator = 0;
    var propertySetterDisambiguator = 0;
    for (final declaration in declarationAst.innerDeclarations) {
      if (declaration is ast.ModuleDeclaration) {
        final name = declaration.name.name;
        declarationIds.add(declarationId.inner(
          DeclarationPathData.module(name),
          moduleDisambiguator[name] = (moduleDisambiguator[name] ?? -1) + 1,
        ));
      } else if (declaration is ast.TraitDeclaration) {
        final name = declaration.name.name;
        declarationIds.add(declarationId.inner(
          DeclarationPathData.trait(name),
          traitDisambiguator[name] = (traitDisambiguator[name] ?? -1) + 1,
        ));
      } else if (declaration is ast.ImplDeclaration) {
        final name = declaration.trait?.toString();
        declarationIds.add(declarationId.inner(
          DeclarationPathData.impl(name),
          implDisambiguator[name] = (implDisambiguator[name] ?? -1) + 1,
        ));
      } else if (declaration is ast.ClassDeclaration) {
        final name = declaration.name.name;
        declarationIds.add(declarationId.inner(
          DeclarationPathData.class_(name),
          classDisambiguator[name] = (classDisambiguator[name] ?? -1) + 1,
        ));
      } else if (declaration is ast.FunctionDeclaration) {
        final name = declaration.name.name;
        declarationIds.add(declarationId.inner(
          DeclarationPathData.function(name),
          functionDisambiguator[name] = (functionDisambiguator[name] ?? -1) + 1,
        ));
      } else if (declaration is ast.PropertyDeclaration) {
        final name = declaration.name.name;
        declarationIds.add(declarationId.inner(
          DeclarationPathData.property(name),
          propertyDisambiguator[name] = (propertyDisambiguator[name] ?? -1) + 1,
        ));
      } else if (declaration is ast.GetterPropertyAccessor) {
        declarationIds.add(declarationId.inner(
          DeclarationPathData.propertyGetter(),
          propertyGetterDisambiguator++,
        ));
      } else if (declaration is ast.SetterPropertyAccessor) {
        declarationIds.add(declarationId.inner(
          DeclarationPathData.propertySetter(),
          propertySetterDisambiguator++,
        ));
      } else {
        assert(false);
      }
    }
    return declarationIds;
  },
);

extension on DeclarationPathData {
  DeclarationType get declarationType => when(
        module: (_) => DeclarationType.module,
        trait: (_) => DeclarationType.trait,
        impl: (_) => DeclarationType.impl,
        class_: (_) => DeclarationType.class_,
        constructor: () => DeclarationType.constructor,
        function: (_) => DeclarationType.function,
        property: (_) => DeclarationType.property,
        propertyGetter: () => DeclarationType.propertyGetter,
        propertySetter: () => DeclarationType.propertySetter,
      );
}

extension on ast.Declaration {
  String get nameOrNull {
    if (this is ast.ModuleDeclaration) {
      return (this as ast.ModuleDeclaration).name.name;
    } else if (this is ast.TraitDeclaration) {
      return (this as ast.TraitDeclaration).name.name;
    } else if (this is ast.ImplDeclaration) {
      return (this as ast.ImplDeclaration).type.toString();
    } else if (this is ast.ClassDeclaration) {
      return (this as ast.ClassDeclaration).name.name;
    } else if (this is ast.FunctionDeclaration) {
      return (this as ast.FunctionDeclaration).name.name;
    } else if (this is ast.PropertyDeclaration) {
      return (this as ast.PropertyDeclaration).name.name;
    } else if (this is ast.GetterPropertyAccessor) {
      return null;
    } else if (this is ast.SetterPropertyAccessor) {
      return null;
    }

    assert(false);
    return null;
  }

  List<ast.Declaration> get innerDeclarations {
    if (this is ast.ModuleDeclaration) {
      return (this as ast.ModuleDeclaration).body?.declarations ?? [];
    } else if (this is ast.TraitDeclaration) {
      return (this as ast.TraitDeclaration).body?.declarations ?? [];
    } else if (this is ast.ImplDeclaration) {
      return (this as ast.ImplDeclaration).body?.declarations ?? [];
    } else if (this is ast.ClassDeclaration) {
      return (this as ast.ClassDeclaration).body?.declarations ?? [];
    } else if (this is ast.FunctionDeclaration) {
      return [];
    } else if (this is ast.PropertyDeclaration) {
      return [];
    } else if (this is ast.GetterPropertyAccessor) {
      return [];
    } else if (this is ast.SetterPropertyAccessor) {
      return [];
    }

    assert(false);
    return null;
  }

  DeclarationType get declarationType {
    if (this is ast.ModuleDeclaration) {
      return DeclarationType.module;
    } else if (this is ast.TraitDeclaration) {
      return DeclarationType.trait;
    } else if (this is ast.ImplDeclaration) {
      return DeclarationType.impl;
    } else if (this is ast.ClassDeclaration) {
      return DeclarationType.class_;
    } else if (this is ast.FunctionDeclaration) {
      return DeclarationType.function;
    } else if (this is ast.PropertyDeclaration) {
      return DeclarationType.property;
    } else if (this is ast.GetterPropertyAccessor) {
      return DeclarationType.propertyGetter;
    } else if (this is ast.SetterPropertyAccessor) {
      return DeclarationType.propertySetter;
    }

    assert(false);
    return null;
  }
}

enum DeclarationType {
  module,
  trait,
  impl,
  // ignore: constant_identifier_names
  class_,
  constructor,
  function,
  property,
  propertyGetter,
  propertySetter,
}
