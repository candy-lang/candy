import 'dart:io';

import 'package:args/command_runner.dart';

import 'compiler.dart';

Future<void> main(List<String> arguments) async {
  final runner = CommandRunner<void>('candy', 'CLI-tool for 🍭 Candy.')
    ..addCommand(BuildCommand());

  try {
    await runner.run(arguments);
  } on UsageException catch (e) {
    print(e);
    exit(HttpStatus.badRequest);
  }
}

class BuildCommand extends Command<void> {
  BuildCommand() {
    argParser.addFlag(_optionCandyDirectory);
  }

  static const _optionCandyDirectory = 'core-path';

  @override
  String get name => 'build';

  @override
  String get description => 'Compiles 🍭 Candy source files to Dart.';

  @override
  void run() {
    final rest = argResults.rest;
    if (rest.length != 1) {
      throw UsageException(
        'Please enter the project directory to compile.',
        'candy build .',
      );
    }

    final candyDirectory =
        Directory(argResults[_optionCandyDirectory] as String);
    if (!candyDirectory.existsSync()) {
      throw UsageException(
        "Candy directory `${candyDirectory.absolute.path}` doesn't exist.",
        'candy build ./my_project --candy-directory /candy/directory',
      );
    }

    final projectDirectory = Directory(rest[0]);
    final validationResult =
        SimpleResourceProvider.isValidProjectDirectory(projectDirectory);
    if (validationResult != null) {
      throw UsageException(
        '${projectDirectory.absolute.path} is not a valid project directory:\n$validationResult',
        'candy build ./my_project --candy-directory /candy/directory',
      );
    }

    // compile(
    //   packageName: p.basename(projectDirectory.absolute.path),
    //   candyDirectory: candyDirectory,
    //   projectDirectory: projectDirectory,
    // );
  }
}
