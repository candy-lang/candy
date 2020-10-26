import 'package:code_builder/code_builder.dart' as dart;
import 'package:collection/collection.dart';
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
    final moduleId = declarationIdToModuleId(context, declarationId);
    if (moduleId == ModuleId.corePrimitives.nested(['Any'])) {
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
      final equ = DeepCollectionEquality();
      final path = declarationId.simplePath;
      if (equ.equals(path, [DeclarationPathData.function('print')])) {
        return compilePrint();
      }
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

  Option<Output> compilePrint();
}

class DartBuiltinCompiler extends BuiltinCompiler<dart.Spec> {
  @override
  Option<dart.Spec> compileAny() {
    // `Any` corresponds to `Object`, hence Never to do.
    return Option.none();
  }

  @override
  Option<dart.Spec> compileToString() {
    // `ToString` is given by Dart's `Object`, hence Never to do.
    return Option.none();
  }

  @override
  Option<dart.Spec> compileUnit() {
    // `Unit` corresponds to `void`, hence Never to do.
    return Option.none();
  }

  @override
  Option<dart.Spec> compileNever() {
    return Option.some(dart.Class((b) => b..name = 'Never'));
  }

  @override
  Option<dart.Spec> compileBool() {
    // `Bool` corresponds to `bool`, hence Never to do.
    return Option.none();
  }

  @override
  Option<dart.Spec> compileNumber() {
    // `Number` corresponds to `num`, hence Never to do.
    return Option.none();
  }

  @override
  Option<dart.Spec> compileInt() {
    // `Int` corresponds to `int`, hence Never to do.
    return Option.none();
  }

  @override
  Option<dart.Spec> compileFloat() {
    // `Float` corresponds to `double`, hence Never to do.
    return Option.none();
  }

  @override
  Option<dart.Spec> compileString() {
    // `String` corresponds to `String`, hence Never to do.
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
      ..optionalParameters.add(dart.Parameter((b) => b
        ..named = true
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
