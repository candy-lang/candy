import 'package:code_builder/code_builder.dart' as dart;
import 'package:compiler/compiler.dart';

import '../body.dart';
import '../type.dart';

final compileProperty = Query<DeclarationId, dart.Field>(
  'dart.compileProperty',
  evaluateAlways: true,
  provider: (context, declarationId) {
    final property = getPropertyDeclarationHir(context, declarationId);
    return dart.Field((b) => b
      ..name = property.name
      ..type = compileType(context, property.type)
      ..assignment = property.initializer != null
          ? compilePropertyInitializer(context, declarationId)
          : null);
  },
);
