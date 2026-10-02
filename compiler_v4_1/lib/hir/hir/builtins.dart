import 'hir.dart';
import 'ids.dart';

/// Declared in source as `struct Int = builtin`.
enum BuiltinType(final String candyName) {
  int('Int'),
  list('List'),
  text('Text');

  static BuiltinType? fromName(String name) =>
      values.where((it) => it.candyName == name).firstOrNull;

  @override
  String toString() => this == list ? 'List[T]' : candyName;
}

/// Types the compiler needs to name. They're declared in ordinary Candy
/// source, so a program that omits one gets [HirType.error] in its place.
class const WellKnownTypes({
  final DeclarationId? intId,
  final DeclarationId? textId,
  final DeclarationId? listId,
  final DeclarationId? nothingId,
  final DeclarationId? neverId,
  final DeclarationId? maybeId,
  final DeclarationId? orderingId,
  final DeclarationId? resultId,
  final DeclarationId? boolId,
  final DeclarationId? typeIdId,
  final DeclarationId? anyId,
  final DeclarationId? oxygenColorId,
  final DeclarationId? oxygenDrawCommandId,
  final DeclarationId? oxygenDrawRectId,
  final DeclarationId? oxygenOffsetId,
  final DeclarationId? oxygenPaintId,
  final DeclarationId? oxygenRectId,
}) {
  static const names = {
    'Int',
    'Text',
    'List',
    'Nothing',
    'Never',
    'Maybe',
    'Ordering',
    'Result',
    'Bool',
    'TypeId',
    'Color',
    'DrawCommand',
    'DrawRect',
    'Offset',
    'Paint',
    'Rect',
  };
  static const traitNames = {'Any'};

  static HirType _named(
    DeclarationId? declaration, [
    List<HirType>? arguments,
  ]) {
    if (declaration == null) return const .error();
    return .named(
      declaration: declaration,
      typeArguments: arguments ?? const [],
    );
  }

  HirType get intType => _named(intId);
  HirType get textType => _named(textId);
  HirType listType(HirType item) => _named(listId, [item]);
  HirType get nothingType => _named(nothingId);
  HirType get neverType => _named(neverId);
  HirType maybeType(HirType value) => _named(maybeId, [value]);
  HirType get orderingType => _named(orderingId);
  HirType resultType(HirType value, HirType error) =>
      _named(resultId, [value, error]);
  HirType get boolType => _named(boolId);
  HirType get typeIdType => _named(typeIdId);
  HirType get oxygenColorType => _named(oxygenColorId);
  HirType get oxygenDrawCommandType => _named(oxygenDrawCommandId);
  HirType get oxygenDrawRectType => _named(oxygenDrawRectId);
  HirType get oxygenPaintType => _named(oxygenPaintId);
  HirType get oxygenOffsetType => _named(oxygenOffsetId);
  HirType get oxygenRectType => _named(oxygenRectId);

  bool isAny(DeclarationId trait) => anyId != null && trait == anyId;

  /// `Never` is assignable to everything.
  bool isNever(HirType type) =>
      neverId != null &&
      type is HirType_Named &&
      type.declaration == neverId &&
      type.typeArguments.isEmpty;
}

/// Implemented by the backend rather than in Candy.
enum BuiltinFunction {
  cairoRender('builtinCairoRender'),
  intAdd('builtinIntAdd'),
  intBitwiseAnd('builtinIntBitwiseAnd'),
  intBitwiseOr('builtinIntBitwiseOr'),
  intBitwiseXor('builtinIntBitwiseXor'),
  intCompareTo('builtinIntCompareTo'),
  intDivideTruncating('builtinIntDivideTruncating'),
  intMultiply('builtinIntMultiply'),
  intParse('builtinIntParse'),
  intRemainder('builtinIntRemainder'),
  intShiftLeft('builtinIntShiftLeft'),
  intShiftRight('builtinIntShiftRight'),
  intSubtract('builtinIntSubtract'),
  intToText('builtinIntToText'),
  listFilled('builtinListFilled'),
  listGenerate('builtinListGenerate'),
  listGet('builtinListGet'),
  listInsert('builtinListInsert'),
  listLength('builtinListLength'),
  listOf0('builtinListOf'),
  listOf1('builtinListOf'),
  listOf2('builtinListOf'),
  listOf3('builtinListOf'),
  listOf4('builtinListOf'),
  listOf5('builtinListOf'),
  downcast('builtinDowncast'),
  listRemoveAt('builtinListRemoveAt'),
  listReplace('builtinListReplace'),
  panic('builtinPanic'),
  print('builtinPrint'),
  textCompareTo('builtinTextCompareTo'),
  textConcat('builtinTextConcat'),
  textGetRange('builtinTextGetRange'),
  textIndexOf('builtinTextIndexOf'),
  textLength('builtinTextLength'),
  typeId('builtinTypeId'),
  typeIdOf('builtinTypeIdOf');

  new(this.candyName);

  /// Not unique: the `builtinListOf` overloads share one.
  final String candyName;

