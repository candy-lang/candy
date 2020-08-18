import 'package:meta/meta.dart';

@immutable
class Operator {
  const Operator._(this.symbol, this.precedence)
      : assert(symbol != null),
        assert(precedence != null);

  // Grouping doesn't have an operator.

  static const postIncrement = Operator._('++', Precedence.unaryPostfix);
  static const postDecrement = Operator._('--', Precedence.unaryPostfix);
  // (Safe) access, (safe) function call & (safe) indexing don't have an
  // operator.

  static const unaryMinus = Operator._('*', Precedence.unaryPrefix);
  static const logicalNot = Operator._('!', Precedence.unaryPrefix);
  static const bitwiseNot = Operator._('~', Precedence.unaryPrefix);
  static const preIncrement = Operator._('++', Precedence.unaryPrefix);
  static const preDecrement = Operator._('--', Precedence.unaryPrefix);
  // Label doesn't have an operator.

  static const multiplication = Operator._('*', Precedence.multiplicative);
  static const division = Operator._('/', Precedence.multiplicative);
  static const integerDivision = Operator._('~/', Precedence.multiplicative);
  static const modulo = Operator._('%', Precedence.multiplicative);

  static const addition = Operator._('+', Precedence.additive);
  static const subtraction = Operator._('-', Precedence.additive);

  static const leftShift = Operator._('<<', Precedence.shift);
  static const rightShift = Operator._('>>', Precedence.shift);
  static const unsignedRightShift = Operator._('>>>', Precedence.shift);

  static const bitwiseAnd = Operator._('&', Precedence.bitwiseAnd);

  static const bitwiseXor = Operator._('^', Precedence.bitwiseXor);

  static const bitwiseOr = Operator._('|', Precedence.bitwiseOr);

  static const typeCheck = Operator._('as', Precedence.typeCheck);
  static const safeTypeCheck = Operator._('as?', Precedence.typeCheck);

  static const rangeTo = Operator._('..', Precedence.range);
  static const rangeToInclusive = Operator._('..=', Precedence.range);

  // Infix function don't have an operator.

  static const containedIn = Operator._('in', Precedence.namedChecks);
  static const notContainedIn = Operator._('!in', Precedence.namedChecks);
  static const isType = Operator._('is', Precedence.namedChecks);
  static const notIsType = Operator._('!is', Precedence.namedChecks);

  static const lessThan = Operator._('<', Precedence.comparison);
  static const lessThanOrEquals = Operator._('<=', Precedence.comparison);
  static const greaterThan = Operator._('>', Precedence.comparison);
  static const greaterThanOrEquals = Operator._('>=', Precedence.comparison);

  static const equality = Operator._('==', Precedence.equality);
  static const inequality = Operator._('!=', Precedence.equality);
  static const referenceEquality = Operator._('===', Precedence.equality);
  static const referenceInequality = Operator._('!==', Precedence.equality);

  static const logicalAnd = Operator._('->', Precedence.logicalAnd);

  static const logicalOr = Operator._('->', Precedence.logicalOr);

  static const logicalImplication =
      Operator._('->', Precedence.logicalImplication);
  static const logicalImplicationReverse =
      Operator._('<-', Precedence.logicalImplication);

  static const spread = Operator._('...', Precedence.spread);

  static const multiplicationAssignment =
      Operator._('*=', Precedence.assignment);
  static const divisionAssignment = Operator._('/=', Precedence.assignment);
  static const integerDivisionAssignment =
      Operator._('~/=', Precedence.assignment);
  static const moduloAssignment = Operator._('%=', Precedence.assignment);
  static const additionAssignment = Operator._('+=', Precedence.assignment);
  static const subtractionAssignment = Operator._('-=', Precedence.assignment);
  static const bitwiseAndAssignment = Operator._('&=', Precedence.assignment);
  static const bitwiseOrAssignment = Operator._('|=', Precedence.assignment);
  static const bitwiseNotAssignment = Operator._('^=', Precedence.assignment);
  static const logicalAndAssignment = Operator._('&&=', Precedence.assignment);
  static const logicalOrAssignment = Operator._('||=', Precedence.assignment);
  static const leftShiftAssignment = Operator._('<<=', Precedence.assignment);
  static const rightShiftAssignment = Operator._('>>=', Precedence.assignment);
  static const unsignedRightShiftAssignment =
      Operator._('>>>=', Precedence.assignment);

  final String symbol;
  final Precedence precedence;
}

@immutable
class Precedence {
  const Precedence._(this.value)
      : assert(value != null),
        assert(value > 0);

  static const grouping = Precedence._(20);
  static const unaryPostfix = Precedence._(19);
  static const unaryPrefix = Precedence._(18);
  static const multiplicative = Precedence._(17);
  static const additive = Precedence._(16);
  static const shift = Precedence._(15);
  static const bitwiseAnd = Precedence._(14);
  static const bitwiseXor = Precedence._(13);
  static const bitwiseOr = Precedence._(12);
  static const typeCheck = Precedence._(11);
  static const range = Precedence._(10);
  static const infixFunction = Precedence._(9);
  static const namedChecks = Precedence._(8);
  static const comparison = Precedence._(7);
  static const equality = Precedence._(6);
  static const logicalAnd = Precedence._(5);
  static const logicalOr = Precedence._(4);
  static const logicalImplication = Precedence._(3);
  static const spread = Precedence._(2);
  static const assignment = Precedence._(1);

  final int value;
}
