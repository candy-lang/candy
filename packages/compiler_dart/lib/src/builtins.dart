import 'package:code_builder/code_builder.dart' as dart;
import 'package:compiler/compiler.dart';
import 'package:dartx/dartx.dart';

import 'constants.dart' hide srcDirectoryName;
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
        ModuleId.coreCollections.nested(['list', 'array', 'Array'])) {
      return compileArray();
    } else if (moduleId == ModuleId.corePrimitives.nested(['Any'])) {
      return compileAny();
    } else if (moduleId == ModuleId.corePrimitives.nested(['ToString'])) {
      return compileToString();
    } else if (moduleId == ModuleId.corePrimitives.nested(['Unit'])) {
      return compileUnit();
    } else if (moduleId == ModuleId.corePrimitives.nested(['Never'])) {
      return compileNever();
    } else if (moduleId == ModuleId.corePrimitives.nested(['Bool'])) {
      return compileBool();
    } else if (moduleId == ModuleId.corePrimitives.nested(['Number'])) {
      return compileNumber();
    } else if (moduleId == ModuleId.corePrimitives.nested(['Int'])) {
      return compileInt();
    } else if (moduleId == ModuleId.corePrimitives.nested(['Float'])) {
      return compileFloat();
    } else if (moduleId == ModuleId.corePrimitives.nested(['String'])) {
      return compileString();
    } else if (moduleId == ModuleId.coreIoPrint && name == 'print') {
      return compilePrint();
    } else if (moduleId ==
        ModuleId.coreRandomSource.nested(['DefaultRandomSource'])) {
      return compileDefaultRandomSource();
    }

    final declaration = getDeclarationAst(context, declarationId);
    throw CompilerError.internalError(
      'Unknown built-in declaration: `$declarationId`.',
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
  List<Output> compileArray();

  // primitives
  List<Output> compileAny();
  List<Output> compileToString();

  List<Output> compileUnit();
  List<Output> compileNever();

  List<Output> compileBool();

  List<Output> compileNumber();
  List<Output> compileInt();
  List<Output> compileFloat();

  List<Output> compileString();

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
          ..type = dart.refer('bool', dartCoreUrl)))
        ..requiredParameters.add(dart.Parameter((b) => b
          ..name = 'message'
          ..type = dart.refer('String', dartCoreUrl)))
        ..body = dart.Block(
          (b) => b.addExpression(dart.InvokeExpression.newOf(
            dart.refer('assert'),
            [dart.refer('condition'), dart.refer('message')],
            {},
            [],
          )),
        )),
    ];
  }

  @override
  List<dart.Spec> compileArray() {
    // `Array<Value>` corresponds to `List<Value>`, hence nothing to do.
    return [];
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
  List<dart.Spec> compileBool() {
    // `Bool` corresponds to `bool`, hence nothing to do.
    return [];
  }

  @override
  List<dart.Spec> compileNumber() {
    // `Number` corresponds to `num`, hence nothing to do.
    return [];
  }

  @override
  List<dart.Spec> compileInt() {
    // `Int` corresponds to `int`, hence nothing to do for the type itself.
    return [
      Extension(
        name: 'IntRandomExtension',
        on: dart.refer('int', dartCoreUrl),
        methods: [
          dart.Method((b) => b
            ..static = true
            ..returns = dart.refer('int', dartCoreUrl)
            ..name = 'randomSample'
            ..requiredParameters.add(dart.Parameter((b) => b
              ..type = compileType(context, CandyType.randomSource)
              ..name = 'source'))
            ..body = dart.Block((b) => b
              ..statements.add(dart
                  .refer('source')
                  .property('generateByte')
                  .call([], {}, [])
                  .returned
                  .statement))),
        ],
      ),
    ];
  }

  @override
  List<dart.Spec> compileFloat() {
    // `Float` corresponds to `double`, hence nothing to do.
    return [];
  }

  @override
  List<dart.Spec> compileString() {
    // `String` corresponds to `String`, hence nothing to do.
    return [];
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
              .assign(random.call([dart.refer('seed')], {}, []))
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
                .returned
                .statement)))))
    ];
  }
}
