import 'package:code_builder/code_builder.dart' as dart;
import 'package:compiler/compiler.dart';
import 'package:dartx/dartx.dart';

import '../builtins.dart';
import '../type.dart';
import 'class.dart';
import 'function.dart';
import 'module.dart';
import 'property.dart';
import 'trait.dart';

final compileDeclaration = Query<DeclarationId, List<dart.Spec>>(
  'dart.compileDeclaration',
  provider: (context, declarationId) {
    final declaration = getDeclarationAst(context, declarationId);
    if (declaration.isBuiltin) return compileBuiltin(context, declarationId);

    if (declarationId.isModule) {
      compileModule(context, declarationIdToModuleId(context, declarationId));
      return [];
    } else if (declarationId.isTrait) {
      return compileTrait(context, declarationId);
    } else if (declarationId.isImpl) {
      // All impls are generated in the final class itself.
      return [];
    } else if (declarationId.isClass) {
      return compileClass(context, declarationId);
    } else if (declarationId.isConstructor) {
      // Constructors are manually compiled within classes as they don't inherit
      // from [Spec].
      return [];
    } else if (declarationId.isFunction) {
      final functionHir = getFunctionDeclarationHir(context, declarationId);
      if (functionHir.isTest) return [];
      return [compileFunction(context, declarationId)];
    } else if (declarationId.isProperty) {
      return [compileProperty(context, declarationId)];
    } else {
      throw CompilerError.unsupportedFeature(
        'Unsupported declaration for Dart compiler: `$declarationId`.',
      );
    }
  },
);

final compileTypeName = Query<DeclarationId, dart.Reference>(
  'dart.compileTypeName',
  provider: (context, declarationId) {
    assert(declarationId.isTrait || declarationId.isClass);
    final name = declarationId.simplePath
        .where((it) =>
            it is TraitDeclarationPathData || it is ClassDeclarationPathData)
        .reversed
        .map((it) => it.nameOrNull)
        .reduce((value, element) => '${element}_$value');

    var containingModule = declarationId.parent;
    while (containingModule.isTrait || containingModule.isClass) {
      containingModule = containingModule.parent;
    }
    final containingModuleId =
        declarationIdToModuleId(context, containingModule);
    return dart.refer(name, moduleIdToImportUrl(context, containingModuleId));
  },
);

final Query<TypeParameter, dart.TypeReference> compileTypeParameter =
    Query<TypeParameter, dart.TypeReference>(
  'dart.compileType',
  evaluateAlways: true,
  provider: (context, parameter) {
    return dart.TypeReference((b) => b
      ..symbol = parameter.name
      ..bound = compileType(context, parameter.upperBound)
      ..isNullable = false);
  },
);
