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
    // The implemented traits, where traits that got implemented multiple times
    // (with potentially different type parameters) are grouped together.
    final traitsByType = impls.expand((impl) => impl.traits).groupBy(
        (trait) => moduleIdToDeclarationId(context, trait.virtualModuleId));
    final implements = traitsByType.values
        .map((traitWithSameType) =>
            compileType(context, traitWithSameType.first))
        .toList();
    final traitsWithMultipleImpls = traitsByType.entries
        .where((entry) => entry.value.length > 1)
        .map((e) => e.value.first.virtualModuleId)
        .toSet();
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

      final isFromTraitWithMultipleImpls = traitsWithMultipleImpls
          .intersection(
            getImplDeclarationHir(context, function.id.parent)
                .traits
                .map((trait) => trait.virtualModuleId)
                .toSet(),
          )
          .isNotEmpty;
      if (isFromTraitWithMultipleImpls) {
        return dart.Method((b) => b
          ..returns = compileType(context, function.returnType)
          ..name = '${function.name}\$'
          ..requiredParameters
              .addAll(compileParameters(context, function.valueParameters))
          ..body = compileBody(context, id).value);
      } else {
        return dart.Method((b) => b
          ..annotations.add(dart.refer('override', dartCoreUrl))
          ..returns = compileType(context, function.returnType)
          ..name = function.name
          ..requiredParameters
              .addAll(compileParameters(context, function.valueParameters))
          ..body = compileBody(context, id).value);
      }
    });
    final dispatchMethods = traitsWithMultipleImpls
        .expand((traitModule) {
          final trait = getTraitDeclarationHir(
              context, moduleIdToDeclarationId(context, traitModule));
          return trait.innerDeclarationIds;
        })
        .where((id) => id.isFunction)
        .map((id) => getFunctionDeclarationHir(context, id))
        .expand((function) {
          return [
            dart.Method((b) => b
              ..annotations.add(dart.refer('override', dartCoreUrl))
              ..returns = compileType(context, function.returnType)
              ..name = function.name
              ..requiredParameters
                  .addAll(compileParameters(context, function.valueParameters))
              ..body = dart.Code(
                  'assert(false, "You shouldn\'t use this method directly.");')),
            dart.Method((b) => b
              ..returns = compileType(context, function.returnType)
              ..name = '${function.name}\$_typed'
              ..requiredParameters
                  .addAll(compileParameters(context, function.valueParameters))
              ..body = dart.Code(
                  'assert(false, "You shouldn\'t use this method directly.");')),
          ];
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
      ..methods.addAll(dispatchMethods)
      ..methods.addAll(methodOverrides)
      ..methods.addAll(methodDelegations));
  },
);

/*

  _i1.int foo$Foo$Int() {
    final _i1.int _0 = 3;
    return _0;
  }

  _i1.bool foo$Foo$Bool() {
    final _i1.bool _0 = true;
    return _0;
  }

  _i1.dynamic foo$_typed<$Foo>() {
    if ($Foo == _i1.int) {
      return foo$Foo$Int();
    } else if ($Foo == _i1.bool) {
      return foo$Foo$Bool();
    }
    assert(false);
  }

  @_i1.override
  _i1.dynamic foo() {
    assert(false);
  }
*/
