import '../../../lexer/lexer.dart';
import '../../../syntactic_entity.dart';
import 'expression.dart';

abstract class Literal<T> extends Expression {
  const Literal(this.valueToken);

  final LiteralToken<T> valueToken;
  T get value => valueToken.value;

  @override
  Iterable<SyntacticEntity> get children => [valueToken];
}

class IntegerLiteral extends Literal<int> {
  const IntegerLiteral(IntegerLiteralToken valueToken)
      : assert(valueToken != null),
        super(valueToken);
}

class BooleanLiteral extends Literal<bool> {
  const BooleanLiteral(BooleanLiteralToken valueToken)
      : assert(valueToken != null),
        super(valueToken);
}
