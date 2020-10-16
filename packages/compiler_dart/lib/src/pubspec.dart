import 'package:compiler/compiler.dart';
import 'package:pub_semver/pub_semver.dart';
import 'package:pubspec/pubspec.dart';

import 'constants.dart';

final generatePubspec = Query<PackageId, PubSpec>(
  'dart.generatePubspec',
  provider: (context, packageId) {
    final candyspec = getCandyspec(context, packageId);

    final dependencyNames = [
      if (packageId.isNotCore) PackageId.core.toString(),
      ...candyspec.dependencies.keys,
    ];
    return PubSpec(
      name: candyspec.name,
      dependencies: {
        'meta':
            HostedReference(VersionConstraint.compatibleWith(Version(1, 1, 7))),
        for (final dependency in dependencyNames)
          dependency: PathReference(
            context.config.buildArtifactManager
                .toPath(context, PackageId(dependency).dartBuildArtifactId),
          ),
      },
      environment: Environment(VersionConstraint.parse('>=2.7.0 <3.0.0'), {}),
    );
  },
);
