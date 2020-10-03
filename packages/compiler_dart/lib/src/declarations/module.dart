import 'package:code_builder/code_builder.dart' as dart;
import 'package:compiler/compiler.dart';
import 'package:dart_style/dart_style.dart';

import '../ids.dart';
import 'declaration.dart';

final _dartFmt = DartFormatter();

final compileModule = Query<ModuleId, Unit>(
  'dart.compileModule',
  evaluateAlways: true,
  provider: (context, moduleId) {
    final module = getModuleDeclarationHir(context, moduleId);

    final library = dart.Library((b) {
      for (final declarationId in module.innerDeclarationIds) {
        final compiled = compileDeclaration(context, declarationId);
        if (compiled.isSome) b.body.add(compiled.value);
      }
    });

    context.config.buildArtifactManager.setContent(
      moduleIdToBuildArtifactId(context, moduleId),
      _dartFmt.format(library.accept(dart.DartEmitter.scoped()).toString()),
    );

    return Unit();
  },
);
