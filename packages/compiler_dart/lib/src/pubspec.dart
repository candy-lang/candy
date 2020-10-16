import 'package:compiler/compiler.dart';
import 'package:pub_semver/pub_semver.dart';
import 'package:pubspec/pubspec.dart';

final generatePubspec = Query<Unit, PubSpec>(
  'dart.generatePubspec',
  provider: (context, _) {
    final candyspec = getCandyspec(context, PackageId.this_);

    return PubSpec(
      name: candyspec.name,
      dependencies: {
        'meta':
            HostedReference(VersionConstraint.compatibleWith(Version(1, 1, 7))),
      },
      environment: Environment(VersionConstraint.parse('>=2.7.0 <3.0.0'), {}),
    );
  },
);
