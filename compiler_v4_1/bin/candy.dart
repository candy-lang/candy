// ignore_for_file: avoid_print

import 'dart:convert';
import 'dart:io';

import 'package:args/args.dart';
import 'package:args/command_runner.dart';
import 'package:compiler_v4_1/context.dart';
import 'package:compiler_v4_1/file.dart';
import 'package:compiler_v4_1/hir/ast_to_hir.dart';
import 'package:compiler_v4_1/hir/hir/to_text.dart';
import 'package:compiler_v4_1/lexer/lexer.dart';
import 'package:compiler_v4_1/lowering/modules.dart';
import 'package:compiler_v4_1/mono/backend/mono_to_c.dart';
import 'package:compiler_v4_1/mono/hir_to_mono.dart';
import 'package:compiler_v4_1/mono/mono/to_text.dart';
import 'package:compiler_v4_1/parser/parser.dart';

Future<void> main(List<String> arguments) async {
  final runner = CommandRunner<void>('candy', 'The 🍭 Candy CLI.')
    ..addCommand(CheckCommand())
    ..addCommand(CompileCommand())
    ..addCommand(DebugCommand());

  try {
    await runner.run(arguments);
  } on UsageException catch (error) {
    stderr.writeln(error);
    exit(64);
  } on Exit catch (error) {
    exit(error.code);
  }
}

/// Thrown to terminate the CLI with a specific exit code.
class Exit implements Exception {
  const Exit(this.code);

  final int code;

  static const fileNotFound = 1;
  static const directoryNotFound = 2;
  static const codeContainsErrors = 3;
  static const goldOutdated = 4;
}

// Loading packages & modules

/// A Candy package is marked by a `_package.candy` file, which is the root
/// folder for resolving imports. Falls back to the file's own directory if
/// no package is found, so single-file invocations still work.
Directory findPackageRoot(File file) {
  for (var directory = file.absolute.parent; ;) {
    if (File('${directory.path}/_package.candy').existsSync()) return directory;
    final parent = directory.parent;
    if (parent.path == directory.path) return file.absolute.parent;
    directory = parent;
  }
}

class LoadedPackage {
  const LoadedPackage(this.context, this.modules);

  final Context context;
  final List<Module> modules;
}

LoadedPackage loadPackageForFile(File file) {
  if (!file.existsSync()) {
    stderr.writeln('The given file does not exist.');
    throw const Exit(Exit.fileNotFound);
  }

  final root = findPackageRoot(file);
  final package = Package(
    name: root.path.split(Platform.pathSeparator).last,
    rootDirectory: root,
  );
  return loadPackage(package, package.discoverFiles());
}

LoadedPackage loadPackage(Package package, List<File> files) {
  final context = Context();
  final modules = <Module>[
    for (final file in files) parseModule(context, package, file),
  ];
  return LoadedPackage(context, modules);
}

Module parseModule(Context context, Package package, File file) {
  final source = file.readAsStringSync();
  final sourceFile = SourceFile(file, source);
  final tokens = Lexer.lex(context, sourceFile, source);
  return Module(
    id: package.moduleIdFor(file),
    file: sourceFile,
    ast: Parser.parse(context, sourceFile, tokens),
  );
}

void printErrors(Context context) {
  for (final error in context.errors) {
    final source = error.span.file.content;
    final start = error.span.span.start.value.clamp(0, source.length);
    final line = '\n'.allMatches(source.substring(0, start)).length + 1;
    stderr.writeln('  ${error.span.file.file.path}:$line: ${error.message}');
  }
}

// `check`

class CheckCommand extends Command<void> {
  @override
  final name = 'check';
  @override
  final description = 'Check a Candy program for obvious errors.';

  @override
  Future<void> run() async {
    final path = _requirePath(argResults!);
    final loaded = loadPackageForFile(File(path));
    astToHir(loaded.context, loaded.modules);

    if (loaded.context.errors.isNotEmpty) {
      printErrors(loaded.context);
      throw const Exit(Exit.codeContainsErrors);
    }
  }
}

// `compile`

