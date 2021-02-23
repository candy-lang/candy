import 'package:code_builder/code_builder.dart' as dart;
import 'package:compiler/compiler.dart';
import 'package:dartx/dartx.dart';
import 'package:parser/parser.dart' hide FunctionDeclaration;

import 'body.dart';
import 'constants.dart' hide srcDirectoryName;
import 'declarations/declaration.dart';
import 'declarations/function.dart';
import 'declarations/module.dart';
import 'type.dart';

final compileBuiltin = Query<DeclarationId, List<dart.Spec>>(
  'dart.compileBuiltin',
  provider: (context, declarationId) =>
      DartBuiltinCompiler(context).compile(context, declarationId),
);

abstract class BuiltinCompiler<Output> {
  const BuiltinCompiler();

  List<Output> compile(QueryContext context, DeclarationId declarationId) {
    if (declarationId.isImpl) return [];

    final moduleId = declarationIdToModuleId(context, declarationId);
    final name = declarationId.simplePath.last.nameOrNull;

    if (moduleId == ModuleId.coreAssert) {
      if (name == 'assert') return compileAssert();
    } else if (moduleId ==
        ModuleId.coreCollections.nested(['array', 'Array'])) {
      return compileArray(declarationId);
    } else if (moduleId == ModuleId.corePrimitives.nested(['Any'])) {
      return compileAny();
    } else if (moduleId == ModuleId.corePrimitives.nested(['ToString'])) {
      return compileToString();
    } else if (moduleId == ModuleId.corePrimitives.nested(['Unit'])) {
      return compileUnit(declarationId);
    } else if (moduleId == ModuleId.corePrimitives.nested(['Never'])) {
      return compileNever();
    } else if (moduleId == ModuleId.coreBool.nested(['Bool'])) {
      return compileBool(declarationId);
    } else if (moduleId == ModuleId.coreNumbersInt.nested(['Int'])) {
      return compileInt(declarationId);
    } else if (moduleId == ModuleId.coreString.nested(['String'])) {
      return compileString(declarationId);
    } else if (moduleId == CandyType.directory.virtualModuleId) {
      return compileDirectory(declarationId);
    } else if (moduleId == CandyType.file.virtualModuleId) {
      return compileFile(declarationId);
    } else if (moduleId == CandyType.path.virtualModuleId) {
      return compilePath(declarationId);
    } else if (moduleId == ModuleId.coreIoPrint && name == 'print') {
      return compilePrint();
    } else if (moduleId == CandyType.process.virtualModuleId) {
      return compileProcess(declarationId);
    } else if (moduleId ==
        ModuleId.coreRandomSource.nested(['DefaultRandomSource'])) {
      return compileDefaultRandomSource();
    }

    final declaration = getDeclarationAst(context, declarationId);
    throw CompilerError.internalError(
      'Unknown built-in declaration: `$declarationId` from module $moduleId.',
      location: ErrorLocation(declarationId.resourceId, declaration.span),
    );
  }

  List<Output> compilePrimitiveGhosts() {
    return 2.rangeTo(10).map(compileTuple).flatten().toList();
  }

  // assert
  List<Output> compileAssert();

  // collections
  // collections.list
  // collections.list.array
  List<Output> compileArray(DeclarationId id);

  // io
  // io.file
  List<Output> compileDirectory(DeclarationId id);
  List<Output> compileFile(DeclarationId id);
  List<Output> compilePath(DeclarationId id);
  // io.print
  List<Output> compilePrint();
  // io.process
  List<Output> compileProcess(DeclarationId id);

  // primitives
  List<Output> compileAny();
  List<Output> compileToString();

  List<Output> compileUnit(DeclarationId id);
  List<Output> compileNever();

  List<Output> compileBool(DeclarationId id);

  List<Output> compileInt(DeclarationId id);

  List<Output> compileString(DeclarationId id);

  List<Output> compileTuple(int size);

  // random.source
  List<Output> compileDefaultRandomSource();
}

class DartBuiltinCompiler extends BuiltinCompiler<dart.Spec> {
  const DartBuiltinCompiler(this.context) : assert(context != null);

  final QueryContext context;

  @override
  List<dart.Spec> compileAssert() {
    return [
      dart.Method((b) => b
        ..returns = compileType(context, CandyType.unit)
        ..name = 'assert_'
        ..requiredParameters.add(dart.Parameter((b) => b
          ..name = 'condition'
          ..type = compileType(context, CandyType.bool)))
        ..requiredParameters.add(dart.Parameter((b) => b
          ..name = 'message'
          ..type = compileType(context, CandyType.string)))
        ..body = dart.Block((b) => b
          ..statements.addAll([
            dart.InvokeExpression.newOf(
              dart.refer('assert'),
              [
                dart.refer('condition').property('value'),
                dart.refer('message').property('value'),
              ],
              {},
              [],
            ).statement,
            compileType(context, CandyType.unit)
                .call([], {}, [])
                .returned
                .statement,
          ]))),
    ];
  }

