import 'package:compiler/compiler.dart';

const dartBuildArtifactPath = 'dart';

extension DartBuildArtifact on PackageId {
  BuildArtifactId get dartBuildArtifactId =>
      BuildArtifactId(this, dartBuildArtifactPath);
}

const dartFileExtension = '.dart';
const pubspecFile = 'pubspec.yaml';
const libDirectoryName = 'lib';
const srcDirectoryName = 'src';
const testDirectoryName = 'test';

const dartCoreUrl = 'dart:core';
const dartIoUrl = 'dart:io';
const dartMathUrl = 'dart:math';
const packageMetaUrl = 'package:meta/meta.dart';
const packagePathUrl = 'package:path/path.dart';
const packageTestUrl = 'package:test/test.dart';
