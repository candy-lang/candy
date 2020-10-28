import 'package:code_builder/code_builder.dart' as dart;
import 'package:compiler/compiler.dart';

import 'constants.dart';
import 'declarations/module.dart';

final Query<CandyType, dart.Reference> compileType =
    Query<CandyType, dart.Reference>(
  'dart.compileType',
  evaluateAlways: true,
  provider: (context, type) {
    dart.Reference compile(CandyType type) => compileType(context, type);

    return type.map(
      this_: (_) => _createType('dynamic'),
      user: (type) {
        if (type == CandyType.any) return _createType('Object');
        if (type == CandyType.unit) return _createType('void', url: null);
        if (type == CandyType.never) return _createType('dynamic');
        if (type == CandyType.bool) return _createType('bool');
        if (type == CandyType.number) return _createType('Num');
        if (type == CandyType.int) return _createType('int');
        if (type == CandyType.float) return _createType('double');
        if (type == CandyType.string) return _createType('String');
        if (type.virtualModuleId == CandyType.arrayModuleId) {
          assert(type.arguments.length == 1);
          return _createType(
            'List',
            typeArguments: [compileType(context, type.arguments.single)],
          );
        }

        return _createType(
          type.name,
          url: moduleIdToImportUrl(context, type.parentModuleId),
          typeArguments:
              type.arguments.map((a) => compileType(context, a)).toList(),
        );
      },
      tuple: (type) {
        final url = moduleIdToImportUrl(context, ModuleId.corePrimitives);
        return dart.TypeReference((b) => b
          ..symbol = 'Tuple${type.items.length}'
          ..url = url
          ..types.addAll(type.items.map((i) => compileType(context, i)))
          ..isNullable = false);
      },
      function: (type) {
        return dart.FunctionType((b) {
          if (type.receiverType != null) {
            b.requiredParameters.add(compile(type.receiverType));
          }
          b
            ..requiredParameters.addAll(type.parameterTypes.map(compile))
            ..returnType = compile(type.returnType);
        });
      },
      union: (_) => _createType('dynamic'),
      intersection: (_) => _createType('dynamic'),
      parameter: (type) => _createType(type.name, url: null),
      reflection: (type) {
        final url = moduleIdToImportUrl(context, ModuleId.coreReflection);
        final id = type.declarationId;
        if (id.isModule) {
          return dart.refer('Module', url);
        } else if (id.isTrait || id.isClass) {
          return dart.refer('Type', url);
        } else if (id.isProperty) {
          final propertyHir = getPropertyDeclarationHir(context, id);
          assert(!propertyHir.isStatic);
          return compileType(
            context,
            CandyType.function(
              receiverType:
                  getPropertyDeclarationParentAsType(context, id).value,
              returnType: propertyHir.type,
            ),
          );
        } else if (id.isFunction) {
          final functionHir = getFunctionDeclarationHir(context, id);
          assert(!functionHir.isStatic);
          return compileType(
            context,
            functionHir.functionType.copyWith(
              receiverType:
                  getPropertyDeclarationParentAsType(context, id).value,
            ),
          );
        } else {
          throw CompilerError.internalError(
            'Invalid reflection target for compiling type: `$id`.',
          );
        }
      },
    );
  },
);

dart.Reference _createType(
  String name, {
  String url = dartCoreUrl,
  List<dart.Reference> typeArguments = const [],
}) {
  return dart.TypeReference((b) => b
    ..symbol = name
    ..url = url
    ..types.addAll(typeArguments)
    ..isNullable = false);
}
