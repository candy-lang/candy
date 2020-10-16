import 'dart:io';

import 'package:meta/meta.dart';
import 'package:path/path.dart' as p;

import 'candyspec.dart';
import 'compilation/ast.dart';
import 'compilation/ids.dart';
import 'errors.dart';
import 'query.dart';

abstract class ResourceProvider {
  const ResourceProvider();

  @experimental
  // ignore: non_constant_identifier_names
  factory ResourceProvider.default_(Directory projectDirectory) =>
      SimpleResourceProvider(
        candyDirectory: _candyDirectory,
        projectDirectory: projectDirectory,
      );

  static final _candyDirectory = Directory('D:/p/candy/packages/candy');

  bool fileExists(QueryContext context, ResourceId id);
  bool directoryExists(QueryContext context, ResourceId id);

  String getContent(QueryContext context, ResourceId id);
}

class SimpleResourceProvider extends ResourceProvider {
  SimpleResourceProvider({
    @required Directory candyDirectory,
    @required Directory projectDirectory,
  })  : assert(candyDirectory != null),
        candyDirectory = candyDirectory.absolute,
        assert(projectDirectory != null),
        projectDirectory = projectDirectory.absolute;

  static String isValidProjectDirectory(Directory directory) {
    if (!directory.existsSync()) return "Directory doesn't exist.";

    return null;
  }

  final Directory candyDirectory;
  final Directory projectDirectory;

  @override
  bool fileExists(QueryContext context, ResourceId id) =>
      File(_resolve(context, id)).existsSync();
  @override
  bool directoryExists(QueryContext context, ResourceId id) =>
      Directory(_resolve(context, id)).existsSync();

  @override
  String getContent(QueryContext context, ResourceId id) {
    final file = File(_resolve(context, id));
    assert(file.existsSync());
    return file.readAsStringSync();
  }

  String _resolve(QueryContext context, ResourceId id) {
    if (id.packageId.isThis) return p.join(projectDirectory.path, id.path);
    if (id.packageId.isCore) return p.join(candyDirectory.path, id.path);

    // TODO(JonasWanke): resolve all dependencies
    final candyspec = getCandyspec(context, PackageId.this_);
    final dependency = candyspec.dependencies[id.packageId.name];
    if (dependency == null) {
      throw CompilerError.unknownPackage(
        'Package `${id.packageId.name}` is not specified as a (direct) dependency.',
      );
    }

    // We include the project directory's path to allow relative dependency
    // paths.
    return p.join(projectDirectory.path, dependency.path, id.path);
  }
}
