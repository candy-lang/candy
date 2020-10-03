import 'package:code_builder/code_builder.dart' as dart;
import 'package:compiler/compiler.dart';

import 'function.dart';
import 'module.dart';

final compileDeclaration = Query<DeclarationId, Option<dart.Spec>>(
  'dart.compileDeclaration',
  provider: (context, declarationId) {
    if (declarationId.isModule) {
      compileModule(context, declarationIdToModuleId(context, declarationId));
      return Option.none();
    } else if (declarationId.isFunction) {
      return Option.some(compileFunction(context, declarationId));
    } else {
      throw CompilerError.unsupportedFeature(
        'Unsupported declaration for Dart compiler: `$declarationId`.',
      );
    }
  },
);