  @override
  List<dart.Spec> compileArray(DeclarationId id) {
    final impls = getAllImplsForTraitOrClassOrImpl(context, id)
        .map((it) => getImplDeclarationHir(context, it));
    final traits = impls.expand((impl) => impl.traits);
    final implements = traits.map((it) => compileType(context, it));
    final implMethodIds = impls
        .expand((impl) => impl.innerDeclarationIds)
        .where((id) => id.isFunction)
        .toList();
    final methodOverrides = implMethodIds
        .map((it) => Tuple2(it, getFunctionDeclarationHir(context, it)))
        .expand((values) sync* {
      final id = values.first;
      final function = values.second;

      if (function.isStatic) {
        throw CompilerError.unsupportedFeature(
          'Static functions in impls are not yet supported.',
          location: ErrorLocation(
            id.resourceId,
            getPropertyDeclarationAst(context, id)
                .modifiers
                .firstWhere((w) => w is StaticModifierToken)
                .span,
          ),
        );
      }

      yield dart.Method((b) => b
        ..annotations.add(dart.refer('override', dartCoreUrl))
        ..returns = compileType(context, function.returnType)
        ..name = function.name
        ..types.addAll(function.typeParameters
            .map((it) => compileTypeParameter(context, it)))
        ..requiredParameters
            .addAll(compileParameters(context, function.valueParameters))
        ..body = compileBody(context, id).value);
    });

    final item = dart.refer('Item');
    final arrayItem = dart.refer('Array<Item>');
    final listItem = dart.TypeReference((b) => b
      ..symbol = 'List'
      ..url = dartCoreUrl
      ..types.add(item));
    return [
      dart.Class((b) => b
        ..annotations.add(dart.refer('sealed', packageMetaUrl))
        ..name = 'Array'
        ..types.add(item)
        ..fields.add(dart.Field((b) => b
          ..name = 'value'
          ..type = listItem))
        ..mixins.addAll(traits.map((it) {
          final type = compileType(context, it);
          return dart.TypeReference((b) => b
            ..symbol = '${type.symbol}\$Default'
            ..types.addAll(it.arguments.map((it) => compileType(context, it)))
            ..url = type.url);
        }))
        ..implements.addAll(implements)
        ..constructors.add(dart.Constructor((b) => b
          ..requiredParameters
              .add(dart.Parameter((b) => b..name = 'this.value'))))
        ..methods.addAll([
          dart.Method((b) => b
            ..name = 'generate'
            ..static = true
            ..types.add(item)
            ..returns = arrayItem
            ..requiredParameters.addAll([
              dart.Parameter((b) => b
                ..name = 'length'
                ..type = compileType(context, CandyType.int)),
              dart.Parameter((b) => b
                ..name = 'generator'
                ..type = dart.FunctionType((b) => b
                  ..requiredParameters.add(compileType(context, CandyType.int))
                  ..returnType = item)),
            ])
            ..body = arrayItem.call([
              listItem.property('generate').call([
                dart.refer('length.value'),
                // The Dart code generator doesn't support lambdas, so we do an ugly workaround.
                dart.Method((b) => b
                  ..requiredParameters
                      .add(dart.Parameter((b) => b..name = 'index'))
                  ..body = dart.refer('generator').call([
                    dart.refer('index').wrapInCandyInt(context),
                  ]).code).closure,
              ]),
            ]).code),
          dart.Method((b) => b
            ..name = 'length'
            ..returns = compileType(context, CandyType.int)
            ..body = dart.refer('value.length').wrapInCandyInt(context).code),
          dart.Method((b) => b
            ..name = 'get'
            ..requiredParameters.add(dart.Parameter((b) => b
              ..name = 'index'
              ..type = compileType(context, CandyType.int)))
            ..returns = item
            ..body = dart.refer('value').index(dart.refer('index.value')).code),
          dart.Method((b) => b
            ..name = 'set'
            ..requiredParameters.addAll([
              dart.Parameter((b) => b
                ..name = 'index'
                ..type = compileType(context, CandyType.int)),
              dart.Parameter((b) => b
                ..name = 'item'
                ..type = item),
            ])
            ..returns = item
            ..body = dart
                .refer('value')
                .index(dart.refer('index.value'))
                .assign(dart.refer('item'))
                .code),
          dart.Method((b) => b
            ..name = 'toString'
            ..returns = dart.refer('String', dartCoreUrl)
            ..body = dart.refer('value').property('toString').call([]).code)
        ])
        ..methods.addAll(getClassDeclarationHir(context, id)
            .innerDeclarationIds
            .where((it) => it.getHir(context) is FunctionDeclaration)
            .where((it) => getBody(context, it).isSome)
            .map((it) {
          final function = getFunctionDeclarationHir(context, it);
          return dart.Method((b) => b
            ..returns = compileType(context, function.returnType)
            ..static = function.isStatic
            ..name = function.name
            ..types.addAll(function.typeParameters
                .map((it) => compileTypeParameter(context, it)))
            ..requiredParameters
                .addAll(compileParameters(context, function.valueParameters))
            ..body = compileBody(context, it).value);
        }))
        ..methods.addAll(methodOverrides)),
    ];
  }

