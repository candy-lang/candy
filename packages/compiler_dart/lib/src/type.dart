import 'package:code_builder/code_builder.dart' as dart;
import 'package:compiler/compiler.dart';

import 'constants.dart';
import 'declarations/declaration.dart';
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
        if (type == CandyType.never) return _createType('dynamic');

        final declarationId =
            moduleIdToDeclarationId(context, type.virtualModuleId);
        assert(
          declarationId.isTrait || declarationId.isClass,
          'Expected a trait or class, got: $declarationId.',
        );

        final reference = compileTypeName(context, declarationId);
        return _createType(
          reference.symbol,
          url: reference.url,
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
      meta: (type) {
        final url = moduleIdToImportUrl(context, ModuleId.coreReflection);
        return dart.refer('Type', url);
      },
      reflection: (type) {
        final url = moduleIdToImportUrl(context, ModuleId.coreReflection);
        final id = type.declarationId;
        if (id.isModule) {
          return dart.refer('Module', url);
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
