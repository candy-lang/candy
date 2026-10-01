import 'dart:io';

import 'package:compiler_v4_1/lowering/modules.dart';
import 'package:supernova/supernova.dart';
import 'package:test/test.dart';

void main() {
  late Directory root;

  setUp(() {
    root = Directory.systemTemp.createTempSync('candy_workspace');
    for (final name in ['core', 'http']) {
      Directory('${root.path}/$name').createSync();
      File('${root.path}/$name/_package.candy').writeAsStringSync('');
      File('${root.path}/$name/main.candy').writeAsStringSync('');
    }
    Directory('${root.path}/not_a_package').createSync();
  });

  tearDown(() => root.deleteSync(recursive: true));

  test('discovers every directory holding a package marker', () {
    final workspace = Workspace.discover(root);
    expect(workspace.packages.keys.sorted(), ['core', 'http']);
  });

  test('ignores a directory without a package marker', () {
    expect(Workspace.discover(root)['not_a_package'], isNull);
  });

  test('excludes the package marker from discovered modules', () {
    final files = Workspace.discover(root)['core']!.discoverFiles();
    expect(files.map((it) => it.uri.pathSegments.last), ['main.candy']);
  });

  test('does not descend into a nested package', () {
    Directory('${root.path}/core/nested').createSync();
    File('${root.path}/core/nested/_package.candy').writeAsStringSync('');
    File('${root.path}/core/nested/inner.candy').writeAsStringSync('');

    final files = Workspace.discover(root)['core']!.discoverFiles();
    expect(files.map((it) => it.uri.pathSegments.last), ['main.candy']);
  });

  test('still descends into a plain subdirectory', () {
    Directory('${root.path}/core/plain').createSync();
    File('${root.path}/core/plain/inner.candy').writeAsStringSync('');

    final files = Workspace.discover(root)['core']!.discoverFiles();
    expect(files.map((it) => it.uri.pathSegments.last), [
      'main.candy',
      'inner.candy',
    ]);
  });
}
