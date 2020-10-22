import 'package:code_builder/code_builder.dart' as dart;
import 'package:compiler/compiler.dart';
import 'package:parser/parser.dart';

import '../type.dart';
import 'function.dart';

final compileTrait = Query<DeclarationId, dart.Class>(
  'dart.compileTrait',
  evaluateAlways: true,
  provider: (context, declarationId) {
    // ignore: non_constant_identifier_names
    final trait = getTraitDeclarationHir(context, declarationId);

    final properties = trait.innerDeclarationIds
        .where((id) => id.isProperty)
        .expand((id) => compilePropertyInsideTrait(context, id));
    final methods = trait.innerDeclarationIds
        .where((id) => id.isFunction)
        .map((id) => compileFunction(context, id));
    return dart.Class((b) => b
      ..abstract = true
      ..name = trait.name
      ..constructors.add(dart.Constructor((b) => b..constant = true))
      ..methods.addAll(properties)
      ..methods.addAll(methods));
  },
);

final compilePropertyInsideTrait = Query<DeclarationId, List<dart.Method>>(
  'dart.compilePropertyInsideTrait',
  evaluateAlways: true,
  provider: (context, declarationId) {
    assert(declarationId.hasParent && declarationId.parent.isTrait);
    final property = getPropertyDeclarationHir(context, declarationId);

    if (property.isStatic) {
      throw CompilerError.unsupportedFeature(
        'Static properties in traits are not yet supported.',
        location: ErrorLocation(
          declarationId.resourceId,
          getPropertyDeclarationAst(context, declarationId)
              .modifiers
              .firstWhere((w) => w is StaticModifierToken)
              .span,
        ),
      );
    }

    return [
      dart.Method((b) => b
        ..returns = compileType(context, property.type)
        ..type = dart.MethodType.getter
        ..name = property.name),
      if (property.isMutable)
        dart.Method.returnsVoid((b) => b
          ..type = dart.MethodType.setter
          ..name = property.name
          ..requiredParameters.add(dart.Parameter((b) => b
            ..type = compileType(context, property.type)
            ..name = 'it'))),
    ];
  },
);