  @override
  List<dart.Spec> compileDirectory(DeclarationId id) {
    final mixinsAndImplementsAndMethodOverrides =
        _prepareMixinsAndImplementsAndMethodOverrides(context, id);

    final bool = compileType(context, CandyType.bool);
    final fileSystemNode = compileType(context, CandyType.fileSystemNode);
    final directory = compileType(context, CandyType.directory);
    final file = compileType(context, CandyType.file);

    final _directory = dart.refer('_directory');
    return [
      dart.Class((b) => b
        ..annotations.add(dart.refer('sealed', packageMetaUrl))
        ..name = 'Directory'
        ..fields.addAll([
          dart.Field((b) => b
            ..annotations.add(dart.refer('override', dartCoreUrl))
            ..name = 'path'
            ..type = dart.refer(
                'Path', moduleIdToImportUrl(context, ModuleId.coreIoFile))),
          dart.Field((b) => b
            ..name = '_directory'
            ..type = dart.refer('Directory', dartIoUrl)),
        ])
        ..mixins.addAll(mixinsAndImplementsAndMethodOverrides.first)
        ..implements.addAll(mixinsAndImplementsAndMethodOverrides.second)
        ..constructors.add(dart.Constructor((b) => b
          ..requiredParameters.add(dart.Parameter((b) => b..name = 'this.path'))
          ..initializers.addAll([
            dart.refer('assert').call(
                [dart.refer('path').notEqualTo(dart.literalNull)], {}, []).code,
            _directory
                .assign(dart
                    .refer('Directory', dartIoUrl)
                    .call([dart.refer('path.value.value')], {}, []))
                .code,
          ])))
        ..methods.addAll([
          dart.Method((b) => b
            ..annotations.add(dart.refer('override', dartCoreUrl))
            ..returns = bool
            ..name = 'doesExist'
            ..body = _directory
                .property('existsSync')
                .call([], {}, [])
                .wrapInCandyBool(context)
                .code),
          dart.Method((b) => b
            ..returns = compileType(context, CandyType.unit)
            ..name = 'create'
            ..requiredParameters.add(dart.Parameter((b) => b
              ..name = 'recursive'
              ..type = bool))
            ..body = dart.Block((b) => b
              ..statements.addAll([
                _directory.property('createSync').call(
                    [],
                    {'recursive': dart.refer('recursive').property('value')},
                    []).statement,
                compileType(context, CandyType.unit)
                    .call([], {}, [])
                    .returned
                    .statement,
              ]))),
          dart.Method((b) => b
            ..returns = compileType(context, CandyType.unit)
            ..name = 'delete'
            ..requiredParameters.add(dart.Parameter((b) => b
              ..name = 'recursive'
              ..type = bool))
            ..body = dart.Block((b) => b
              ..statements.addAll([
                _directory.property('deleteSync').call(
                    [],
                    {'recursive': dart.refer('recursive').property('value')},
                    []).statement,
                compileType(context, CandyType.unit)
                    .call([], {}, [])
                    .returned
                    .statement,
              ]))),
          dart.Method((b) => b
            ..returns =
                compileType(context, CandyType.list(CandyType.fileSystemNode))
            ..name = 'listContents'
            ..requiredParameters.add(dart.Parameter((b) => b
              ..name = 'recursive'
              ..type = bool))
            ..body = dart.Block((b) => b
              ..statements.addAll([
                _directory
                    .property('listSync')
                    .call(
                      [],
                      {'recursive': dart.refer('recursive').property('value')},
                      [],
                    )
                    .property('map')
                    .call(
                      [
                        dart.Method((b) => b
                          ..requiredParameters.add(dart.Parameter((b) => b
                            ..type = dart.refer('FileSystemEntity', dartIoUrl)
                            ..name = 'it'))
                          ..body = dart.Block((b) {
                            dart.Code checkAndConvertTo(
                              dart.Reference type,
                              String name,
                            ) {
                              final pathArgument = dart
                                  .refer('it')
                                  .property('absolute')
                                  .property('path')
                                  .wrapInCandyString(context);
                              return dart.CodeExpression(dart.Block.of([
                                dart.Code('if ('),
                                dart
                                    .refer('it')
                                    .isA(dart.refer(name, dartIoUrl))
                                    .code,
                                dart.Code(') {'),
                                type
                                    .call(
                                      [pathArgument.wrapInCandyPath(context)],
                                      {},
                                      [],
                                    )
                                    .returned
                                    .statement,
                                dart.Code('}'),
                              ])).code;
                            }

                            b.statements.addAll([
                              dart.CodeExpression(dart.Block.of([
                                checkAndConvertTo(directory, 'Directory'),
                                checkAndConvertTo(file, 'File'),
                              ])).code,
                              dart.literalNull.returned.statement,
                            ]);
                          })).closure,
                      ],
                      {},
                      [fileSystemNode],
                    )
                    .property('where')
                    .call([
                      dart.Method((b) => b
                        ..requiredParameters.add(dart.Parameter((b) => b
                          ..type = fileSystemNode
                          ..name = 'it'))
                        ..body = dart
                            .refer('it')
                            .notEqualTo(dart.literalNull)
                            .code).closure,
                    ], {}, [])
                    .property('toList')
                    .call([], {}, [])
                    .wrapInCandyArray(context, CandyType.fileSystemNode)
                    .assignFinal('contents')
                    .statement,
                dart
                    .refer(
                        'ArrayList',
                        moduleIdToImportUrl(
                            context, ModuleId.coreCollectionsListArrayList))
                    .property('fromArray')
                    .call([dart.refer('contents')], {}, [fileSystemNode])
                    .returned
                    .statement,
              ]))),
          dart.Method((b) => b
            ..returns = bool
            ..name = 'equals'
            ..requiredParameters.add(dart.Parameter((b) => b
              ..name = 'other'
              ..type = dart.refer('dynamic', dartCoreUrl)))
            ..body = dart
                .refer('path')
                .equalToCandyValue(dart.refer('other.path'))
                .code),
          dart.Method((b) => b
            ..returns = dart.refer('String', dartCoreUrl)
            ..name = 'toString'
            ..body = dart.literalString('Directory(\${path.toString()})').code)
        ])
        ..methods.addAll(mixinsAndImplementsAndMethodOverrides.third)),
    ];
  }

  @override
  List<dart.Spec> compileFile(DeclarationId id) {
    final mixinsAndImplementsAndMethodOverrides =
        _prepareMixinsAndImplementsAndMethodOverrides(context, id);

    final bool = compileType(context, CandyType.bool);
    final string = compileType(context, CandyType.string);

    final _file = dart.refer('_file');
    return [
      dart.Class((b) => b
        ..annotations.add(dart.refer('sealed', packageMetaUrl))
        ..name = 'File'
        ..fields.addAll([
          dart.Field((b) => b
            ..annotations.add(dart.refer('override', dartCoreUrl))
            ..name = 'path'
            ..type = dart.refer(
                'Path', moduleIdToImportUrl(context, ModuleId.coreIoFile))),
          dart.Field((b) => b
            ..name = '_file'
            ..type = dart.refer('File', dartIoUrl)),
        ])
        ..mixins.addAll(mixinsAndImplementsAndMethodOverrides.first)
        ..implements.addAll(mixinsAndImplementsAndMethodOverrides.second)
        ..constructors.add(dart.Constructor((b) => b
          ..requiredParameters.add(dart.Parameter((b) => b..name = 'this.path'))
          ..initializers.addAll([
            dart.refer('assert').call(
                [dart.refer('path').notEqualTo(dart.literalNull)], {}, []).code,
            _file
                .assign(dart
                    .refer('File', dartIoUrl)
                    .call([dart.refer('path.value.value')]))
                .code,
          ])))
        ..methods.addAll([
          dart.Method((b) => b
            ..annotations.add(dart.refer('override', dartCoreUrl))
            ..returns = bool
            ..name = 'doesExist'
            ..body = _file
                .property('existsSync')
                .call([], {}, [])
                .wrapInCandyBool(context)
                .code),
          dart.Method((b) => b
            ..annotations.add(dart.refer('override', dartCoreUrl))
            ..returns = compileType(context, CandyType.unit)
            ..name = 'create'
            ..requiredParameters.add(dart.Parameter((b) => b
              ..name = 'recursive'
              ..type = bool))
            ..body = dart.Block((b) => b
              ..statements.addAll([
                _file.property('createSync').call(
                  [],
                  {'recursive': dart.refer('recursive').property('value')},
                  [],
                ).statement,
                compileType(context, CandyType.unit)
                    .call([], {}, [])
                    .returned
                    .statement,
              ]))),
          dart.Method((b) => b
            ..returns = string
            ..name = 'read'
            ..body = _file
                .property('readAsStringSync')
                .call([], {}, [])
                .wrapInCandyString(context)
                .code),
          dart.Method((b) => b
            ..returns = compileType(context, CandyType.unit)
            ..name = 'write'
            ..requiredParameters.add(dart.Parameter((b) => b
              ..name = 'content'
              ..type = string))
            ..body = dart.Block((b) => b
              ..statements.addAll([
                _file.property('writeAsStringSync').call(
                  [dart.refer('content').property('value')],
                  {},
                  [],
                ).statement,
                compileType(context, CandyType.unit)
                    .call([], {}, [])
                    .returned
                    .statement,
              ]))),
          dart.Method((b) => b
            ..returns = bool
            ..name = 'equals'
            ..requiredParameters.add(dart.Parameter((b) => b
              ..name = 'other'
              ..type = dart.refer('dynamic', dartCoreUrl)))
            ..body = dart
                .refer('path')
                .equalToCandyValue(dart.refer('other.path'))
                .code),
          dart.Method((b) => b
            ..returns = dart.refer('String', dartCoreUrl)
            ..name = 'toString'
            ..body = dart.literalString('File(\${path.toString()})').code)
        ])
        ..methods.addAll(mixinsAndImplementsAndMethodOverrides.third)),
    ];
  }

