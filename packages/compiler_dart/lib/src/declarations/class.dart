import 'package:code_builder/code_builder.dart' as dart;
import 'package:compiler/compiler.dart';
import 'package:parser/parser.dart';

import '../body.dart';
import '../constants.dart';
import '../type.dart';
import 'constructor.dart';
import 'declaration.dart';
import 'function.dart';
import 'property.dart';
import 'trait.dart';

final Query<DeclarationId, List<dart.Spec>> compileClass =
    Query<DeclarationId, List<dart.Spec>>(
  'dart.compileClass',
  evaluateAlways: true,
  provider: (context, declarationId) {
    // ignore: non_constant_identifier_names
    final classHir = getClassDeclarationHir(context, declarationId);

    final impls = getAllImplsForTraitOrClassOrImpl(context, declarationId)
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

      yield dart.Method((b) => b
        ..annotations.add(dart.refer('override', dartCoreUrl))
        ..returns = compileType(context, function.returnType)
        ..name = function.name
        ..types.addAll(function.typeParameters
            .map((it) => compileTypeParameter(context, it)))
        ..requiredParameters
            .addAll(compileParameters(context, function.valueParameters))
        ..body = compileBody(context, id).value);
    });

    final properties = classHir.innerDeclarationIds
        .where((id) => id.isProperty)
        .map((id) => compileProperty(context, id));
    final methods = classHir.innerDeclarationIds
        .where((id) => id.isFunction)
        .map((id) => compileFunction(context, id));
    final name = compileTypeName(context, declarationId).symbol;
    return [
      dart.Class((b) => b
        ..annotations.add(dart.refer('sealed', packageMetaUrl))
        ..name = name
        ..types.addAll(classHir.typeParameters
            .map((p) => compileTypeParameter(context, p)))
        ..mixins.addAll(traits.map((it) {
          final type = compileType(context, it);
          return dart.TypeReference((b) => b
            ..symbol = '${type.symbol}\$Default'
            ..types.addAll(it.arguments.map((it) => compileType(context, it)))
            ..url = type.url);
        }))
        ..implements.addAll(implements)
        ..constructors.addAll(compileConstructor(
          context,
          declarationId.inner(DeclarationPathData.constructor()),
        ))
        ..fields.addAll(properties)
        ..methods.addAll(methods)
        ..methods.addAll(methodOverrides)
        ..methods.add(dart.Method((b) => b
          ..annotations.add(dart.refer('override', dartCoreUrl))
          ..returns = dart.refer('String', dartCoreUrl)
          ..name = 'toString'
          ..body = dart.Block((b) {
            var typeParametersString = classHir.typeParameters
                .map((it) => it.name)
                .map((it) => '$it = \${$it}')
                .join(', ');
            if (typeParametersString.isNotEmpty) {
              typeParametersString = '<$typeParametersString>';
            }
            final propertiesString = properties
                .map((it) => it.name)
                .map((it) => ', "$it": \${$it}')
                .join();
            b.statements.add(dart
                .literalString(
                  '{"_type": "$name$typeParametersString"$propertiesString}',
                )
                .returned
                .statement);
          })))),
      for (final classId
          in classHir.innerDeclarationIds.where((it) => it.isClass))
        ...compileClass(context, classId),
      for (final traitId
          in classHir.innerDeclarationIds.where((it) => it.isTrait))
        ...compileTrait(context, traitId),
    ];
  },
);
