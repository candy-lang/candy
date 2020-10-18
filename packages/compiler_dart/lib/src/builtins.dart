import 'package:code_builder/code_builder.dart' as dart;
import 'package:collection/collection.dart';
import 'package:compiler/compiler.dart';

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
    } else if (moduleId == ModuleId.corePrimitives.nested(['Nothing'])) {
      return compileNothing();
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

  Option<Output> compileAny();
  Option<Output> compileToString();

  Option<Output> compileUnit();
  Option<Output> compileNothing();

  Option<Output> compileBool();

  Option<Output> compileNumber();
  Option<Output> compileInt();
  Option<Output> compileFloat();

  Option<Output> compileString();

  Option<Output> compilePrint();
}

class DartBuiltinCompiler extends BuiltinCompiler<dart.Spec> {
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
  Option<dart.Spec> compileNothing() {
    return Option.some(dart.Class((b) => b..name = 'Nothing'));
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
  Option<dart.Spec> compilePrint() {
    return Option.some(dart.Method((b) => b
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
