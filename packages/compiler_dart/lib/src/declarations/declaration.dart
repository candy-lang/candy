import 'package:code_builder/code_builder.dart' as dart;
import 'package:compiler/compiler.dart';

import '../builtins.dart';
import 'class.dart';
import 'function.dart';
import 'module.dart';
import 'property.dart';

final compileDeclaration = Query<DeclarationId, Option<dart.Spec>>(
  'dart.compileDeclaration',
  provider: (context, declarationId) {
    final declaration = getDeclarationAst(context, declarationId);
    if (declaration.isBuiltin) {
      return compileBuiltin(context, declarationId);
    }

    if (declarationId.isModule) {
      compileModule(context, declarationIdToModuleId(context, declarationId));
      return Option.none();
    } else if (declarationId.isClass) {
      return Option.some(compileClass(context, declarationId));
    } else if (declarationId.isConstructor) {
      // Constructors are manually compiled within classes as they don't inherit
      // from [Spec].
      return Option.none();
    } else if (declarationId.isFunction) {
      return Option.some(compileFunction(context, declarationId));
    } else if (declarationId.isProperty) {
      return Option.some(compileProperty(context, declarationId));
    } else {
      throw CompilerError.unsupportedFeature(
        'Unsupported declaration for Dart compiler: `$declarationId`.',
      );
    }
  },
);
