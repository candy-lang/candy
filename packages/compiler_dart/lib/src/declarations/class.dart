import 'package:code_builder/code_builder.dart' as dart;
import 'package:compiler/compiler.dart';
import 'package:parser/parser.dart';
import 'package:dartx/dartx.dart';

import '../body.dart';
import '../constants.dart';
import '../type.dart';
import 'constructor.dart';
import 'declaration.dart';
import 'function.dart';
import 'property.dart';

final compileClass = Query<DeclarationId, dart.Class>(
  'dart.compileClass',
  evaluateAlways: true,
  provider: (context, declarationId) {
    // ignore: non_constant_identifier_names
    final classHir = getClassDeclarationHir(context, declarationId);

    final impls = getAllImplsForClass(context, declarationId)
        .map((id) => getImplDeclarationHir(context, id));
    final implements = impls
        .expand((impl) => impl.traits)
        .map((trait) => compileType(context, trait));
    final implInnerDeclarationIds =
        impls.expand((impl) => impl.innerDeclarationIds);
    final implMethodIds =
        implInnerDeclarationIds.where((id) => id.isFunction).toList();
    final methodOverrides = implMethodIds
        .map((id) => Tuple2(id, getFunctionDeclarationHir(context, id)))
        .map((values) {
      final id = values.first;
      final function = values.second;

      if (function.isStatic) {
        throw CompilerError.unsupportedFeature(
          'Static functions in impls are not yet supported.',
          location: ErrorLocation(
            declarationId.resourceId,
            getPropertyDeclarationAst(context, declarationId)
                .modifiers
                .firstWhere((w) => w is StaticModifierToken)
                .span,
          ),
        );
      }

      return dart.Method((b) => b
        ..annotations.add(dart.refer('override', dartCoreUrl))
        ..returns = compileType(context, function.returnType)
        ..name = function.name
        ..requiredParameters
            .addAll(compileParameters(context, function.valueParameters))
        ..body = compileBody(context, id).value);
    });
    // Super calls for all methods that aren't overriden in the impl.
    final methodDelegations = impls
        .expand((impl) => impl.traits)
        .map((trait) => trait.virtualModuleId)
        .map((moduleId) => moduleIdToDeclarationId(context, moduleId))
        .map((id) => getTraitDeclarationHir(context, id))
        .expand((trait) => trait.innerDeclarationIds)
        .where((id) => id.isFunction)
        .distinctBy((id) => id.simplePath.last.nameOrNull)
        .whereNot((id) {
          return implMethodIds.any((implId) =>
              implId.simplePath.last.nameOrNull ==
              id.simplePath.last.nameOrNull);
        })
        .map((id) => Tuple2(id, getFunctionDeclarationHir(context, id)))
        .map((inputs) {
          final id = inputs.first;
          final functionHir = inputs.second;

          return dart.Method((b) => b
            ..annotations.add(dart.refer('override', dartCoreUrl))
            ..returns = compileType(context, functionHir.returnType)
            ..name = functionHir.name
            ..types.addAll(functionHir.typeParameters
                .map((p) => compileTypeParameter(context, p)))
            ..requiredParameters
                .addAll(compileParameters(context, functionHir.valueParameters))
            ..body = compileBody(context, id).value);
        });

    final properties = classHir.innerDeclarationIds
        .where((id) => id.isProperty)
        .map((id) => compileProperty(context, id));
    final methods = classHir.innerDeclarationIds
        .where((id) => id.isFunction)
        .map((id) => compileFunction(context, id));
    return dart.Class((b) => b
      ..annotations.add(dart.refer('sealed', packageMetaUrl))
      ..name = classHir.name
      ..types.addAll(
          classHir.typeParameters.map((p) => compileTypeParameter(context, p)))
      ..implements.addAll(implements)
      ..constructors.addAll(compileConstructor(
        context,
        declarationId.inner(DeclarationPathData.constructor()),
      ))
      ..fields.addAll(properties)
      ..methods.addAll(methods)
      ..methods.addAll(methodOverrides)
      ..methods.addAll(methodDelegations));
  },
);
