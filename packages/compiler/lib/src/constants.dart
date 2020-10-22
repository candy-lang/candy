import 'compilation/hir/ids.dart';
import 'compilation/ids.dart';

const candyFileExtension = '.candy';
const candyspecName = 'candyspec.yml';
const srcDirectoryName = 'src';
const buildDirectoryName = 'build';

// TODO(JonasWanke): load this from pubspec.yaml
final mainModuleId = ModuleId(PackageId.this_, ['main']);
