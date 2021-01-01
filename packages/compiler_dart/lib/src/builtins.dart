import 'package:code_builder/code_builder.dart' as dart;
import 'package:compiler/compiler.dart';
import 'package:dartx/dartx.dart';
import 'package:parser/parser.dart' hide FunctionDeclaration;

import 'body.dart';
import 'constants.dart' hide srcDirectoryName;
import 'declarations/declaration.dart';
import 'declarations/function.dart';
import 'type.dart';
import 'utils.dart';

final compileBuiltin = Query<DeclarationId, List<dart.Spec>>(
  'dart.compileBuiltin',
  provider: (context, declarationId) =>
      DartBuiltinCompiler(context).compile(context, declarationId),
);

abstract class BuiltinCompiler<Output> {
  const BuiltinCompiler();

  List<Output> compile(QueryContext context, DeclarationId declarationId) {
    if (declarationId.isImpl) return [];

    final moduleId = declarationIdToModuleId(context, declarationId);
    final name = declarationId.simplePath.last.nameOrNull;

    if (moduleId == ModuleId.coreAssert) {
      if (name == 'assert') return compileAssert();
    } else if (moduleId ==
        ModuleId.coreCollections.nested(['array', 'Array'])) {
      return compileArray(declarationId);
    } else if (moduleId == ModuleId.corePrimitives.nested(['Any'])) {
      return compileAny();
    } else if (moduleId == ModuleId.corePrimitives.nested(['ToString'])) {
      return compileToString();
    } else if (moduleId == ModuleId.corePrimitives.nested(['Unit'])) {
      return compileUnit();
    } else if (moduleId == ModuleId.corePrimitives.nested(['Never'])) {
      return compileNever();
    } else if (moduleId == ModuleId.coreBool.nested(['Bool'])) {
      return compileBool(declarationId);
    } else if (moduleId == ModuleId.coreNumbersInt.nested(['Int'])) {
      return compileInt(declarationId);
    } else if (moduleId == ModuleId.coreString.nested(['String'])) {
      return compileString(declarationId);
    } else if (moduleId == ModuleId.coreIoPrint && name == 'print') {
      return compilePrint();
    } else if (moduleId ==
        ModuleId.coreRandomSource.nested(['DefaultRandomSource'])) {
      return compileDefaultRandomSource();
    }

    final declaration = getDeclarationAst(context, declarationId);
    throw CompilerError.internalError(
      'Unknown built-in declaration: `$declarationId` from module $moduleId.',
      location: ErrorLocation(declarationId.resourceId, declaration.span),
    );
  }

  List<Output> compilePrimitiveGhosts() {
    return 2.rangeTo(10).map(compileTuple).flatten().toList();
  }

  // assert
  List<Output> compileAssert();

  // collections
  // collections.list
  // collections.list.array
  List<Output> compileArray(DeclarationId id);

  // primitives
  List<Output> compileAny();
  List<Output> compileToString();

  List<Output> compileUnit();
  List<Output> compileNever();

  List<Output> compileBool(DeclarationId id);

  List<Output> compileInt(DeclarationId id);

  List<Output> compileString(DeclarationId id);

  List<Output> compileTuple(int size);

  // stdio
  List<Output> compilePrint();

  // random.source
  List<Output> compileDefaultRandomSource();
}

class DartBuiltinCompiler extends BuiltinCompiler<dart.Spec> {
  const DartBuiltinCompiler(this.context) : assert(context != null);

  final QueryContext context;

  @override
  List<dart.Spec> compileAssert() {
    return [
      dart.Method.returnsVoid((b) => b
        ..name = 'assert_'
        ..requiredParameters.add(dart.Parameter((b) => b
          ..name = 'condition'
          ..type = compileType(context, CandyType.bool)))
        ..requiredParameters.add(dart.Parameter((b) => b
          ..name = 'message'
          ..type = compileType(context, CandyType.string)))
        ..body = dart.Block(
          (b) => b.addExpression(dart.InvokeExpression.newOf(
            dart.refer('assert'),
            [
              dart.refer('condition').property('value'),
              dart.refer('message').property('value'),
            ],
            {},
            [],
          )),
        )),
    ];
  }

