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
  factory ResourceProvider.default_({
    Directory candyDirectory,
    Directory projectDirectory,
  }) = SimpleResourceProvider;

  Directory get candyDirectory;
  Directory get projectDirectory;

  List<ResourceId> getAllFileResourceIds(
    QueryContext context,
    PackageId packageId,
  );

  bool fileExists(QueryContext context, ResourceId id);
  bool directoryExists(QueryContext context, ResourceId id);

  String getContent(QueryContext context, ResourceId id);

  Directory getPackageDirectory(QueryContext context, PackageId packageId) {
    if (packageId.isThis) return projectDirectory;
    if (packageId.isCore) return candyDirectory;

    // TODO(JonasWanke): resolve all dependencies
    final candyspec = getCandyspec(context, PackageId.this_);
    final dependency = candyspec.dependencies[packageId.name];
    if (dependency == null) {
      throw CompilerError.unknownPackage(
        'Package `${packageId.name}` is not specified as a (direct) dependency.',
      );
    }

    // We include the project directory's path to allow relative dependency
    // paths.
    return Directory(p.join(projectDirectory.path, dependency.path));
  }
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

  @override
  final Directory candyDirectory;
  @override
  final Directory projectDirectory;

  @override
  List<ResourceId> getAllFileResourceIds(
    QueryContext context,
    PackageId packageId,
  ) {
    final directory = getPackageDirectory(context, packageId);
    return directory
        .listSync(recursive: true)
        .whereType<File>()
        .map((file) => p.relative(file.path, from: directory.path))
        .map((path) => p.split(path).join('/'))
        .map((path) => ResourceId(packageId, path))
        .toList();
  }

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

  String _resolve(QueryContext context, ResourceId id) =>
      p.join(getPackageDirectory(context, id.packageId).path, id.path);
}