class CompileCommand extends Command<void> {
  @override
  final name = 'compile';
  @override
  final description = 'Compile a Candy program to a native executable via C.';

  @override
  Future<void> run() async {
    final path = _requirePath(argResults!);
    final loaded = loadPackageForFile(File(path));
    final hir = astToHir(loaded.context, loaded.modules);

    if (loaded.context.errors.isNotEmpty) {
      printErrors(loaded.context);
      throw const Exit(Exit.codeContainsErrors);
    }

    final mono = hirToMono(hir);
    if (mono == null) {
      stderr.writeln('No `main` function found.');
      throw const Exit(Exit.codeContainsErrors);
    }

    final cCode = monoToC(mono);
    final cPath = '${path.substring(0, path.lastIndexOf('.'))}.c';
    File(cPath).writeAsStringSync(cCode);
    await Process.run('clang-format', ['-i', cPath]);

    final executablePath = path.substring(0, path.lastIndexOf('.'));
    final gcc = await Process.run('gcc', [
      cPath,
      '-O3',
      '-o',
      executablePath,
    ]);
    if (gcc.exitCode != 0) {
      stderr.write(gcc.stderr);
      throw const Exit(Exit.codeContainsErrors);
    }

    print('Done 🎉');
  }
}

String _requirePath(ArgResults results) {
  if (results.rest.isEmpty) {
    stderr.writeln('Please provide a file path.');
    throw const Exit(64);
  }
  return results.rest.first;
}

// `debug`

class DebugCommand extends Command<void> {
  DebugCommand() {
    addSubcommand(DebugAstCommand());
    addSubcommand(DebugHirCommand());
    addSubcommand(DebugMonoCommand());
    addSubcommand(DebugCCommand());
    addSubcommand(DebugGoldCommand());
  }

  @override
  final name = 'debug';
  @override
  final description = 'Debug the Candy compiler itself.';
}

class DebugAstCommand extends Command<void> {
  @override
  final name = 'ast';
  @override
  final description = 'Abstract Syntax Tree';

  @override
  Future<void> run() async {
    final path = _requirePath(argResults!);
    final file = File(path);
    if (!file.existsSync()) {
      stderr.writeln('The given file does not exist.');
      throw const Exit(Exit.fileNotFound);
    }

    final root = findPackageRoot(file);
    final package = Package(
      name: root.path.split(Platform.pathSeparator).last,
      rootDirectory: root,
    );
    final context = Context();
    final module = parseModule(context, package, file);

    print(const JsonEncoder.withIndent('  ').convert(module.ast));
    printErrors(context);
    if (context.errors.isNotEmpty) throw const Exit(Exit.codeContainsErrors);
  }
}

class DebugHirCommand extends Command<void> {
  @override
  final name = 'hir';
  @override
  final description = 'High-Level Intermediate Representation';

  @override
  Future<void> run() async {
    final path = _requirePath(argResults!);
    final loaded = loadPackageForFile(File(path));
    final hir = astToHir(loaded.context, loaded.modules);

    print(hirToText(hir));
    printErrors(loaded.context);
    if (loaded.context.errors.isNotEmpty) {
      throw const Exit(Exit.codeContainsErrors);
    }
  }
}

class DebugMonoCommand extends Command<void> {
  @override
  final name = 'mono';
  @override
  final description = 'Monomorphized Intermediate Representation';

  @override
  Future<void> run() async {
    final path = _requirePath(argResults!);
    final loaded = loadPackageForFile(File(path));
    final hir = astToHir(loaded.context, loaded.modules);

    if (loaded.context.errors.isNotEmpty) {
      printErrors(loaded.context);
      throw const Exit(Exit.codeContainsErrors);
    }

    final mono = hirToMono(hir);
    if (mono == null) {
      stderr.writeln('No `main` function found.');
      throw const Exit(Exit.codeContainsErrors);
    }

    print(monoToText(mono));
  }
}

class DebugCCommand extends Command<void> {
  @override
  final name = 'c';
  @override
  final description = 'The generated C code';

