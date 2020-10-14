import 'package:code_builder/code_builder.dart' as dart;
import 'package:compiler/compiler.dart';
import 'package:compiler_dart/src/constants.dart';

import '../body.dart';

final compileConstructor = Query<DeclarationId, Iterable<dart.Constructor>>(
  'dart.compileConstructor',
  evaluateAlways: true,
  provider: (context, declarationId) sync* {
    assert(true);
    final constructor = getConstructorDeclarationHir(context, declarationId);
    final parameters = constructor.parameters;

    if (parameters.every((p) => p.defaultValue == null)) {
      yield _compileWithoutDefaults(parameters);
    } else {
      final className =
          (declarationId.parent.simplePath.last as ClassDeclarationPathData)
              .name;
      yield _compileWithDefaultsPublic(context, className, parameters);
      yield _compileWithDefaultsPrivate(parameters);
    }
  },
);

dart.Constructor _compileWithoutDefaults(List<ValueParameter> parameters) {
  final dartParameters = parameters.map((parameter) {
    return dart.Parameter((b) => b
      ..named = true
      // TODO(JonasWanke): Change this when we support Dart NNBD.
      // ..required = true
      ..annotations.add(dart.refer('required', packageMetaUrl))
      ..toThis = true
      ..name = parameter.name);
  });
  final initializers =
      parameters.map((p) => _nonNullAssert(dart.refer(p.name)).code);
  return dart.Constructor((b) => b
    ..optionalParameters.addAll(dartParameters)
    ..initializers.addAll(initializers));
}

dart.Constructor _compileWithDefaultsPublic(
  QueryContext context,
  String className,
  List<ValueParameter> parameters,
) {
  final publicParameters = parameters.map((parameter) {
    return dart.Parameter((b) {
      b.named = true;
      if (parameter.defaultValue == null) {
        // TODO(JonasWanke): Change this when we support Dart NNBD.
        // b.required = true;
        b.annotations.add(dart.refer('required', packageMetaUrl));
      }
      b.name = parameter.name;
    });
  });
  final publicBody = dart.Block((b) {
    for (final parameter in parameters) {
      final paramRefer = dart.refer(parameter.name);
      if (parameter.defaultValue == null) {
        b.addExpression(_nonNullAssert(paramRefer));
      } else {
        final defaultValue = compileExpression(context, parameter.defaultValue);
        b.addExpression(paramRefer.assignNullAware(defaultValue));
      }
    }

    b.addExpression(dart
        .refer(className)
        .property('_')
        .call(parameters.map((p) => dart.refer(p.name)))
        .returned);
  });
  return dart.Constructor((b) => b
    ..factory = true
    ..optionalParameters.addAll(publicParameters)
    ..body = publicBody);
}

dart.Constructor _compileWithDefaultsPrivate(List<ValueParameter> parameters) {
  final privateParameters = parameters.map((parameter) {
    return dart.Parameter((b) => b
      ..toThis = true
      ..name = parameter.name);
  });
  return dart.Constructor((b) => b
    ..constant = true
    ..name = '_'
    ..requiredParameters.addAll(privateParameters));
}

dart.Expression _nonNullAssert(dart.Expression expression) {
  return dart.InvokeExpression.newOf(
    dart.refer('assert'),
    [expression.notEqualTo(dart.literalNull)],
    {},
    [],
  );
}
