import 'package:code_builder/code_builder.dart' as dart;
import 'package:compiler/compiler.dart';

import '../body.dart';
import '../type.dart';

final compileProperty = Query<DeclarationId, dart.Field>(
  'dart.compileProperty',
  evaluateAlways: true,
  provider: (context, declarationId) {
    // This is only for global properties and those in classes, not for
    // properties in traits as they create getter/setter methods.
    final property = getPropertyDeclarationHir(context, declarationId);
    final isInsideClass =
        declarationId.hasParent && declarationId.parent.isClass;

    return dart.Field((b) => b
      ..static = property.isStatic && declarationId.parent.isNotModule
      ..modifier = property.isMutable
          ? dart.FieldModifier.var$
          : dart.FieldModifier.final$
      ..name = property.name
      ..type = compileType(context, property.type)
      // In classes, the constructor is reponsible for handling defaults.
      ..assignment = isInsideClass
          ? null
          : compilePropertyInitializer(context, declarationId).valueOrNull);
  },
);
