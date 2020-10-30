import 'package:code_builder/code_builder.dart' as dart;
import 'package:compiler/compiler.dart';

import '../body.dart';
import '../type.dart';

final compileConstructor = Query<DeclarationId, List<dart.Constructor>>(
  'dart.compileConstructor',
  evaluateAlways: true,
  provider: (context, declarationId) {
    assert(true);
    final constructor = getConstructorDeclarationHir(context, declarationId);
    final parameters = constructor.valueParameters;

    if (parameters.every((p) => p.defaultValue == null)) {
      return [_compileWithoutDefaults(parameters)];
    } else {
      final className =
          (declarationId.parent.simplePath.last as ClassDeclarationPathData)
              .name;
      return [
        _compileWithDefaultsPublic(context, className, parameters),
        _compileWithDefaultsPrivate(parameters),
      ];
    }
  },
);

dart.Constructor _compileWithoutDefaults(List<ValueParameter> parameters) {
  final dartParameters = parameters.map((parameter) {
    return dart.Parameter((b) => b
      ..toThis = true
      ..name = parameter.name);
  });
  final initializers =
      parameters.map((p) => _nonNullAssert(dart.refer(p.name)).code);
  return dart.Constructor((b) => b
    ..requiredParameters.addAll(dartParameters)
    ..initializers.addAll(initializers));
}

dart.Constructor _compileWithDefaultsPublic(
  QueryContext context,
  String className,
  List<ValueParameter> parameters,
) {
  final publicParameters = parameters.map((p) => dart.Parameter((b) => b
    ..type = compileType(context, p.type)
    ..name = p.name));
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
        .call(parameters.map((p) => dart.refer(p.name)), {}, []).returned);
  });
  return dart.Constructor((b) => b
    ..factory = true
    ..requiredParameters.addAll(publicParameters)
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