  @override
  List<dart.Spec> compilePath(DeclarationId id) {
    final mixinsAndImplementsAndMethodOverrides =
        _prepareMixinsAndImplementsAndMethodOverrides(context, id);

    final bool = compileType(context, CandyType.bool);
    final string = compileType(context, CandyType.string);
    final path = compileType(context, CandyType.path);
    final t = dart.refer('T');

    return [
      dart.Class((b) => b
        ..annotations.add(dart.refer('sealed', packageMetaUrl))
        ..name = 'Path'
        ..fields.add(dart.Field((b) => b
          ..name = 'value'
          ..type = string))
        ..mixins.addAll(mixinsAndImplementsAndMethodOverrides.first)
        ..implements.addAll(mixinsAndImplementsAndMethodOverrides.second)
        ..constructors.add(dart.Constructor((b) => b
          ..requiredParameters
              .add(dart.Parameter((b) => b..name = 'this.value'))
          ..initializers.add(dart.refer('assert').call(
              [dart.refer('value').notEqualTo(dart.literalNull)],
              {},
              []).code)))
        ..methods.addAll([
          dart.Method((b) => b
            ..static = true
            ..returns = path
            ..name = 'current'
            ..body = dart
                .refer('current', packagePathUrl)
                .wrapInCandyString(context)
                .wrapInCandyPath(context)
                .code),
          dart.Method((b) => b
            ..static = true
            ..returns = path
            ..name = 'parse'
            ..requiredParameters.add(dart.Parameter((b) => b
              ..name = 'path'
              ..type = string))
            ..body = dart.refer('path').wrapInCandyPath(context).code),
          dart.Method((b) => b
            ..returns = path
            ..name = 'normalized'
            ..body = dart
                .refer('normalize', packagePathUrl)
                .call([dart.refer('value.value')], {}, [])
                .wrapInCandyString(context)
                .wrapInCandyPath(context)
                .code),
          dart.Method((b) => b
            ..returns = bool
            ..name = 'isAbsolute'
            ..body = dart
                .refer('isAbsolute', packagePathUrl)
                .call([dart.refer('value.value')])
                .wrapInCandyBool(context)
                .code),
          dart.Method((b) => b
            ..returns = compileType(context, CandyType.maybe(CandyType.path))
            ..name = 'parent'
            ..body = dart.Block((b) => b
              ..statements.addAll([
                dart.CodeExpression(dart.Block.of([
                  dart.Code('if ('),
                  dart
                      .refer('split', packagePathUrl)
                      .call([
                        dart
                            .refer('normalize', packagePathUrl)
                            .call([dart.refer('value.value')]),
                      ])
                      .property('length')
                      .equalTo(dart.literalNum(1))
                      .code,
                  dart.Code(') {'),
                  compileType(context, CandyType.none(CandyType.path))
                      .call([])
                      .returned
                      .statement,
                  dart.Code('} else {'),
                  compileType(context, CandyType.some(CandyType.path))
                      .call([
                        path.call([
                          dart.refer('dirname', packagePathUrl).call([
                            dart.refer('value.value')
                          ]).wrapInCandyString(context),
                        ]),
                      ])
                      .returned
                      .statement,
                  dart.Code('}'),
                ])).code,
              ]))),
          dart.Method((b) => b
            ..returns = path
            ..name = 'child'
            ..requiredParameters.add(dart.Parameter((b) => b
              ..name = 'name'
              ..type = string))
            ..body = path.call([
              dart.refer('join', packagePathUrl).call([
                dart.refer('value.value'),
                dart.refer('name.value'),
              ]).wrapInCandyString(context),
            ]).code),
          dart.Method((b) => b
            ..returns = path
            ..name = 'append'
            ..requiredParameters.add(dart.Parameter((b) => b
              ..name = 'other'
              ..type = path))
            ..body = path.call([
              dart.refer('join', packagePathUrl).call([
                dart.refer('value.value'),
                dart.refer('other.value.value'),
              ]).wrapInCandyString(context),
            ]).code),
          dart.Method(
            (b) => b
              ..returns = string
              ..name = 'baseName'
              ..body = dart
                  .refer('basename', packagePathUrl)
                  .call([dart.refer('value.value')])
                  .wrapInCandyString(context)
                  .code,
          ),
          dart.Method(
            (b) => b
              ..returns = string
              ..name = 'baseNameWithoutExtension'
              ..body = dart
                  .refer('basenameWithoutExtension', packagePathUrl)
                  .call([dart.refer('value.value')])
                  .wrapInCandyString(context)
                  .code,
          ),
          dart.Method((b) => b
            ..returns = bool
            ..name = 'equals'
            ..requiredParameters.add(dart.Parameter((b) => b
              ..name = 'other'
              ..type = dart.refer('dynamic', dartCoreUrl)))
            ..body = dart
                .refer('value')
                .equalToCandyValue(dart.refer('other.value'))
                .code),
          dart.Method((b) => b
            ..returns = compileType(context, CandyType.unit)
            ..name = 'hash'
            ..types.add(t)
            ..requiredParameters.add(dart.Parameter((b) => b
              ..name = 'hasher'
              ..type = compileType(
                context,
                CandyType.hasher(CandyType.parameter('T', id)),
              )))
            ..body = dart.Block((b) => b
              ..statements.addAll([
                dart
                    .refer('value')
                    .property('hash')
                    .call([dart.refer('hasher')]).statement,
                compileType(context, CandyType.unit).call([]).returned.statement
              ]))),
          dart.Method((b) => b
            ..returns = dart.refer('String', dartCoreUrl)
            ..name = 'toString'
            ..body = dart.literalString('\${value.toString()}').code)
        ])
        ..methods.addAll(mixinsAndImplementsAndMethodOverrides.third)),
    ];
  }

