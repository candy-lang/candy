import 'package:freezed_annotation/freezed_annotation.dart';
import 'package:parser/parser.dart';
import 'package:path/path.dart' as p;

import '../../constants.dart';
import '../../query.dart';
import '../ids.dart';

part 'parser.freezed.dart';
part 'parser.g.dart';

/// Identifier of a single file.
///
/// `path` is a relative path from the package's `./src/`-folder that may not
/// leave this directory (via `..`).
@freezed
abstract class ResourceId implements _$ResourceId {
  const factory ResourceId(PackageId packageId, String path) = _ResourceId;
  factory ResourceId.fromJson(Map<String, dynamic> json) =>
      _$ResourceIdFromJson(json);
  const ResourceId._();

  String get extension => p.extension(path);
  bool get isCandyFile => extension == candyFileExtension;

  String get fileName => p.basename(path);
  String get fileNameWithoutExtension => p.basenameWithoutExtension(path);

  bool get isPackageRoot => p.dirname(path) == '.';
  ResourceId get parent {
    assert(!isPackageRoot);
    return ResourceId(packageId, p.dirname(path));
  }

  ResourceId sibling(String name) =>
      ResourceId(packageId, '${p.dirname(path)}/$name');
  ResourceId child(String name) => ResourceId(packageId, '$path/$name');
}

final doesResourceExist = Query<ResourceId, bool>(
  'doesResourceExist',
  evaluateAlways: true,
  provider: (context, resourceId) =>
      context.resourceProvider.fileExists(resourceId),
);
final doesResourceDirectoryExist = Query<ResourceId, bool>(
  'doesResourceDirectoryExist',
  evaluateAlways: true,
  provider: (context, resourceId) =>
      context.resourceProvider.directoryExists(resourceId),
);

final getSourceCode = Query<ResourceId, String>(
  'getSourceCode',
  evaluateAlways: true,
  provider: (context, resourceId) {
    assert(resourceId.isCandyFile);
    return context.resourceProvider.getContent(resourceId);
  },
);

final getAst = Query<ResourceId, CandyFile>(
  'getAst',
  provider: (context, resourceId) {
    final source = context.callQuery(getSourceCode, resourceId);
    return parseCandySource(resourceId.fileNameWithoutExtension, source);
  },
);
