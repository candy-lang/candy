import 'package:code_builder/code_builder.dart' as dart;
import 'package:compiler/compiler.dart';

import 'constants.dart' hide srcDirectoryName;
import 'type.dart';

final compileBuiltin = Query<DeclarationId, Option<dart.Spec>>(
  'dart.compileBuiltin',
  provider: (context, declarationId) {
    if (declarationId ==
        DeclarationId(
                ResourceId(PackageId.this_, '$srcDirectoryName/main.candy'))
            .inner(DeclarationPathData.function('print'))) {
      return Option.some(dart.Method((b) => b
        ..name = 'print'
        ..requiredParameters.add(dart.Parameter((b) => b
          ..name = 'object'
          ..type = compileType(context, CandyType.any)))
        ..body = dart.Block(
          (b) => b.addExpression(dart.InvokeExpression.newOf(
            dart.refer('print', dartCoreUrl),
            [dart.refer('object')],
            {},
            [],
          )),
        )));
    } else {
      final declaration = getDeclarationAst(context, declarationId);
      throw CompilerError.internalError(
        'Unknown built-in declaration: `$declarationId`.',
        location: ErrorLocation(declarationId.resourceId, declaration.span),
      );
    }
  },
);