  @override
  List<dart.Spec> compileProcess(DeclarationId id) {
    final mixinsAndImplementsAndMethodOverrides =
        _prepareMixinsAndImplementsAndMethodOverrides(context, id);

    final processResult = compileType(context, CandyType.processResult);
    final path = compileType(context, CandyType.path);

    return [
      dart.Class((b) => b
        ..annotations.add(dart.refer('sealed', packageMetaUrl))
        ..name = 'Process'
        ..mixins.addAll(mixinsAndImplementsAndMethodOverrides.first)
        ..implements.addAll(mixinsAndImplementsAndMethodOverrides.second)
        ..constructors.add(dart.Constructor((b) => b..name = '_'))
        ..methods.addAll([
          dart.Method((b) => b
            ..static = true
            ..returns = processResult
            ..name = 'run'
            ..requiredParameters.addAll([
              dart.Parameter((b) => b
                ..type = path
                ..name = 'executable'),
              dart.Parameter((b) => b
                ..type = compileType(context, CandyType.list(CandyType.string))
                ..name = 'arguments'),
              dart.Parameter((b) => b
                ..type = path
                ..name = 'workingDirectory'),
            ])
            ..body = dart.Block((b) => b
              ..statements.addAll([
                dart
                    .refer('Process', dartIoUrl)
                    .property('runSync')
                    .call([
                      dart.refer('executable.value.value'),
                      dart
                          .refer('List', dartCoreUrl)
                          .property('generate')
                          .call([
                        dart.refer('arguments.length().value'),
                        dart.Method((b) => b
                          ..requiredParameters
                              .add(dart.Parameter((b) => b..name = 'it'))
                          ..body = dart
                              .refer('arguments.get')
                              .call([
                                dart.refer('it').wrapInCandyInt(context),
                              ])
                              .property('unwrap().value')
                              .code).closure,
                      ]),
                    ], {
                      'workingDirectory':
                          dart.refer('workingDirectory.value.value'),
                    })
                    .assignFinal('result')
                    .statement,
                processResult
                    .call([
                      dart.refer('result.exitCode').wrapInCandyInt(context),
                      dart.refer('result.pid').wrapInCandyInt(context),
                      dart
                          .refer('result.stdout')
                          .asA(dart.refer('String', dartCoreUrl))
                          .wrapInCandyString(context),
                      dart
                          .refer('result.stderr')
                          .asA(dart.refer('String', dartCoreUrl))
                          .wrapInCandyString(context),
                    ])
                    .returned
                    .statement,
              ]))),
        ])
        ..methods.addAll(mixinsAndImplementsAndMethodOverrides.third)),
    ];
  }

  @override
  List<dart.Spec> compileAny() {
    // `Any` corresponds to `Object`, hence nothing to do.
    return [];
  }

  @override
  List<dart.Spec> compileToString() {
    // `ToString` is given by Dart's `Object`, hence nothing to do.
    return [];
  }

  @override
  List<dart.Spec> compileUnit(DeclarationId id) {
    final mixinsAndImplementsAndMethodOverrides =
        _prepareMixinsAndImplementsAndMethodOverrides(context, id);

    return [
      dart.Class((b) => b
        ..annotations.add(dart.refer('sealed', packageMetaUrl))
        ..name = 'Unit'
        ..constructors.add(dart.Constructor((b) => b..constant = true))
        ..mixins.addAll(mixinsAndImplementsAndMethodOverrides.first)
        ..implements.addAll(mixinsAndImplementsAndMethodOverrides.second)
        ..methods.addAll([
          dart.Method((b) => b
            ..name = 'toString'
            ..returns = dart.refer('String', dartCoreUrl)
            ..body = dart.literalString('"unit"').code)
        ])
        ..methods.addAll(mixinsAndImplementsAndMethodOverrides.third)),
    ];
  }

  @override
  List<dart.Spec> compileNever() {
    return [dart.Class((b) => b..name = 'Never')];
  }

