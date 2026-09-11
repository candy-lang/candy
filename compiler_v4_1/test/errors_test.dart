import 'dart:io';

import 'package:compiler_v4_1/context.dart';
import 'package:compiler_v4_1/error.dart';
import 'package:compiler_v4_1/file.dart';
import 'package:compiler_v4_1/position.dart';
import 'package:deranged/deranged.dart';
import 'package:test/test.dart';

void main() {
  group('a single analysis', () {
    test('reports errors ordered by file and position', () {
      final context = Context()
        ..addError(_errorAt(_b, 10, 'third'))
        ..addError(_errorAt(_a, 20, 'second'))
        ..addError(_errorAt(_a, 5, 'first'));

      expect(context.errors.map((it) => it.message), [
        'first',
        'second',
        'third',
      ]);
    });

    test('reports the same error only once', () {
      final context = Context()
        ..addError(_errorAt(_a, 5, 'once'))
        ..addError(_errorAt(_a, 5, 'once'));

      expect(context.errors, hasLength(1));
    });

    test('keeps two different errors at the same position', () {
      final context = Context()
        ..addError(_errorAt(_a, 5, 'one thing'))
        ..addError(_errorAt(_a, 5, 'another thing'));

      expect(context.errors, hasLength(2));
    });
  });

  group('several analyses', () {
    test('drops the errors two of them share', () {
      final merged = mergeErrors([
        [_errorAt(_a, 5, 'shared'), _errorAt(_a, 20, 'only in the first')],
        [_errorAt(_a, 5, 'shared'), _errorAt(_b, 10, 'only in the second')],
      ]);

      expect(merged.map((it) => it.message), [
        'shared',
        'only in the first',
        'only in the second',
      ]);
    });

    test('recognizes an error in an unsaved buffer as the same error', () {
      final edited = SourceFile(File('/packages/test/a.candy'), 'other text');
      final merged = mergeErrors([
        [_errorAt(_a, 5, 'shared')],
        [_errorAt(edited, 5, 'shared')],
      ]);

      expect(merged, hasLength(1));
    });
  });
}

final _a = SourceFile(File('/packages/test/a.candy'), 'some text');
final _b = SourceFile(File('/packages/test/b.candy'), 'some text');

CompilerError _errorAt(SourceFile file, int offset, String message) =>
    CompilerError(
      span: SourceSpan(
        file,
        RangeInclusive(Offset(offset), Offset(offset + 1)),
      ),
      message: message,
    );
