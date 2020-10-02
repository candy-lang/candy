import 'package:compiler/compiler.dart';
import 'package:freezed_annotation/freezed_annotation.dart';

import 'constants.dart';

part 'pubspec.freezed.dart';
part 'pubspec.g.dart';

@freezed
abstract class Pubspec implements _$Pubspec {
  @JsonSerializable(includeIfNull: false)
  const factory Pubspec({
    @required String name,
    String version,
    String desription,
    String homepage,
    String repository,
    String issueTracker,
    String documentation,
    // TODO(JonasWanke): dependencies, devDependencies, dependencyOverrides
    @required PubspecEnvironment environment,
    // TODO(JonasWanke): executables
    String publishTo,
  }) = _Pubspec;
  factory Pubspec.fromJson(Map<String, dynamic> json) =>
      _$PubspecFromJson(json);
  const Pubspec._();
}

@freezed
abstract class PubspecEnvironment implements _$PubspecEnvironment {
  const factory PubspecEnvironment({@required String dart}) =
      _PubspecEnvironment;
  factory PubspecEnvironment.fromJson(Map<String, dynamic> json) =>
      _$PubspecEnvironmentFromJson(json);
  const PubspecEnvironment._();
}

final generatePubspec = Query<Unit, Pubspec>(
  'dart.generatePubspec',
  provider: (context, _) {
    return Pubspec(
      name: packageName,
      environment: PubspecEnvironment(dart: '>=2.7.0 <3.0.0'),
    );
  },
);
