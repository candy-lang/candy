import 'package:code_builder/code_builder.dart' as dart;
import 'package:compiler/compiler.dart';
import 'package:dartx/dartx.dart';

import 'constants.dart' hide srcDirectoryName;

final compileBuiltin = Query<DeclarationId, Option<dart.Spec>>(
  'dart.compileBuiltin',
  provider: (context, declarationId) =>
      DartBuiltinCompiler().compile(context, declarationId),
);

abstract class BuiltinCompiler<Output> {
  Option<Output> compile(QueryContext context, DeclarationId declarationId) {
    if (declarationId.isImpl) return None();

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
    } else if (moduleId == ModuleId.coreStdio) {
      if (name == 'print') return compilePrint();
    }

    final declaration = getDeclarationAst(context, declarationId);
    throw CompilerError.internalError(
      'Unknown built-in declaration: `$declarationId`.',
      location: ErrorLocation(declarationId.resourceId, declaration.span),
    );
  }

  List<Output> compilePrimitiveGhosts() {
    return 2
        .rangeTo(10)
        .map(compileTuple)
        .mapNotNull((output) => output.valueOrNull)
        .toList();
  }

  // assert
  Option<Output> compileAssert();

  // collections
  // collections.list
  // collections.list.array
  Option<Output> compileArray();

  // primitives
  Option<Output> compileAny();
  Option<Output> compileToString();

  Option<Output> compileUnit();
  Option<Output> compileNever();

  Option<Output> compileBool();

  Option<Output> compileNumber();
  Option<Output> compileInt();
  Option<Output> compileFloat();

  Option<Output> compileString();

  Option<Output> compileTuple(int size);

  // stdio
  Option<Output> compilePrint();
}

class DartBuiltinCompiler extends BuiltinCompiler<dart.Spec> {
  @override
  Option<dart.Spec> compileAssert() {
    return Option.some(dart.Method.returnsVoid((b) => b
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
      )));
  }

  @override
  Option<dart.Spec> compileArray() {
    // `Array<Value>` corresponds to `List<Value>`, hence nothing to do.
    return Option.none();
  }

  @override
  Option<dart.Spec> compileAny() {
    // `Any` corresponds to `Object`, hence nothing to do.
    return Option.none();
  }

  @override
  Option<dart.Spec> compileToString() {
    // `ToString` is given by Dart's `Object`, hence nothing to do.
    return Option.none();
  }

  @override
  Option<dart.Spec> compileUnit() {
    // `Unit` corresponds to `void`, hence nothing to do.
    return Option.none();
  }

  @override
  Option<dart.Spec> compileNever() {
    return Option.some(dart.Class((b) => b..name = 'Never'));
  }

  @override
  Option<dart.Spec> compileBool() {
    // `Bool` corresponds to `bool`, hence nothing to do.
    return Option.none();
  }

  @override
  Option<dart.Spec> compileNumber() {
    // `Number` corresponds to `num`, hence nothing to do.
    return Option.none();
  }

  @override
  Option<dart.Spec> compileInt() {
    // `Int` corresponds to `int`, hence nothing to do.
    return Option.none();
  }

  @override
  Option<dart.Spec> compileFloat() {
    // `Float` corresponds to `double`, hence nothing to do.
    return Option.none();
  }

  @override
  Option<dart.Spec> compileString() {
    // `String` corresponds to `String`, hence nothing to do.
    return Option.none();
  }

  @override
  Option<dart.Spec> compileTuple(int size) {
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

    return Option.some(dart.Class((b) => b
      ..annotations.add(dart.refer('sealed', packageMetaUrl))
      ..name = 'Tuple$size'
      ..types.addAll(1.rangeTo(size).map((number) => dart.refer('T$number')))
      ..fields.addAll(fields.mapIndexed((index, name) => dart.Field((b) => b
        ..modifier = dart.FieldModifier.final$
        ..type = dart.refer('T${index + 1}')
        ..name = name)))
      ..constructors.add(dart.Constructor((b) => b
        ..constant = true
        ..requiredParameters.addAll(fields.map((name) => dart.Parameter((b) => b
          ..toThis = true
          ..name = name)))
        ..initializers.addAll(fields.map((name) => dart.refer('assert').call(
            [dart.refer(name).notEqualTo(dart.literalNull)], {}, []).code))))
      ..methods.add(dart.Method((b) => b
        ..annotations.add(dart.refer('override', dartCoreUrl))
        ..returns = dart.refer('String', dartCoreUrl)
        ..name = 'toString'
        ..lambda = true
        ..body = dart.Code("'(${fields.map((f) => '\$$f').join(', ')})'")))));
  }

  @override
  Option<dart.Spec> compilePrint() {
    return Option.some(dart.Method.returnsVoid((b) => b
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
      )));
  }
}