  static final Map<BuiltinFunction, int> _disambiguators = () {
    final counts = <String, int>{};
    return {
      for (final function in values)
        function: counts.update(
          function.candyName,
          (it) => it + 1,
          ifAbsent: () => 0,
        ),
    };
  }();

  DeclarationId get id => DeclarationId(
    module: .builtin,
    name: candyName,
    disambiguator: _disambiguators[this]!,
  );

  HirFunctionSignature signature(WellKnownTypes types, HirIdGenerator ids) {
    final intType = types.intType;
    final textType = types.textType;

    HirFunctionSignature build(
      List<(String, HirType)> parameters,
      HirType returnType, {
      List<HirTypeParameter> typeParameters = const [],
    }) => HirFunctionSignature(
      id: id,
      name: candyName,
      typeParameters: typeParameters,
      parameters: [
        for (final (name, type) in parameters)
          HirParameter(id: ids.generate(), name: name, type: type),
      ],
      returnType: returnType,
    );

    final tParameter = _typeParameter(0, 'T');
    final t = tParameter.type;
    // A value's static type, which may itself be a trait object.
    final vParameter = _typeParameter(0, 'V');
    final vAfterTParameter = _typeParameter(1, 'V');

    HirFunctionSignature intBinary(String a, String b) =>
        build([(a, intType), (b, intType)], intType);

    HirFunctionSignature listOf(int count) => build(
      [for (var i = 0; i < count; i++) ('item$i', t)],
      types.listType(t),
      typeParameters: [tParameter],
    );

    return switch (this) {
      .cairoRender => build([
        ('drawCommands', types.listType(types.oxygenDrawCommandType)),
      ], types.nothingType),
      .intAdd => intBinary('a', 'b'),
      .intBitwiseAnd => intBinary('a', 'b'),
      .intBitwiseOr => intBinary('a', 'b'),
      .intBitwiseXor => intBinary('a', 'b'),
      .intCompareTo => build([
        ('a', intType),
        ('b', intType),
      ], types.orderingType),
      .intDivideTruncating => intBinary('dividend', 'divisor'),
      .intMultiply => intBinary('factorA', 'factorB'),
      .intParse => build([
        ('text', textType),
      ], types.resultType(intType, textType)),
      .intRemainder => intBinary('dividend', 'divisor'),
      .intShiftLeft => intBinary('value', 'amount'),
      .intShiftRight => intBinary('value', 'amount'),
      .intSubtract => intBinary('minuend', 'subtrahend'),
      .intToText => build([('int', intType)], textType),
      .listFilled => build(
        [('length', intType), ('item', t)],
        types.listType(t),
        typeParameters: [tParameter],
      ),
      .listGenerate => build(
        [
          ('length', intType),
          (
            'itemGetter',
            HirType.function(parameterTypes: [intType], returnType: t),
          ),
        ],
        types.listType(t),
        typeParameters: [tParameter],
      ),
      .listGet => build(
        [('list', types.listType(t)), ('index', intType)],
        types.maybeType(t),
        typeParameters: [tParameter],
      ),
      .listInsert => build(
        [('list', types.listType(t)), ('index', intType), ('item', t)],
        types.listType(t),
        typeParameters: [tParameter],
      ),
      .listLength => build(
        [('list', types.listType(t))],
        intType,
        typeParameters: [tParameter],
      ),
      .listOf0 => listOf(0),
      .listOf1 => listOf(1),
      .listOf2 => listOf(2),
      .listOf3 => listOf(3),
      .listOf4 => listOf(4),
      .listOf5 => listOf(5),
      .listRemoveAt => build(
        [('list', types.listType(t)), ('index', intType)],
        types.listType(t),
        typeParameters: [tParameter],
      ),
      .listReplace => build(
        [('list', types.listType(t)), ('index', intType), ('newItem', t)],
        types.listType(t),
        typeParameters: [tParameter],
      ),
      .panic => build([('message', textType)], types.neverType),
      .print => build([('message', textType)], types.nothingType),
      .textCompareTo => build([
        ('a', textType),
        ('b', textType),
      ], types.orderingType),
      .textConcat => build([('a', textType), ('b', textType)], textType),
      .textGetRange => build([
        ('text', textType),
        ('startInclusive', intType),
        ('endExclusive', intType),
      ], textType),
      .textIndexOf => build([
        ('a', textType),
        ('b', textType),
      ], types.maybeType(intType)),
      .textLength => build([('text', textType)], intType),
      .typeId => build(
        const [],
        types.typeIdType,
        typeParameters: [tParameter],
      ),
      .typeIdOf => build(
        [('value', vParameter.type)],
        types.typeIdType,
        typeParameters: [vParameter],
      ),
      .downcast => build(
        [('value', vAfterTParameter.type)],
        types.maybeType(t),
        typeParameters: [tParameter, vAfterTParameter],
      ),
    };
  }

  HirTypeParameter _typeParameter(int index, String name) => HirTypeParameter(
    id: TypeParameterId.declared(owner: id, index: index, name: name),
  );
}
