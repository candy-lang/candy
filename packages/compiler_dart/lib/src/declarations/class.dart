import 'package:code_builder/code_builder.dart' as dart;
import 'package:compiler/compiler.dart';
import 'package:dartx/dartx.dart';
import 'package:parser/parser.dart';

import '../body.dart';
import '../constants.dart';
import '../type.dart';
import 'constructor.dart';
import 'declaration.dart';
import 'function.dart';
import 'property.dart';
import 'trait.dart';

final Query<DeclarationId, List<dart.Class>> compileClass =
    Query<DeclarationId, List<dart.Class>>(
  'dart.compileClass',
  evaluateAlways: true,
  provider: (context, declarationId) {
    // ignore: non_constant_identifier_names
    final classHir = getClassDeclarationHir(context, declarationId);

    final impls = getAllImplsForTraitOrClass(context, declarationId)
        .map((id) => getImplDeclarationHir(context, id));
    final traits = impls.expand((impl) => impl.traits);
    final implements = traits.map((it) => compileType(context, it));

    final implMethodIds = impls
        .expand((impl) => impl.innerDeclarationIds)
        .where((id) => id.isFunction)
        .toList();
    final methodOverrides = implMethodIds
        .map((id) => Tuple2(id, getFunctionDeclarationHir(context, id)))
        .expand((values) sync* {
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

      final implHir = getImplDeclarationHir(context, id.parent);
      final trait = implHir.traits.single;
      var name = function.name;
      if (trait == CandyType.comparable) {
        name = 'compareToTyped';

        final parameter = function.valueParameters.single;
        final comparableId =
            ModuleId.coreOperatorsComparison.nested(['Comparable']);
        final variants = {
          'Less': -1,
          'Equal': 0,
          'Greater': 1,
        };
        final statements = [
          dart
              .refer('this')
              .property('compareToTyped')
              .call([dart.refer(parameter.name)], {}, [])
              .assignFinal(
                'result',
                compileType(context, function.returnType),
              )
              .statement,
          for (final entry in variants.entries)
            dart.Block.of([
              dart.Code('if ('),
              dart
                  .refer('result')
                  .isA(compileType(
                    context,
                    CandyType.user(comparableId, entry.key),
                  ))
                  .code,
              dart.Code(') {'),
              dart.literalNum(entry.value).returned.statement,
              dart.Code('}'),
            ]),
          dart
              .refer('StateError', dartCoreUrl)
              .call(
                [
                  dart.literalString(
                    '`compareToTyped` returned an invalid object: `\$result`.',
                  )
                ],
                {},
                [],
              )
              .thrown
              .statement,
        ];
        yield dart.Method((b) => b
          ..annotations.add(dart.refer('override', dartCoreUrl))
          ..returns = compileType(context, CandyType.int)
          ..name = 'compareTo'
          ..requiredParameters
              .addAll(compileParameters(context, function.valueParameters))
          ..body = dart.Block((b) => b.statements.addAll(statements)));
      } else if (trait == CandyType.equals) {
        name = 'operator ==';
      }

      yield dart.Method((b) => b
        ..annotations.add(dart.refer('override', dartCoreUrl))
        ..returns = compileType(context, function.returnType)
        ..name = name
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

          const operatorMethods = {
            'lessThan': 'operator <',
            'lessThanOrEqual': 'operator <=',
            'greaterThan': 'operator >',
            'greaterThanOrEqual': 'operator >=',
          };
          final name = operatorMethods[functionHir.name] ?? functionHir.name;

          return dart.Method((b) => b
            ..annotations.add(dart.refer('override', dartCoreUrl))
            ..returns = compileType(context, functionHir.returnType)
            ..name = name
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
    return [
      dart.Class((b) => b
        ..annotations.add(dart.refer('sealed', packageMetaUrl))
        ..name = compileTypeName(context, declarationId).symbol
        ..types.addAll(classHir.typeParameters
            .map((p) => compileTypeParameter(context, p)))
        ..implements.addAll(implements)
        ..constructors.addAll(compileConstructor(
          context,
          declarationId.inner(DeclarationPathData.constructor()),
        ))
        ..fields.addAll(properties)
        ..methods.addAll(methods)
        ..methods.addAll(methodOverrides)
        ..methods.addAll(methodDelegations)),
      for (final classId
          in classHir.innerDeclarationIds.where((it) => it.isClass))
        ...compileClass(context, classId),
      for (final traitId
          in classHir.innerDeclarationIds.where((it) => it.isTrait))
        ...compileTrait(context, traitId),
    ];
  },
);