  @override
  Future<void> run() async {
    final path = _requirePath(argResults!);
    final loaded = loadPackageForFile(File(path));
    final hir = astToHir(loaded.context, loaded.modules);

    if (loaded.context.errors.isNotEmpty) {
      printErrors(loaded.context);
      throw const Exit(Exit.codeContainsErrors);
    }

    final mono = hirToMono(hir);
    if (mono == null) {
      stderr.writeln('No `main` function found.');
      throw const Exit(Exit.codeContainsErrors);
    }

    print(monoToC(mono));
  }
}

// `debug gold`

class DebugGoldCommand extends Command<void> {
  DebugGoldCommand() {
    addSubcommand(DebugGoldGenerateCommand());
    addSubcommand(DebugGoldCheckCommand());
  }

  @override
  final name = 'gold';
  @override
  final description =
      'Dump IRs next to the original files to compare outputs of different '
      'compiler versions.';
}

abstract class DebugGoldSubcommand extends Command<void> {
  DebugGoldSubcommand() {
    argParser.addOption(
      'output-directory',
      help:
          'Where to write the golden files. Defaults to `.goldens` inside '
          'the given directory.',
    );
  }

  Directory get directory {
    final path = argResults!.rest.isNotEmpty
        ? argResults!.rest.first
        : Directory.current.path;
    final directory = Directory(path);
    if (!directory.existsSync()) {
      stderr.writeln('${directory.path} is not a directory');
      throw const Exit(Exit.directoryNotFound);
    }
    return directory;
  }

  Directory get outputDirectory {
    final option = argResults!['output-directory'] as String?;
    final result = option != null
        ? Directory(option)
        : Directory('${directory.path}/.goldens');
    result.createSync(recursive: true);
    return result;
  }

  /// Runs the full pipeline for every Candy file in [directory] and calls
  /// [visit] with the rendered text of each IR.
  void visitIrs(
    void Function(File file, String irName, File irFile, String ir) visit,
  ) {
    final directory = this.directory;
    final outputDirectory = this.outputDirectory;
    final package = Package(
      name: directory.path.split(Platform.pathSeparator).last,
      rootDirectory: directory,
    );
    final files = package.discoverFiles();

    for (final file in files) {
      final context = Context();
      final module = parseModule(context, package, file);
      final relative = file.absolute.path.substring(
        directory.absolute.path.length,
      );
      final irDirectory = Directory('${outputDirectory.path}/$relative');
      irDirectory.createSync(recursive: true);

      void visitIr(String irName, String ir) {
        final irFile = File('${irDirectory.path}/$irName.txt');
        visit(file, irName, irFile, ir);
      }

      visitIr('AST', const JsonEncoder.withIndent('  ').convert(module.ast));

      final hirContext = Context();
      final hir = astToHir(hirContext, [module]);
      visitIr('HIR', hirToText(hir));

      if (hirContext.errors.isEmpty) {
        final mono = hirToMono(hir);
        if (mono != null) {
          visitIr('MONO', monoToText(mono));
          visitIr('C', monoToC(mono));
        }
      }
    }
  }
}

class DebugGoldGenerateCommand extends DebugGoldSubcommand {
  @override
  final name = 'generate';
  @override
  final description = 'For each Candy file, generate the IRs next to the file.';

  @override
  Future<void> run() async {
    visitIrs((file, irName, irFile, ir) => irFile.writeAsStringSync(ir));
  }
}

class DebugGoldCheckCommand extends DebugGoldSubcommand {
  @override
  final name = 'check';
  @override
  final description =
      'For each Candy file, check if the IRs next to the file are up-to-date.';

  @override
  Future<void> run() async {
    var didChange = false;
    visitIrs((file, irName, irFile, ir) {
      if (!irFile.existsSync()) {
        print('$irName of ${file.path} does not exist yet');
        didChange = true;
        return;
      }

      final oldIr = irFile.readAsStringSync();
      if (oldIr != ir) {
        didChange = true;
        print('$irName of ${file.path} changed');
      }
    });

    if (didChange) {
      print('❌ Some goldens are outdated');
      throw const Exit(Exit.goldOutdated);
    } else {
      print('✅ All goldens are up-to-date');
    }
  }
}