  @override
  List<dart.Spec> compileBool(DeclarationId id) {
    final mixinsAndImplementsAndMethodOverrides =
        _prepareMixinsAndImplementsAndMethodOverrides(context, id);

    final otherBool = dart.Parameter((b) => b
      ..name = 'other'
      ..type = dart.refer('dynamic', dartCoreUrl));
    return [
      dart.Class((b) => b
        ..annotations.add(dart.refer('sealed', packageMetaUrl))
        ..name = 'Bool'
        ..fields.add(dart.Field((b) => b
          ..name = 'value'
          ..type = dart.refer('bool', dartCoreUrl)))
        ..mixins.addAll(mixinsAndImplementsAndMethodOverrides.first)
        ..implements.addAll(mixinsAndImplementsAndMethodOverrides.second)
        ..constructors.add(dart.Constructor((b) => b
          ..requiredParameters
              .add(dart.Parameter((b) => b..name = 'this.value'))))
        ..methods.addAll([
          dart.Method((b) => b
            ..name = 'equals'
            ..returns = compileType(context, CandyType.bool)
            ..requiredParameters.add(otherBool)
            ..body = dart
                .refer('value')
                .equalTo(dart.refer('other.value'))
                .wrapInCandyBool(context)
                .code),
          dart.Method((b) => b
            ..name = 'and'
            ..returns = compileType(context, CandyType.bool)
            ..requiredParameters.add(otherBool)
            ..body = dart
                .refer('value')
                .and(dart.refer('other.value'))
                .wrapInCandyBool(context)
                .code),
          dart.Method((b) => b
            ..name = 'or'
            ..returns = compileType(context, CandyType.bool)
            ..requiredParameters.add(otherBool)
            ..body = dart
                .refer('value')
                .or(dart.refer('other.value'))
                .wrapInCandyBool(context)
                .code),
          dart.Method((b) => b
            ..name = 'opposite'
            ..returns = compileType(context, CandyType.bool)
            ..body =
                dart.refer('value').negate().wrapInCandyBool(context).code),
          dart.Method((b) => b
            ..name = 'implies'
            ..returns = compileType(context, CandyType.bool)
            ..requiredParameters.add(otherBool)
            ..body = dart
                .refer('value')
                .negate()
                .or(dart.refer('other.value'))
                .wrapInCandyBool(context)
                .code),
          dart.Method((b) => b
            ..name = 'toString'
            ..returns = dart.refer('String', dartCoreUrl)
            ..body = dart.refer('value').property('toString').call([]).code)
        ])
        ..methods.addAll(mixinsAndImplementsAndMethodOverrides.third)),
    ];
  }

  @override
  List<dart.Spec> compileInt(DeclarationId id) {
    final mixinsAndImplementsAndMethodOverrides =
        _prepareMixinsAndImplementsAndMethodOverrides(context, id);

    final otherInt = dart.Parameter((b) => b
      ..name = 'other'
      ..type = dart.refer('dynamic', dartCoreUrl));
    return [
      dart.Class((b) => b
        ..annotations.add(dart.refer('sealed', packageMetaUrl))
        ..name = 'Int'
        ..fields.add(dart.Field((b) => b
          ..name = 'value'
          ..type = dart.refer('int', dartCoreUrl)))
        ..mixins.addAll(mixinsAndImplementsAndMethodOverrides.first)
        ..implements.addAll(mixinsAndImplementsAndMethodOverrides.second)
        ..constructors.add(dart.Constructor((b) => b
          ..requiredParameters
              .add(dart.Parameter((b) => b..name = 'this.value'))))
        ..methods.addAll([
          dart.Method((b) => b
            ..name = 'equals'
            ..returns = compileType(context, CandyType.bool)
            ..requiredParameters.add(otherInt)
            ..body = dart
                .refer('value')
                .equalTo(dart.refer('other.value'))
                .wrapInCandyBool(context)
                .code),
          dart.Method((b) => b
            ..name = 'compareTo'
            ..returns = compileType(
              context,
              CandyType.union([
                CandyType.comparableLess,
                CandyType.comparableEqual,
                CandyType.comparableGreater,
              ]),
            )
            ..requiredParameters.add(otherInt)
            ..body = dart
                .refer('value.compareTo')
                .call([dart.refer('other.value')])
                .toComparisonResult(context)
                .code),
          dart.Method((b) => b
            ..name = 'add'
            ..returns = compileType(context, CandyType.int)
            ..requiredParameters.add(otherInt)
            ..body = dart
                .refer('value')
                .operatorAdd(dart.refer('other.value'))
                .wrapInCandyInt(context)
                .code),
          dart.Method((b) => b
            ..name = 'subtract'
            ..returns = compileType(context, CandyType.int)
            ..requiredParameters.add(otherInt)
            ..body = dart
                .refer('value')
                .operatorSubstract(dart.refer('other.value'))
                .wrapInCandyInt(context)
                .code),
          dart.Method((b) => b
            ..name = 'negate'
            ..returns = compileType(context, CandyType.int)
            ..body = dart.refer('-value').wrapInCandyInt(context).code),
          dart.Method((b) => b
            ..name = 'multiply'
            ..returns = compileType(context, CandyType.int)
            ..requiredParameters.add(otherInt)
            ..body = dart
                .refer('value')
                .operatorMultiply(dart.refer('other.value'))
                .wrapInCandyInt(context)
                .code),
          dart.Method((b) => b
            ..name = 'divideTruncating'
            ..returns = compileType(context, CandyType.int)
            ..requiredParameters.add(otherInt)
            ..body = dart
                .refer('value ~/ other.value')
                .wrapInCandyInt(context)
                .code),
          dart.Method((b) => b
            ..name = 'modulo'
            ..returns = compileType(context, CandyType.int)
            ..requiredParameters.add(otherInt)
            ..body = dart
                .refer('value')
                .operatorEuclideanModulo(dart.refer('other.value'))
                .wrapInCandyInt(context)
                .code),
          dart.Method((b) => b
            ..name = 'toString'
            ..returns = dart.refer('String', dartCoreUrl)
            ..body = dart.refer('value').property('toString').call([]).code),
          dart.Method((b) => b
            ..static = true
            ..returns = compileType(context, CandyType.int)
            ..name = 'parse'
            ..requiredParameters.add(dart.Parameter((b) => b
              ..type = compileType(context, CandyType.string)
              ..name = 'value'))
            ..body = compileType(context, CandyType.int).call(
              [
                dart.refer('int', dartCoreUrl).property('parse').call(
                  [dart.refer('value').property('value')],
                  {},
                  [],
                ),
              ],
              {},
              [],
            ).code),
        ])
        ..methods.addAll(mixinsAndImplementsAndMethodOverrides.third)),
    ];
  }

