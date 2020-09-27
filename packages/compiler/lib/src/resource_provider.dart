import 'dart:io';

import 'package:compiler/compiler.dart';
import 'package:meta/meta.dart';
import 'package:path/path.dart' as p;

import 'compilation/ast/parser.dart';

abstract class ResourceProvider {
  const ResourceProvider();

  @experimental
  // ignore: non_constant_identifier_names
  factory ResourceProvider.default_(Directory projectDirectory) =>
      SimpleResourceProvider(
        coreDirectory: Directory(p.join(_candyDirectory.path, 'core')),
        projectDirectory: projectDirectory,
        packagesDirectory: _candyDirectory,
      );

  static final _candyDirectory = Directory('D:/p/candy/packages');
  static const srcDirectoryName = 'src';

  bool fileExists(ResourceId id);
  bool directoryExists(ResourceId id);

  String getContent(ResourceId id);
}

class SimpleResourceProvider extends ResourceProvider {
  SimpleResourceProvider({
    @required Directory coreDirectory,
    @required Directory projectDirectory,
    @required Directory packagesDirectory,
  })  : assert(coreDirectory != null),
        coreDirectory = coreDirectory.absolute,
        assert(projectDirectory != null),
        projectDirectory = projectDirectory.absolute,
        assert(packagesDirectory != null),
        packagesDirectory = packagesDirectory.absolute;

  static String isValidProjectDirectory(Directory directory) {
    final srcDirectory =
        Directory(p.join(directory.path, ResourceProvider.srcDirectoryName));
    if (!srcDirectory.existsSync()) return 'No `src`-directory found.';

    return null;
  }

  final Directory coreDirectory;
  final Directory projectDirectory;
  final Directory packagesDirectory;

  @override
  bool fileExists(ResourceId id) => File(_resolve(id)).existsSync();
  @override
  bool directoryExists(ResourceId id) => Directory(_resolve(id)).existsSync();

  @override
  String getContent(ResourceId id) {
    final file = File(_resolve(id));
    assert(file.existsSync());
    return file.readAsStringSync();
  }

  String _resolve(ResourceId id) {
    return p.joinAll([
      if (id.packageId.isCore)
        coreDirectory.path
      else if (id.packageId.isThis)
        projectDirectory.path
      else ...[packagesDirectory.path, id.packageId.name],
      ResourceProvider.srcDirectoryName,
      id.path,
    ]);
  }
}