  @override
  List<dart.Spec> compileArray(DeclarationId id) {
    final impls = getAllImplsForTraitOrClassOrImpl(context, id)
        .map((it) => getImplDeclarationHir(context, it));
    final traits = impls.expand((impl) => impl.traits);
    final implements = traits.map((it) => compileType(context, it));
    final implMethodIds = impls
        .expand((impl) => impl.innerDeclarationIds)
        .where((id) => id.isFunction)
        .toList();
    final methodOverrides = implMethodIds
        .map((it) => Tuple2(it, getFunctionDeclarationHir(context, it)))
        .expand((values) sync* {
      final id = values.first;
      final function = values.second;

      if (function.isStatic) {
        throw CompilerError.unsupportedFeature(
          'Static functions in impls are not yet supported.',
          location: ErrorLocation(
            id.resourceId,
            getPropertyDeclarationAst(context, id)
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

    final t = dart.refer('T');
    final arrayT = dart.refer('Array<T>');
    final listT = dart.TypeReference((b) => b
      ..symbol = 'List'
      ..url = dartCoreUrl
      ..types.add(t));
    return [
      dart.Class((b) => b
        ..annotations.add(dart.refer('sealed', packageMetaUrl))
        ..name = 'Array'
        ..types.add(t)
        ..fields.add(dart.Field((b) => b
          ..name = 'value'
          ..type = listT))
        ..mixins.addAll(traits.map((it) {
          final type = compileType(context, it);
          return dart.TypeReference((b) => b
            ..symbol = '${type.symbol}\$Default'
            ..types.addAll(it.arguments.map((it) => compileType(context, it)))
            ..url = type.url);
        }))
        ..implements.addAll(implements)
        ..constructors.add(dart.Constructor((b) => b
          ..requiredParameters
              .add(dart.Parameter((b) => b..name = 'this.value'))))
        ..methods.addAll([
          dart.Method((b) => b
            ..name = 'generate'
            ..static = true
            ..types.add(t)
            ..returns = arrayT
            ..requiredParameters.addAll([
              dart.Parameter((b) => b
                ..name = 'length'
                ..type = compileType(context, CandyType.int)),
              dart.Parameter((b) => b
                ..name = 'generator'
                ..type = dart.FunctionType((b) => b
                  ..requiredParameters.add(compileType(context, CandyType.int))
                  ..returnType = t)),
            ])
            ..body = arrayT.call([
              listT.property('generate').call([
                dart.refer('length.value'),
                // The Dart code generator doesn't support lambdas, so we do an ugly workaround.
                dart.Method((b) => b
                  ..requiredParameters
                      .add(dart.Parameter((b) => b..name = 'index'))
                  ..body = dart.refer('generator').call([
                    dart.refer('index').wrapInCandyInt(context),
                  ]).code).closure,
              ]),
            ]).code),
          dart.Method((b) => b
            ..name = 'length'
            ..returns = compileType(context, CandyType.int)
            ..body = dart.refer('value.length').wrapInCandyInt(context).code),
          dart.Method((b) => b
            ..name = 'get'
            ..requiredParameters.add(dart.Parameter((b) => b
              ..name = 'index'
              ..type = compileType(context, CandyType.int)))
            ..returns = t
            ..body = dart.refer('value').index(dart.refer('index.value')).code),
          dart.Method((b) => b
            ..name = 'set'
            ..requiredParameters.addAll([
              dart.Parameter((b) => b
                ..name = 'index'
                ..type = compileType(context, CandyType.int)),
              dart.Parameter((b) => b
                ..name = 'item'
                ..type = t),
            ])
            ..returns = t
            ..body = dart
                .refer('value')
                .index(dart.refer('index.value'))
                .assign(dart.refer('item'))
                .code),
          dart.Method((b) => b
            ..name = 'toString'
            ..returns = dart.refer('String', dartCoreUrl)
            ..body = dart.refer('value').property('toString').call([]).code)
        ])
        ..methods.addAll(getClassDeclarationHir(context, id)
            .innerDeclarationIds
            .where((it) => it.getHir(context) is FunctionDeclaration)
            .where((it) => getBody(context, it).isSome)
            .map((it) {
          final function = getFunctionDeclarationHir(context, it);
          return dart.Method((b) => b
            ..returns = compileType(context, function.returnType)
            ..static = function.isStatic
            ..name = function.name
            ..types.addAll(function.typeParameters
                .map((it) => compileTypeParameter(context, it)))
            ..requiredParameters
                .addAll(compileParameters(context, function.valueParameters))
            ..body = compileBody(context, it).value);
        }))
        ..methods.addAll(methodOverrides)),
    ];
  }

  @override
  List<dart.Spec> compileAny() {
    // `Any` corresponds to `Object`, hence nothing to do.
    return [];
  }

  @override
  List<dart.Spec> compileToString() {
    // `ToString` is given by Dart's `Object`, hence nothing to do.
    return [];
  }

  @override
  List<dart.Spec> compileUnit() {
    // `Unit` corresponds to `void`, hence nothing to do.
    return [];
  }

  @override
  List<dart.Spec> compileNever() {
    return [dart.Class((b) => b..name = 'Never')];
  }

  @override
  List<dart.Spec> compileBool(DeclarationId id) {
    final impls = getAllImplsForTraitOrClassOrImpl(context, id)
        .map((it) => getImplDeclarationHir(context, it));
    final traits = impls.expand((impl) => impl.traits);
    final implements = traits.map((it) => compileType(context, it));
    final implMethodIds = impls
        .expand((impl) => impl.innerDeclarationIds)
        .where((id) => id.isFunction)
        .toList();
    final methodOverrides = implMethodIds
        .map((it) => Tuple2(it, getFunctionDeclarationHir(context, it)))
        .expand((values) sync* {
      final id = values.first;
      final function = values.second;

      if (function.isStatic) {
        throw CompilerError.unsupportedFeature(
          'Static functions in impls are not yet supported.',
          location: ErrorLocation(
            id.resourceId,
            getPropertyDeclarationAst(context, id)
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

    final otherBool = dart.Parameter((b) => b
      ..name = 'other'
      ..type = dart.refer('dynamic', dartCoreUrl));
    return [
      dart.Class((b) => b
        ..annotations.add(dart.refer('sealed', packageMetaUrl))
        ..name = 'Bool'
        ..fields.add(dart.Field((b) => b
          ..name = 'value'
          ..type = dart.refer('bool', dartCoreUrl)))
        ..mixins.addAll(traits.map((it) {
          final type = compileType(context, it);
          return dart.TypeReference((b) => b
            ..symbol = '${type.symbol}\$Default'
            ..types.addAll(it.arguments.map((it) => compileType(context, it)))
            ..url = type.url);
        }))
        ..implements.addAll(implements)
        ..constructors.add(dart.Constructor((b) => b
          ..requiredParameters
              .add(dart.Parameter((b) => b..name = 'this.value'))))
        ..methods.addAll([
          dart.Method((b) => b
            ..name = 'equals'
            ..returns = compileType(context, CandyType.bool)
            ..requiredParameters.add(otherBool)
            ..body = dart
                .refer('value')
                .equalTo(dart.refer('other.value'))
                .wrapInCandyBool(context)
                .code),
          dart.Method((b) => b
            ..name = 'and'
            ..returns = compileType(context, CandyType.bool)
            ..requiredParameters.add(otherBool)
            ..body = dart
                .refer('value')
                .and(dart.refer('other.value'))
                .wrapInCandyBool(context)
                .code),
          dart.Method((b) => b
            ..name = 'or'
            ..returns = compileType(context, CandyType.bool)
            ..requiredParameters.add(otherBool)
            ..body = dart
                .refer('value')
                .or(dart.refer('other.value'))
                .wrapInCandyBool(context)
                .code),
          dart.Method((b) => b
            ..name = 'opposite'
            ..returns = compileType(context, CandyType.bool)
            ..body =
                dart.refer('value').negate().wrapInCandyBool(context).code),
          dart.Method((b) => b
            ..name = 'implies'
            ..returns = compileType(context, CandyType.bool)
            ..requiredParameters.add(otherBool)
            ..body = dart
                .refer('value')
                .negate()
                .or(dart.refer('other.value'))
                .wrapInCandyBool(context)
                .code),
          dart.Method((b) => b
            ..name = 'toString'
            ..returns = dart.refer('String', dartCoreUrl)
            ..body = dart.refer('value').property('toString').call([]).code)
        ])
        ..methods.addAll(methodOverrides)),
    ];
  }

  @override
  List<dart.Spec> compileInt(DeclarationId id) {
    final impls = getAllImplsForTraitOrClassOrImpl(context, id)
        .map((it) => getImplDeclarationHir(context, it));
    final traits = impls.expand((impl) => impl.traits);
    final implements = traits.map((it) => compileType(context, it));
    final implMethodIds = impls
        .expand((impl) => impl.innerDeclarationIds)
        .where((id) => id.isFunction)
        .toList();
    final methodOverrides = implMethodIds
        .map((it) => Tuple2(it, getFunctionDeclarationHir(context, it)))
        .expand((values) sync* {
      final id = values.first;
      final function = values.second;

      if (function.isStatic) {
        throw CompilerError.unsupportedFeature(
          'Static functions in impls are not yet supported.',
          location: ErrorLocation(
            id.resourceId,
            getPropertyDeclarationAst(context, id)
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

    final otherInt = dart.Parameter((b) => b
      ..name = 'other'
      ..type = dart.refer('dynamic', dartCoreUrl));
    return [
      dart.Class((b) => b
        ..annotations.add(dart.refer('sealed', packageMetaUrl))
        ..name = 'Int'
        ..fields.add(dart.Field((b) => b
          ..name = 'value'
          ..type = dart.refer('int', dartCoreUrl)))
        ..mixins.addAll(traits.map((it) {
          final type = compileType(context, it);
          return dart.TypeReference((b) => b
            ..symbol = '${type.symbol}\$Default'
            ..types.addAll(it.arguments.map((it) => compileType(context, it)))
            ..url = type.url);
        }))
        ..implements.addAll(implements)
        ..constructors.add(dart.Constructor((b) => b
          ..requiredParameters
              .add(dart.Parameter((b) => b..name = 'this.value'))))
        ..methods.addAll([
          dart.Method((b) => b
            ..name = 'equals'
            ..returns = compileType(context, CandyType.bool)
            ..requiredParameters.add(otherInt)
            ..body = dart
                .refer('value')
                .equalTo(dart.refer('other.value'))
                .wrapInCandyBool(context)
                .code),
          dart.Method((b) => b
            ..name = 'compareTo'
            ..returns = compileType(
              context,
              CandyType.union([
                CandyType.comparableLess,
                CandyType.comparableEqual,
                CandyType.comparableGreater,
              ]),
            )
            ..requiredParameters.add(otherInt)
            ..body = dart
                .refer('value.compareTo')
                .call([dart.refer('other.value')])
                .toComparisonResult(context)
                .code),
          dart.Method((b) => b
            ..name = 'add'
            ..returns = compileType(context, CandyType.int)
            ..requiredParameters.add(otherInt)
            ..body = dart
                .refer('value')
                .operatorAdd(dart.refer('other.value'))
                .wrapInCandyInt(context)
                .code),
          dart.Method((b) => b
            ..name = 'subtract'
            ..returns = compileType(context, CandyType.int)
            ..requiredParameters.add(otherInt)
            ..body = dart
                .refer('value')
                .operatorSubstract(dart.refer('other.value'))
                .wrapInCandyInt(context)
                .code),
          dart.Method((b) => b
            ..name = 'negate'
            ..returns = compileType(context, CandyType.int)
            ..body = dart.refer('-value').wrapInCandyInt(context).code),
          dart.Method((b) => b
            ..name = 'multiply'
            ..returns = compileType(context, CandyType.int)
            ..requiredParameters.add(otherInt)
            ..body = dart
                .refer('value')
                .operatorMultiply(dart.refer('other.value'))
                .wrapInCandyInt(context)
                .code),
          dart.Method((b) => b
            ..name = 'divideTruncating'
            ..returns = compileType(context, CandyType.int)
            ..requiredParameters.add(otherInt)
            ..body = dart
                .refer('value ~/ other.value')
                .wrapInCandyInt(context)
                .code),
          dart.Method((b) => b
            ..name = 'modulo'
            ..returns = compileType(context, CandyType.int)
            ..requiredParameters.add(otherInt)
            ..body = dart
                .refer('value')
                .operatorEuclideanModulo(dart.refer('other.value'))
                .wrapInCandyInt(context)
                .code),
          dart.Method((b) => b
            ..name = 'toString'
            ..returns = dart.refer('String', dartCoreUrl)
            ..body = dart.refer('value').property('toString').call([]).code)
        ])
        ..methods.addAll(methodOverrides)),
    ];
  }

  @override
  List<dart.Spec> compileString(DeclarationId id) {
    // `String` corresponds to `String`, hence nothing to do for the type itself.
    final impls = getAllImplsForTraitOrClassOrImpl(context, id)
        .map((it) => getImplDeclarationHir(context, it));
    final traits = impls.expand((impl) => impl.traits);
    final implements = traits.map((it) => compileType(context, it));
    final implMethodIds = impls
        .expand((impl) => impl.innerDeclarationIds)
        .where((id) => id.isFunction)
        .toList();
    final methodOverrides = implMethodIds
        .map((it) => Tuple2(it, getFunctionDeclarationHir(context, it)))
        .expand((values) sync* {
      final id = values.first;
      final function = values.second;

      if (function.isStatic) {
        throw CompilerError.unsupportedFeature(
          'Static functions in impls are not yet supported.',
          location: ErrorLocation(
            id.resourceId,
            getPropertyDeclarationAst(context, id)
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

    final otherString = dart.Parameter((b) => b
      ..name = 'other'
      ..type = dart.refer('dynamic', dartCoreUrl));
    return [
      dart.Class((b) => b
        ..annotations.add(dart.refer('sealed', packageMetaUrl))
        ..name = 'String'
        ..fields.add(dart.Field((b) => b
          ..name = 'value'
          ..type = dart.refer('String', dartCoreUrl)))
        ..mixins.addAll(traits.map((it) {
          final type = compileType(context, it);
          return dart.TypeReference((b) => b
            ..symbol = '${type.symbol}\$Default'
            ..types.addAll(it.arguments.map((it) => compileType(context, it)))
            ..url = type.url);
        }))
        ..implements.addAll(implements)
        ..constructors.add(dart.Constructor((b) => b
          ..requiredParameters
              .add(dart.Parameter((b) => b..name = 'this.value'))))
        ..methods.addAll([
          dart.Method((b) => b
            ..name = 'equals'
            ..returns = compileType(context, CandyType.bool)
            ..requiredParameters.add(otherString)
            ..body = dart
                .refer('value')
                .equalTo(dart.refer('other.value'))
                .wrapInCandyBool(context)
                .code),
          dart.Method((b) => b
            ..name = 'compareTo'
            ..returns = compileType(
              context,
              CandyType.union([
                CandyType.comparableLess,
                CandyType.comparableEqual,
                CandyType.comparableGreater,
              ]),
            )
            ..requiredParameters.add(otherString)
            ..body = dart
                .refer('value.compareTo')
                .call([dart.refer('other.value')])
                .toComparisonResult(context)
                .code),
          dart.Method((b) => b
            ..name = 'characters'
            ..returns =
                compileType(context, CandyType.iterable(CandyType.string))
            ..body = compileTypeName(
              context,
              moduleIdToDeclarationId(context, CandyType.arrayListModuleId),
            ).property('fromArray').call(
              [
                dart
                    .refer('value')
                    .property('characters')
                    .property('map')
                    .call([
                      dart.Method((b) => b
                        ..requiredParameters.add(dart.Parameter((b) => b
                          ..name = 'char'
                          ..type = dart.refer('String', dartCoreUrl)))
                        ..body = dart
                            .refer('char')
                            .wrapInCandyString(context)
                            .code).closure
                    ])
                    .property('toList')
                    .call([])
                    .wrapInCandyArray(context, CandyType.string),
              ],
              {},
              [compileType(context, CandyType.string)],
            ).code),
          dart.Method((b) => b
            ..name = 'substring'
            ..returns = compileType(context, CandyType.string)
            ..requiredParameters.addAll([
              dart.Parameter((b) => b
                ..name = 'offset'
                ..type = compileType(context, CandyType.int)),
              dart.Parameter((b) => b
                ..name = 'length'
                ..type = compileType(context, CandyType.int)),
            ])
            ..body = dart
                .refer('value.substring')
                .call([dart.refer('offset.value'), dart.refer('length.value')])
                .wrapInCandyString(context)
                .code),
          dart.Method((b) => b
            ..name = 'length'
            ..returns = compileType(context, CandyType.int)
            ..body = dart.refer('value.length').wrapInCandyInt(context).code),
          dart.Method((b) => b
            ..name = 'toString'
            ..returns = dart.refer('String', dartCoreUrl)
            ..body = dart.refer('value').property('toString').call([]).code)
        ])
        ..methods.addAll(methodOverrides)),
    ];
  }

  @override
  List<dart.Spec> compileTuple(int size) {
    const fieldNames = [
      'first',
      'second',
      'third',
      'fourth',
      'fifth',
      'sixth',
      'seventh',
      'eight',
      'nineth',
      'tenth',
    ];

    final fields = 1.rangeTo(size).map((i) => fieldNames[i - 1]);

    return [
      dart.Class((b) => b
        ..annotations.add(dart.refer('sealed', packageMetaUrl))
        ..name = 'Tuple$size'
        ..types.addAll(1.rangeTo(size).map((number) => dart.refer('T$number')))
        ..fields.addAll(fields.mapIndexed((index, name) => dart.Field((b) => b
          ..modifier = dart.FieldModifier.final$
          ..type = dart.refer('T${index + 1}')
          ..name = name)))
        ..constructors.add(dart.Constructor((b) => b
          ..constant = true
          ..requiredParameters
              .addAll(fields.map((name) => dart.Parameter((b) => b
                ..toThis = true
                ..name = name)))
          ..initializers.addAll(fields.map((name) => dart.refer('assert').call(
              [dart.refer(name).notEqualTo(dart.literalNull)], {}, []).code))))
        ..methods.add(dart.Method((b) => b
          ..annotations.add(dart.refer('override', dartCoreUrl))
          ..returns = dart.refer('String', dartCoreUrl)
          ..name = 'toString'
          ..lambda = true
          ..body = dart.Code("'(${fields.map((f) => '\$$f').join(', ')})'")))),
    ];
  }

  @override
  List<dart.Spec> compilePrint() {
    return [
      dart.Method.returnsVoid((b) => b
        ..name = 'print'
        ..requiredParameters.add(dart.Parameter((b) => b
          ..name = 'object'
          ..type = dart.refer('Object', dartCoreUrl)))
        ..body = dart.Block(
          (b) => b.addExpression(dart.InvokeExpression.newOf(
            dart.refer('print', dartCoreUrl),
            [dart.refer('object')],
            {},
            [],
          )),
        )),
    ];
  }

  @override
  List<dart.Spec> compileDefaultRandomSource() {
    final int = compileType(context, CandyType.int);
    final random = dart.refer('Random', dartMathUrl);
    return [
      dart.Class((b) => b
        ..name = 'DefaultRandomSource'
        ..implements.add(compileType(context, CandyType.randomSource))
        ..mixins.add(dart.refer('RandomSource\$Default'))
        ..constructors.add(dart.Constructor((b) => b
          ..optionalParameters.add(dart.Parameter((b) => b
            ..named = false
            ..type = int
            ..name = 'seed'))
          ..initializers.add(dart
              .refer('_random')
              .assign(random.call([dart.refer('seed.value')], {}, []))
              .code)))
        ..methods.add(dart.Method((b) => b
          ..static = true
          ..name = 'withSeed'
          ..requiredParameters.add(dart.Parameter((b) => b
            ..type = int
            ..name = 'seed'))
          ..body = dart.Block((b) => b
            ..statements.add(dart
                .refer('DefaultRandomSource')
                .call([dart.refer('seed')], {}, [])
                .returned
                .statement))))
        ..fields.add(dart.Field((b) => b
          ..modifier = dart.FieldModifier.final$
          ..type = random
          ..name = '_random'))
        ..methods.add(dart.Method((b) => b
          ..annotations.add(dart.refer('override', dartCoreUrl))
          ..returns = compileType(context, CandyType.int)
          ..name = 'generateByte'
          ..body = dart.Block((b) => b
            ..statements.add(dart
                .refer('_random')
                .property('nextInt')
                .call([dart.literalNum(1 << 8)], {}, [])
                .wrapInCandyInt(context)
                .returned
                .statement)))))
    ];
  }
}

extension WrappingInCandyTypes on dart.Expression {
  dart.Expression wrapInCandyBool(QueryContext context) {
    return compileType(context, CandyType.bool).call([this]);
  }

  dart.Expression wrapInCandyInt(QueryContext context) {
    return compileType(context, CandyType.int).call([this]);
  }

  dart.Expression wrapInCandyString(QueryContext context) {
    return compileType(context, CandyType.string).call([this]);
  }

  dart.Expression wrapInCandyArray(QueryContext context, CandyType itemType) {
    return compileType(context, CandyType.array(itemType)).call([this]);
  }

  dart.Expression toComparisonResult(QueryContext context) {
    return dart.Method((b) => b
      ..body = dart.Block((b) => b
        ..statements.addAll([
          // ignore: unnecessary_this
          this.assignFinal('result').statement,
          dart.Code('if (result < 0) {'),
          compileType(context, CandyType.comparableLess)
              .call([])
              .returned
              .statement,
          dart.Code('} else if (result > 0) {'),
          compileType(context, CandyType.comparableGreater)
              .call([])
              .returned
              .statement,
          dart.Code('} else {'),
          compileType(context, CandyType.comparableEqual)
              .call([])
              .returned
              .statement,
          dart.Code('}'),
        ]))).closure.call([]);
  }
}
