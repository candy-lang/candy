import 'package:petitparser/petitparser.dart';

/// Combines the [Object.hashCode] values of an arbitrary number of objects
/// from an [Iterable] into one value. This function will return the same
/// value if given `null` as if given an empty list.
// Borrowed from dart:ui.
int hashList(Iterable<Object> arguments) {
  var result = 0;
  if (arguments != null) {
    for (final argument in arguments) {
      var hash = result;
      hash = 0x1fffffff & (hash + argument.hashCode);
      hash = 0x1fffffff & (hash + ((0x0007ffff & hash) << 10));
      result = hash ^ (hash >> 6);
    }
  }
  result = 0x1fffffff & (result + ((0x03ffffff & result) << 3));
  result = result ^ (result >> 11);
  return 0x1fffffff & (result + ((0x00003fff & result) << 15));
}

extension CandyParser<T> on Parser<T> {
  Parser<String> ignore() => map((_) => '');

  Parser<T> operator |(Parser<T> other) => _ChoiceParser([this, other]);
}

/// Copy of [ListParser] with proper type arguments.
abstract class _ListParser<T> extends Parser<T> {
  _ListParser(Iterable<Parser<T>> children)
      : children = List.of(children, growable: false);

  @override
  final List<Parser<T>> children;

  @override
  void replace(Parser<dynamic> source, Parser<dynamic> target) {
    super.replace(source, target);
    for (var i = 0; i < children.length; i++) {
      if (children[i] == source) {
        children[i] = target as Parser<T>;
      }
    }
  }
}

/// Copy of [ChoiceParser] with proper type arguments.
class _ChoiceParser<T> extends _ListParser<T> {
  _ChoiceParser(Iterable<Parser<T>> children) : super(children) {
    if (children.isEmpty) {
      throw ArgumentError('Choice parser cannot be empty.');
    }
  }

  @override
  Result<T> parseOn(Context context) {
    Result<T> result;
    for (var i = 0; i < children.length; i++) {
      result = children[i].parseOn(context);
      if (result.isSuccess) {
        return result;
      }
    }
    return result;
  }

  @override
  int fastParseOn(String buffer, int position) {
    var result = -1;
    for (var i = 0; i < children.length; i++) {
      result = children[i].fastParseOn(buffer, position);
      if (result >= 0) {
        return result;
      }
    }
    return result;
  }

  @override
  _ChoiceParser<T> copy() => _ChoiceParser(children);
}
