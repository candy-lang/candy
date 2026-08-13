import 'hir.dart';
import 'ids.dart';

/// Declared in source as `struct Int = builtin`.
enum BuiltinType {
  int('Int'),
  list('List'),
  text('Text');

  const BuiltinType(this.candyName);

  final String candyName;

  static BuiltinType? fromName(String name) =>
      values.where((it) => it.candyName == name).firstOrNull;

  @override
  String toString() => this == list ? 'List[T]' : candyName;
}

/// Types the compiler needs to name. They're declared in ordinary Candy
/// source, so a program that omits one gets [HirType.error] in its place.
class WellKnownTypes {
  const WellKnownTypes({
    this.intId,
    this.textId,
    this.listId,
    this.nothingId,
    this.neverId,
    this.maybeId,
    this.orderingId,
    this.resultId,
    this.boolId,
  });

  final DeclarationId? intId;
  final DeclarationId? textId;
  final DeclarationId? listId;
  final DeclarationId? nothingId;
  final DeclarationId? neverId;
  final DeclarationId? maybeId;
  final DeclarationId? orderingId;
  final DeclarationId? resultId;
  final DeclarationId? boolId;

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
  };

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

  /// `Never` is assignable to everything.
  bool isNever(HirType type) =>
      neverId != null &&
      type is HirType_Named &&
      type.declaration == neverId &&
      type.typeArguments.isEmpty;
}

/// Implemented by the backend rather than in Candy.
enum BuiltinFunction {
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
  listRemoveAt('builtinListRemoveAt'),
  listReplace('builtinListReplace'),
  panic('builtinPanic'),
  print('builtinPrint'),
  textCompareTo('builtinTextCompareTo'),
  textConcat('builtinTextConcat'),
  textGetRange('builtinTextGetRange'),
  textIndexOf('builtinTextIndexOf'),
  textLength('builtinTextLength');

  const BuiltinFunction(this.candyName);

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

  TypeParameterId get _itemTypeParameter =>
      TypeParameterId.declared(owner: id, index: 0, name: 'T');

  HirFunctionSignature signature(WellKnownTypes types, HirIdGenerator ids) {
    final t = HirType.parameter(_itemTypeParameter);
    final intType = types.intType;
    final textType = types.textType;

    HirFunctionSignature build(
      List<(String, HirType)> parameters,
      HirType returnType, {
      bool isGeneric = false,
    }) => HirFunctionSignature(
      id: id,
      name: candyName,
      typeParameters: isGeneric
          ? [HirTypeParameter(id: _itemTypeParameter, upperBound: null)]
          : const [],
      parameters: [
        for (final (name, type) in parameters)
          HirParameter(id: ids.generate(), name: name, type: type),
      ],
      returnType: returnType,
    );

    HirFunctionSignature intBinary(String a, String b) =>
        build([(a, intType), (b, intType)], intType);

    HirFunctionSignature listOf(int count) => build(
      [for (var i = 0; i < count; i++) ('item$i', t)],
      types.listType(t),
      isGeneric: true,
    );

    return switch (this) {
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
        isGeneric: true,
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
        isGeneric: true,
      ),
      .listGet => build(
        [('list', types.listType(t)), ('index', intType)],
        types.maybeType(t),
        isGeneric: true,
      ),
      .listInsert => build(
        [('list', types.listType(t)), ('index', intType), ('item', t)],
        types.listType(t),
        isGeneric: true,
      ),
      .listLength => build(
        [('list', types.listType(t))],
        intType,
        isGeneric: true,
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
        isGeneric: true,
      ),
      .listReplace => build(
        [('list', types.listType(t)), ('index', intType), ('newItem', t)],
        types.listType(t),
        isGeneric: true,
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
    };
  }
}
