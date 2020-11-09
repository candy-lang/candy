import 'package:freezed_annotation/freezed_annotation.dart';
import 'package:yaml/yaml.dart';

import 'compilation/ast.dart';
import 'compilation/ids.dart';
import 'constants.dart';
import 'errors.dart';
import 'query.dart';
import 'utils.dart';

part 'candyspec.freezed.dart';
part 'candyspec.g.dart';

@freezed
abstract class Candyspec implements _$Candyspec {
  const factory Candyspec({
    @required String name,
    @Default(<String, Dependency>{}) Map<String, Dependency> dependencies,
  }) = _Candyspec;
  factory Candyspec.fromJson(Map<String, dynamic> json) =>
      _$CandyspecFromJson(json);
  const Candyspec._();
}

@freezed
abstract class Dependency implements _$Dependency {
  const factory Dependency({@required String path}) = _Dependency;
  factory Dependency.fromJson(Map<String, dynamic> json) =>
      _$DependencyFromJson(json);
  const Dependency._();
}

extension CandyspecResourceId on PackageId {
  ResourceId get candyspecId => ResourceId(this, candyspecName);
}

final getCandyspec = Query<PackageId, Candyspec>(
  'getCandyspec',
  provider: (context, packageId) {
    final content = getCandyspecFileContent(context, packageId);
    final json =
        (loadYaml(content) as Map<dynamic, dynamic>).cast<String, dynamic>();
    return Candyspec.fromJson(json);
  },
);
final getCandyspecFileContent = Query<PackageId, String>(
  'getCandyspecFileContent',
  evaluateAlways: true,
  provider: (context, packageId) {
    final candyspecId = packageId.candyspecId;
    if (!context.config.resourceProvider.fileExists(context, candyspecId)) {
      throw CompilerError.candyspecMissing('`$candyspecName` does not exist.');
    }

    return context.config.resourceProvider
        .getContent(context, packageId.candyspecId);
  },
);

final getAllDependencies = Query<Unit, List<PackageId>>(
  'getAllDependencies',
  provider: (context, _) {
    final candyspec = getCandyspec(context, context.config.packageId);
    return candyspec.dependencies.keys
        .map((name) => PackageId(name))
        .followedBy([
      if (context.config.packageId != PackageId.core) PackageId.core,
    ]).toList();
  },
);
