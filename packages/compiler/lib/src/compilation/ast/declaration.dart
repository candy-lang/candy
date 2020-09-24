import 'package:parser/parser.dart' as ast;

import '../../query.dart';
import '../hir/ids.dart';
import 'parser.dart';

final getDeclarationAst = Query<DeclarationId, ast.Declaration>(
  'getDeclarationAst',
  provider: (context, declarationId) {
    assert(declarationId.path.isEmpty, 'Unsupported path in declaration ID.');

    final ast = context.callQuery(getAst, declarationId.resourceId);
    final declaration =
        _findDeclarationAst(ast.declarations, declarationId.path);
    assert(declaration != null, 'Declaration $declarationId not found.');
    return declaration;
  },
);

ast.Declaration _findDeclarationAst(
  List<ast.Declaration> declarations,
  List<DisambiguatedDeclarationPathData> path,
) {
  assert(path.isNotEmpty);
  final dPathData = path.first;
  final pathData = dPathData.data;
  final disambiguator = dPathData.disambiguator;

  var index = 0;
  for (final declaration in declarations) {
    if (declaration.runtimeType != pathData.correspondingDeclarationType) {
      continue;
    }
    if (declaration.rawName != pathData.nameOrNull) continue;
    if (index != disambiguator) {
      index++;
      continue;
    }

    final remainingPath = path.sublist(1);
    if (remainingPath.isEmpty) return declaration;
    return _findDeclarationAst(declaration.innerDeclarations, remainingPath);
  }
  return null;
}

extension on DeclarationPathData {
  Type get correspondingDeclarationType => when(
        module: (_) => ast.ModuleDeclaration,
        trait: (_) => ast.TraitDeclaration,
        impl: (_) => ast.ImplDeclaration,
        class_: (_) => ast.ClassDeclaration,
        function: (_) => ast.FunctionDeclaration,
        property: (_) => ast.PropertyDeclaration,
        propertyGetter: () => ast.GetterPropertyAccessor,
        propertySetter: () => ast.SetterPropertyAccessor,
      );
  String get nameOrNull => when(
        module: (name) => name,
        trait: (name) => name,
        impl: (name) => name,
        class_: (name) => name,
        function: (name) => name,
        property: (name) => name,
        propertyGetter: () => null,
        propertySetter: () => null,
      );
}

extension on ast.Declaration {
  String get rawName {
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
      return (this as ast.ModuleDeclaration).body.declarations;
    } else if (this is ast.TraitDeclaration) {
      return (this as ast.TraitDeclaration).body.declarations;
    } else if (this is ast.ImplDeclaration) {
      return (this as ast.ImplDeclaration).body.declarations;
    } else if (this is ast.ClassDeclaration) {
      return (this as ast.ClassDeclaration).body.declarations;
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
}
