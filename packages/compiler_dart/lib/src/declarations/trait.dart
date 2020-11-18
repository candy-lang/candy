import 'package:code_builder/code_builder.dart' as dart;
import 'package:compiler/compiler.dart';
import 'package:parser/parser.dart';

import '../constants.dart';
import '../type.dart';
import '../utils.dart';
import 'class.dart';
import 'declaration.dart';
import 'function.dart';

/// Traits get compiled into an abstract class containing the method signatures
/// and a mixin containing easily reusable default implementations.
final Query<DeclarationId, List<dart.Spec>> compileTrait =
    Query<DeclarationId, List<dart.Spec>>(
  'dart.compileTrait',
  evaluateAlways: true,
  provider: (context, declarationId) {
    // ignore: non_constant_identifier_names
    final traitHir = getTraitDeclarationHir(context, declarationId);

    final implements = <dart.Reference>[
      for (final bound in traitHir.upperBounds) compileType(context, bound)
    ];

    final properties = traitHir.innerDeclarationIds
        .where((id) => id.isProperty)
        .expand((id) => compilePropertyInsideTrait(context, id));
    final methods = traitHir.innerDeclarationIds
        .where((id) => id.isFunction)
        .map((id) => compileFunction(context, id))
        .toList();

    final moduleId = declarationIdToModuleId(context, declarationId);
    if (moduleId == CandyType.comparable.virtualModuleId) {
      implements.add(dart.TypeReference((b) => b
        ..symbol = 'Comparable'
        ..url = dartCoreUrl
        ..types.add(dart.refer('dynamic', dartCoreUrl))));
      methods.add(dart.Method((b) => b
        ..annotations.add(dart.refer('override', dartCoreUrl))
        ..returns = dart.refer('int', dartCoreUrl)
        ..name = 'compareTo'
        ..requiredParameters.add(dart.Parameter((b) => b
          ..type = dart.refer('dynamic', dartCoreUrl)
          ..name = 'other'))));
    }

    final name = compileTypeName(context, declarationId).symbol;
    final typeParameters = traitHir.typeParameters
        .map((p) => compileTypeParameter(context, p))
        .toList();
    return [
      dart.Class((b) => b
        ..abstract = true
        ..name = name
        ..types.addAll(typeParameters)
        ..implements.addAll(implements)
        ..constructors.add(dart.Constructor((b) => b..constant = true))
        ..methods.addAll(properties)
        ..methods.addAll(methods)),
      Mixin(
        name: '$name\$Default',
        types: typeParameters,
        on: traitHir.upperBounds.map((it) => compileType(context, it)).toList(),
        implements: [
          dart.TypeReference((b) => b
            ..symbol = name
            ..types.addAll(
              typeParameters.map((it) => it.rebuild((b) => b.bound = null)),
            ))
        ],
        methods: methods,
      ),
      for (final classId
          in traitHir.innerDeclarationIds.where((it) => it.isClass))
        ...compileClass(context, classId),
      for (final traitId
          in traitHir.innerDeclarationIds.where((it) => it.isTrait))
        ...compileTrait(context, traitId),
    ];
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
