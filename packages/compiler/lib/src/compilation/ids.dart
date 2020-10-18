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

  // ignore: constant_identifier_names
  static const this_ = PackageId('this');
  bool get isThis => this == this_;

  @override
  String toString() => name;
}