  @override
  List<dart.Spec> compileString(DeclarationId id) {
    final mixinsAndImplementsAndMethodOverrides =
        _prepareMixinsAndImplementsAndMethodOverrides(context, id);

    final bool = compileType(context, CandyType.bool);
    final int = compileType(context, CandyType.int);
    final string = compileType(context, CandyType.string);
    final otherString = dart.Parameter((b) => b
      ..name = 'other'
      ..type = dart.refer('dynamic', dartCoreUrl));
    return [
      dart.Class((b) => b
        ..annotations.add(dart.refer('sealed', packageMetaUrl))
        ..name = 'String'
        ..fields.add(dart.Field((b) => b
          ..name = 'value'
          ..type = dart.refer('String', dartCoreUrl)))
        ..mixins.addAll(mixinsAndImplementsAndMethodOverrides.first)
        ..implements.addAll(mixinsAndImplementsAndMethodOverrides.second)
        ..constructors.add(dart.Constructor((b) => b
          ..requiredParameters
              .add(dart.Parameter((b) => b..name = 'this.value'))))
        ..methods.addAll([
          dart.Method((b) => b
            ..name = 'equals'
            ..returns = compileType(context, CandyType.bool)
            ..requiredParameters.add(otherString)
            ..body = dart
                .refer('value')
                .equalTo(dart.refer('other.value'))
                .wrapInCandyBool(context)
                .code),
          dart.Method((b) => b
            ..name = 'compareTo'
            ..returns = compileType(
              context,
              CandyType.union([
                CandyType.comparableLess,
                CandyType.comparableEqual,
                CandyType.comparableGreater,
              ]),
            )
            ..requiredParameters.add(otherString)
            ..body = dart
                .refer('value.compareTo')
                .call([dart.refer('other.value')])
                .toComparisonResult(context)
                .code),
          dart.Method((b) => b
            ..name = 'characters'
            ..returns =
                compileType(context, CandyType.iterable(CandyType.string))
            ..body = dart
                .refer('value')
                .property('characters')
                .property('map')
                .call([
                  dart.Method((b) => b
                        ..requiredParameters.add(dart.Parameter((b) => b
                          ..name = 'char'
                          ..type = dart.refer('String', dartCoreUrl)))
                        ..body =
                            dart.refer('char').wrapInCandyString(context).code)
                      .closure
                ])
                .property('toList')
                .call([])
                .wrapInCandyArrayList(context, CandyType.string)
                .code),
          dart.Method((b) => b
            ..name = 'substring'
            ..returns = string
            ..requiredParameters.addAll([
              dart.Parameter((b) => b
                ..type = int
                ..name = 'offset'),
              dart.Parameter((b) => b
                ..type = int
                ..name = 'length'),
            ])
            ..body = dart
                .refer('value.substring')
                .call([dart.refer('offset.value'), dart.refer('length.value')])
                .wrapInCandyString(context)
                .code),
          dart.Method((b) => b
            ..returns = bool
            ..name = 'isEmpty'
            ..body = dart.refer('value.isEmpty').wrapInCandyBool(context).code),
          dart.Method((b) => b
            ..returns = bool
            ..name = 'isNotEmpty'
            ..body =
                dart.refer('value.isNotEmpty').wrapInCandyBool(context).code),
          dart.Method((b) => b
            ..returns = int
            ..name = 'length'
            ..body = dart.refer('value.length').wrapInCandyInt(context).code),
          dart.Method((b) => b
            ..returns = compileType(context, CandyType.list(CandyType.string))
            ..name = 'split'
            ..requiredParameters.add(dart.Parameter((b) => b
              ..type = string
              ..name = 'pattern'))
            ..body = dart
                .refer('value')
                .property('split')
                .call([dart.refer('pattern.value')])
                .property('map')
                .call([
                  dart.Method((b) => b
                        ..requiredParameters
                            .add(dart.Parameter((b) => b..name = 'it'))
                        ..body =
                            dart.refer('it').wrapInCandyString(context).code)
                      .closure,
                ])
                .property('toList')
                .call([], {}, [])
                .wrapInCandyArrayList(context, CandyType.string)
                .code),
          dart.Method((b) => b
            ..returns = string
            ..name = 'trimmed'
            ..body = dart
                .refer('value')
                .property('trim')
                .call([], {}, [])
                .wrapInCandyString(context)
                .code),
          dart.Method((b) => b
            ..name = 'toString'
            ..returns = dart.refer('String', dartCoreUrl)
            ..body = dart.literalString('\${value.toString()}').code)
        ])
        ..methods.addAll(mixinsAndImplementsAndMethodOverrides.third)),
    ];
  }

  @override
  List<dart.Spec> compileTuple(int size) {
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

    final name = 'Tuple$size';
    final indices = 1.rangeTo(size);
    final typeParameterNames = indices.map((it) => 'T$it');
    final propertyNames = indices.map((i) => fieldNames[i - 1]);

    return [
      dart.Class((b) => b
        ..annotations.add(dart.refer('sealed', packageMetaUrl))
        ..name = name
        ..types.addAll(typeParameterNames.map(dart.refer))
        ..fields.addAll(
            propertyNames.mapIndexed((index, name) => dart.Field((b) => b
              ..modifier = dart.FieldModifier.final$
              ..type = dart.refer('T${index + 1}')
              ..name = name)))
        ..constructors.add(dart.Constructor((b) => b
          ..constant = true
          ..requiredParameters
              .addAll(propertyNames.map((name) => dart.Parameter((b) => b
                ..toThis = true
                ..name = name)))
          ..initializers.addAll(propertyNames.map((name) => dart
              .refer('assert')
              .call([dart.refer(name).notEqualTo(dart.literalNull)], {},
                  []).code))))
        ..methods.add(dart.Method((b) => b
          ..annotations.add(dart.refer('override', dartCoreUrl))
          ..returns = dart.refer('String', dartCoreUrl)
          ..name = 'toString'
          ..body = dart.Block((b) {
            final typeParametersString =
                typeParameterNames.map((it) => '$it = \${$it}').join(', ');
            final propertiesString =
                propertyNames.map((it) => ', "$it": \${this.$it}').join('');
            b.statements.add(dart
                .literalString(
                  '{"_type": "$name<$typeParametersString>"$propertiesString}',
                )
                .returned
                .statement);
          })))),
    ];
  }

