import 'dart:io';

import 'package:compiler_v4_1/hir/hir/ids.dart';
import 'package:compiler_v4_1/lowering/modules.dart';
import 'package:test/test.dart';

void main() {
  final package = Package(
    name: 'core',
    rootDirectory: Directory('/packages/core'),
  );

  test('maps a plain file to its path', () {
    expect(
      package.moduleIdFor(File('/packages/core/text/trim.candy')),
      const ModuleId(packageName: 'core', path: ['text', 'trim']),
    );
  });

  test('maps `text/_.candy` to the module `text`', () {
    expect(
      package.moduleIdFor(File('/packages/core/text/_.candy')),
      const ModuleId(packageName: 'core', path: ['text']),
    );
  });

  test('maps `_.candy` at the root to the package root module', () {
    expect(
      package.moduleIdFor(File('/packages/core/_.candy')),
      const ModuleId(packageName: 'core', path: []),
    );
  });
}
