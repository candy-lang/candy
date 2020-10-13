import 'package:code_builder/code_builder.dart' as dart;
import 'package:compiler/compiler.dart';

import 'constants.dart';
import 'declarations/module.dart';

final compileType = Query<CandyType, dart.TypeReference>(
  'dart.compileType',
  evaluateAlways: true,
  provider: (context, type) {
    return type.map(
      user: (type) {
        if (type == CandyType.any) return _createDartType('Object');
        if (type == CandyType.unit) return _createDartType('void', url: null);
        if (type == CandyType.nothing) return _createDartType('dynamic');
        if (type == CandyType.bool) return _createDartType('bool');
        if (type == CandyType.number) return _createDartType('Num');
        if (type == CandyType.int) return _createDartType('int');
        if (type == CandyType.float) return _createDartType('double');
        if (type == CandyType.string) return _createDartType('String');

        return _createDartType(
          type.name,
          url: moduleIdToImportUrl(context, type.moduleId),
        );
      },
      tuple: _unsupportedType,
      function: _unsupportedType,
      union: _unsupportedType,
      intersection: _unsupportedType,
    );
  },
);

dart.TypeReference _createDartType(
  String name, {
  String url = dartCoreUrl,
  List<dart.TypeReference> typeArguments = const [],
}) {
  return dart.TypeReference((b) => b
    ..symbol = name
    ..url = url
    ..types.addAll(typeArguments)
    ..isNullable = false);
}

dart.TypeReference _unsupportedType(CandyType type) {
  throw CompilerError.unsupportedFeature(
    'Compiling type `$type` to Dart is not yet supported.',
  );
}
