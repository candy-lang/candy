import 'package:freezed_annotation/freezed_annotation.dart';

part 'ids.freezed.dart';
part 'ids.g.dart';

@freezed
abstract class PackageId implements _$PackageId {
  const factory PackageId(String name) = _PackageId;
  factory PackageId.fromJson(Map<String, dynamic> json) =>
      _$PackageIdFromJson(json);
  const PackageId._();

  static const core = PackageId('core');
  bool get isCore => this == core;
  bool get isNotCore => !isCore;

  @override
  String toString() => name;
}