  @override
  List<dart.Spec> compilePrint() {
    return [
      dart.Method((b) => b
        ..returns = compileType(context, CandyType.unit)
        ..name = 'print'
        ..requiredParameters.add(dart.Parameter((b) => b
          ..name = 'object'
          ..type = dart.refer('Object', dartCoreUrl)))
        ..body = dart.Block((b) => b
          ..statements.addAll([
            dart.InvokeExpression.newOf(
              dart.refer('print', dartCoreUrl),
              [dart.refer('object')],
              {},
              [],
            ).statement,
            compileType(context, CandyType.unit)
                .call([], {}, [])
                .returned
                .statement,
          ]))),
    ];
  }

  @override
  List<dart.Spec> compileDefaultRandomSource() {
    final int = compileType(context, CandyType.int);
    final random = dart.refer('Random', dartMathUrl);
    return [
      dart.Class((b) => b
        ..name = 'DefaultRandomSource'
        ..implements.add(compileType(context, CandyType.randomSource))
        ..mixins.add(dart.refer('RandomSource\$Default'))
        ..constructors.add(dart.Constructor((b) => b
          ..optionalParameters.add(dart.Parameter((b) => b
            ..named = false
            ..type = int
            ..name = 'seed'))
          ..initializers.add(dart
              .refer('_random')
              .assign(random.call([dart.refer('seed.value')], {}, []))
              .code)))
        ..methods.add(dart.Method((b) => b
          ..static = true
          ..name = 'withSeed'
          ..requiredParameters.add(dart.Parameter((b) => b
            ..type = int
            ..name = 'seed'))
          ..body = dart.Block((b) => b
            ..statements.add(dart
                .refer('DefaultRandomSource')
                .call([dart.refer('seed')], {}, [])
                .returned
                .statement))))
        ..fields.add(dart.Field((b) => b
          ..modifier = dart.FieldModifier.final$
          ..type = random
          ..name = '_random'))
        ..methods.add(dart.Method((b) => b
          ..annotations.add(dart.refer('override', dartCoreUrl))
          ..returns = compileType(context, CandyType.int)
          ..name = 'generateByte'
          ..body = dart.Block((b) => b
            ..statements.add(dart
                .refer('_random')
                .property('nextInt')
                .call([dart.literalNum(1 << 8)], {}, [])
                .wrapInCandyInt(context)
                .returned
                .statement)))))
    ];
  }

  Tuple3<Iterable<dart.Reference>, Iterable<dart.Reference>,
      Iterable<dart.Method>> _prepareMixinsAndImplementsAndMethodOverrides(
    QueryContext context,
    DeclarationId id,
  ) {
    final impls = getAllImplsForTraitOrClassOrImpl(context, id)
        .map((it) => getImplDeclarationHir(context, it));
    final traits = impls.expand((impl) => impl.traits);
    final mixins = traits.map((it) {
      final type = compileType(context, it);
      return dart.TypeReference((b) => b
        ..symbol = '${type.symbol}\$Default'
        ..types.addAll(it.arguments.map((it) => compileType(context, it)))
        ..url = type.url);
    });

    final implements = traits.map((it) => compileType(context, it));

    final implMethodIds = impls
        .expand((impl) => impl.innerDeclarationIds)
        .where((id) => id.isFunction)
        .toList();
    final methodOverrides = implMethodIds
        .map((it) => Tuple2(it, getFunctionDeclarationHir(context, it)))
        .expand((values) sync* {
      final id = values.first;
      final function = values.second;

      if (function.isStatic) {
        throw CompilerError.unsupportedFeature(
          'Static functions in impls are not yet supported.',
          location: ErrorLocation(
            id.resourceId,
            getPropertyDeclarationAst(context, id)
                .modifiers
                .firstWhere((w) => w is StaticModifierToken)
                .span,
          ),
        );
      }

      yield dart.Method((b) => b
        ..annotations.add(dart.refer('override', dartCoreUrl))
        ..returns = compileType(context, function.returnType)
        ..name = function.name
        ..types.addAll(function.typeParameters
            .map((it) => compileTypeParameter(context, it)))
        ..requiredParameters
            .addAll(compileParameters(context, function.valueParameters))
        ..body = compileBody(context, id).value);
    });

    return Tuple3(mixins, implements, methodOverrides);
  }
}

extension WrappingInCandyTypes on dart.Expression {
  dart.Expression wrapInCandyBool(QueryContext context) {
    return compileType(context, CandyType.bool).call([this]);
  }

  dart.Expression wrapInCandyInt(QueryContext context) {
    return compileType(context, CandyType.int).call([this]);
  }

  dart.Expression wrapInCandyString(QueryContext context) {
    return compileType(context, CandyType.string).call([this]);
  }

  dart.Expression wrapInCandyArray(QueryContext context, CandyType itemType) {
    return compileType(context, CandyType.array(itemType)).call([this]);
  }

  dart.Expression wrapInCandyArrayList(
    QueryContext context,
    CandyType itemType,
  ) {
    return compileTypeName(
      context,
      moduleIdToDeclarationId(context, CandyType.arrayListModuleId),
    ).property('fromArray').call(
      [wrapInCandyArray(context, itemType)],
      {},
      [compileType(context, itemType)],
    );
  }

  dart.Expression wrapInCandyPath(QueryContext context) {
    return compileType(context, CandyType.path).call([this]);
  }

  dart.Expression equalToCandyValue(dart.Expression other) {
    return property('equalsAny').call([other]);
  }

  dart.Expression toComparisonResult(QueryContext context) {
    return dart.Method((b) => b
      ..body = dart.Block((b) => b
        ..statements.addAll([
          // ignore: unnecessary_this
          this.assignFinal('result').statement,
          dart.Code('if (result < 0) {'),
          compileType(context, CandyType.comparableLess)
              .call([])
              .returned
              .statement,
          dart.Code('} else if (result > 0) {'),
          compileType(context, CandyType.comparableGreater)
              .call([])
              .returned
              .statement,
          dart.Code('} else {'),
          compileType(context, CandyType.comparableEqual)
              .call([])
              .returned
              .statement,
          dart.Code('}'),
        ]))).closure.call([]);
  }
}
