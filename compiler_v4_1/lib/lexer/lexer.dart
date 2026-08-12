import 'package:deranged/deranged.dart';
import 'package:supernova/supernova.dart';

import '../error.dart';
import '../file.dart';
import '../position.dart';
import 'token.dart';

class Lexer {
  Lexer._(this.file, this.source);

  final SourceFile file;
  final String source;

  static (List<Token>, List<CompilerError>) lex(
    SourceFile file,
    String source,
  ) {
    final lexer = Lexer._(file, source);
    lexer._scanTokens();
    return (lexer._result, lexer._errors);
  }

  late final _characters = source.characters.iterator;
  var _start = const Offset(0);
  var _current = const Offset(0);
  bool _isAtEnd() => _current.value == source.length;
  final _result = <Token>[];
  final _errors = <CompilerError>[];

  void _add(TokenKind kind) {
    _result.add(
      Token(span: SourceSpan(file, _start.rangeTo(_current)), kind: kind),
    );
    _start = _current;
  }

  String _currentString() => source.substring(_start.value, _current.value);

  void _scanTokens() {
    while (!_isAtEnd()) {
      _scanToken();
    }
    _add(const .endOfFile());
  }

  void _scanToken() {
    final char = _advance()!;
    switch (char) {
      case '(':
        _add(const .openingParenthesis());
      case ')':
        _add(const .closingParenthesis());
      case '{':
        _add(const .openingBrace());
      case '}':
        _add(const .closingBrace());
      case '[':
        _add(const .openingBracket());
      case ']':
        _add(const .closingBracket());
      case ':':
        _add(const .colon());
      case ',':
        _add(const .comma());
      case '.':
        _add(const .dot());
      case '=':
        _add(const .equal());
      case '-' when _peek() == '>':
        _advance();
        _add(const .arrow());
      case '#':
        while (true) {
          final next = _peek();
          if (next == null || next == '\n') break;

          _advance();
        }
        _add(.comment(_currentString()));
      case ' ' || '\t' || '\n' || '\r':
        while (true) {
          final next = _peek();
          if (next == null ||
              (next != ' ' && next != '\t' && next != '\n' && next != '\r')) {
            break;
          }
          _advance();
        }
        _add(.whitespace(_currentString()));
      case '"':
        _add(const .openingQuote());
        _scanString();
      case _ when char.isAsciiDigit:
        _scanNumber();
      case _ when char.isAsciiLetter || char == '_':
        _scanIdentifierOrKeyword();
    }
  }

  void _scanString() {
    var regularEnd = _current;
    void maybeFinishPrecedingRegular() {
      if (regularEnd == _start) return;

      _add(const .regularStringPart());
      _start = regularEnd;
    }

    while (true) {
      switch (_advance()) {
        case '"':
          maybeFinishPrecedingRegular();
          _add(const .closingQuote());
          return;
        case r'\':
          maybeFinishPrecedingRegular();
          switch (_advance()) {
            case 'r':
              _add(const .escapedChar('\r'));
            case 'n':
              _add(const .escapedChar('\n'));
            case 't':
              _add(const .escapedChar('\t'));
            case '"':
              _add(const .escapedChar('"'));
            case r'\':
              _add(const .escapedChar(r'\'));
            case final char:
              _add(const .regularStringPart());
              _errors.add(
                CompilerError(
                  span: SourceSpan(file, _start.rangeTo(_current)),
                  message: 'Incomplete escape sequence',
                ),
              );
              if (char == null) return;
          }
        case null:
          maybeFinishPrecedingRegular();
          _add(const .regularStringPart());
          _errors.add(
            CompilerError(
              span: SourceSpan(file, _start.rangeTo(_current)),
              message: 'Unterminated string literal',
            ),
          );
          return;
        default:
        // Handled by [maybeFinishPrecedingRegular].
      }
      regularEnd = _current;
    }
  }

  void _scanNumber() {
    while (_peek()?.isAsciiDigit ?? false) {
      _advance();
    }
    _add(.int(_currentString().toInt()));
  }

  void _scanIdentifierOrKeyword() {
    while (true) {
      final next = _peek();
      if (next == null ||
          !(next.isAsciiLetter || next.isAsciiDigit || next == '_')) {
        break;
      }
      _advance();
    }

    _add(switch (_currentString()) {
      'builtin' => const .builtinKeyword(),
      'struct' => const .structKeyword(),
      'enum' => const .enumKeyword(),
      'trait' => const .traitKeyword(),
      'impl' => const .implKeyword(),
      'where' => const .whereKeyword(),
      'fun' => const .funKeyword(),
      'let' => const .letKeyword(),
      'switch' => const .switchKeyword(),
      _ => const .identifier(),
    });
  }

  String? _peek() {
    final copy = _characters.copy();
    if (!copy.moveNext()) return null;

    return copy.current;
  }

  String? _advance() {
    if (!_characters.moveNext()) return null;

    final char = _characters.current;
    _current += char.length;
    return char;
  }
}

extension on String {
  bool get isAsciiDigit => length == 1 && this >= '0' && this <= '9';
  bool get isAsciiLetter =>
      length == 1 && (this >= 'a' && this <= 'z' || this >= 'A' && this <= 'Z');
}
