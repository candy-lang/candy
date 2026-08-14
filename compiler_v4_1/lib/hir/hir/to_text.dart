import 'package:supernova/supernova.dart';

import 'hir.dart';
import 'ids.dart';

/// Renders [Hir] for `hir_check --dump-hir`, in a stable order.
String hirToText(Hir hir) {
  final builder = _TextBuilder();

  for (final id in hir.typeDeclarations.keys.sorted()) {
    builder.writeTypeDeclaration(hir.typeDeclarations[id]!);
  }
  for (final id in hir.traits.keys.sorted()) {
    builder.writeTrait(hir.traits[id]!);
  }
  hir.impls.forEach(builder.writeImpl);
  for (final id in hir.assignments.keys.sorted()) {
    builder.writeAssignment(hir.assignments[id]!);
  }
  for (final id in hir.functions.keys.sorted()) {
    builder.writeFunction(hir.functions[id]!);
  }

  builder.writeLine('main: ${hir.mainFunction ?? '<none>'}');
  return builder.toString();
}

extension on Iterable<DeclarationId> {
  List<DeclarationId> sorted() => toList()..sort();
}

class _TextBuilder {
  final _buffer = StringBuffer();
  var _indentation = 0;

  void writeLine([String line = '']) {
    if (line.isEmpty) {
      _buffer.writeln();
      return;
    }
    _buffer
      ..write('  ' * _indentation)
      ..writeln(line);
  }

  void writeBlock(String open, void Function() body, String close) {
    writeLine(open);
    _indentation++;
    body();
    _indentation--;
    writeLine(close);
  }

  static String _substitution(MapEntry<TypeParameterId, HirType> entry) =>
      '${entry.key} = ${entry.value}';

  static String typeParameters(List<HirTypeParameter> parameters) =>
      parameters.isEmpty ? '' : '[${parameters.join(', ')}]';

  void writeTypeDeclaration(HirTypeDeclaration declaration) {
    final name = declaration.id.name;
    final parameters = typeParameters(declaration.typeParameters);
    switch (declaration.kind) {
      case HirTypeDeclarationKind_Builtin(:final type):
        writeLine('struct $name$parameters = builtin($type)');
      case HirTypeDeclarationKind_Struct(:final fields):
        if (fields.isEmpty) {
          writeLine('struct $name$parameters {}');
          return;
        }
        writeBlock('struct $name$parameters {', () {
          for (final field in fields) {
            writeLine('${field.name}: ${field.type},');
          }
        }, '}');
      case HirTypeDeclarationKind_Enum(:final variants):
        if (variants.isEmpty) {
          writeLine('enum $name$parameters {}');
          return;
        }
        writeBlock('enum $name$parameters {', () {
          for (final variant in variants) {
            final type = variant.type;
            writeLine(
              type == null ? '${variant.name},' : '${variant.name}: $type,',
            );
          }
        }, '}');
    }
  }

  void writeTrait(HirTraitDefinition trait) {
    final header =
        'trait ${trait.id.name}${typeParameters(trait.typeParameters)}';
    if (trait.functions.isEmpty) {
      writeLine('$header {}');
      return;
    }
    writeBlock('$header {', () {
      for (final id in trait.functions.keys.sorted()) {
        final function = trait.functions[id]!;
        final body = function.body;
        if (body == null) {
          writeLine('$id@${function.signature}');
        } else {
          writeSignatureAndBody(id, '${function.signature}', body);
        }
      }
    }, '}');
  }

  void writeImpl(HirImpl impl) {
    final trait = switch (impl.trait) {
      null => '',
      Some(:final some) => ': $some',
      None() => ': <error>',
    };
    final header =
        'impl${typeParameters(impl.typeParameters)} ${impl.type}$trait';
    if (impl.functions.isEmpty && impl.inherentFunctions.isEmpty) {
      writeLine('$header {}');
      return;
    }
    writeBlock('$header {', () {
      for (final traitFunctionId in impl.functions.keys.sorted()) {
        final function = impl.functions[traitFunctionId]!;
        writeLine('# implements $traitFunctionId');
        writeFunction(function);
      }
      for (final id in impl.inherentFunctions.keys.sorted()) {
        writeFunction(impl.inherentFunctions[id]!);
      }
    }, '}');
  }

  void writeAssignment(HirAssignment assignment) {
    writeSignatureAndBody(
      assignment.id,
      '${assignment.name}: ${assignment.type} =',
      assignment.body,
    );
  }

  void writeFunction(HirFunction function) {
    switch (function.body) {
      case HirBodyOrBuiltin_Builtin(function: final builtin):
        writeLine(
          '${function.signature.id}@${function.signature} = '
          '${builtin.name}',
        );
      case HirBodyOrBuiltin_Body(:final body):
        writeSignatureAndBody(
          function.signature.id,
          '${function.signature}',
          body,
        );
    }
  }

  void writeSignatureAndBody(DeclarationId id, String signature, HirBody body) {
    writeBlock('$id@$signature {', () => writeBodyEntries(body), '}');
  }

  void writeBodyEntries(HirBody body) {
    for (final entry in body.expressions) {
      final name = entry.name == null ? '' : '${entry.name} = ';
      final prefix = '${entry.id}: ${entry.expression.type} = $name';
      writeExpression(prefix, entry.expression.kind);
    }
  }

  void writeExpression(String prefix, HirExpressionKind kind) {
    switch (kind) {
      case HirExpressionKind_Int(:final value):
        writeLine('$prefix$value');
      case HirExpressionKind_Text(:final value):
        writeLine('$prefix"$value"');
      case HirExpressionKind_CreateStruct(:final fields):
        writeLine('$prefix{${fields.join(', ')}}');
      case HirExpressionKind_StructAccess(:final struct, :final field):
        writeLine('$prefix$struct.$field');
      case HirExpressionKind_CreateEnum(:final variant, :final value):
        writeLine('$prefix.$variant${value == null ? '' : '($value)'}');
      case HirExpressionKind_Reference(:final id):
        writeLine('$prefix$id');
      case HirExpressionKind_GlobalReference(:final id):
        writeLine('$prefix$id');
      case HirExpressionKind_Call(
        :final function,
        :final substitutions,
        :final arguments,
      ):
        final substitutionText = substitutions.isEmpty
            ? ''
            : '[${substitutions.entries.map(_substitution).join(', ')}]';
        writeLine('$prefix$function$substitutionText(${arguments.join(', ')})');
      case HirExpressionKind_CallLambda(:final lambda, :final arguments):
        writeLine('$prefix$lambda(${arguments.join(', ')})');
      case HirExpressionKind_Switch(
        :final value,
        :final enumType,
        :final cases,
      ):
        writeBlock('${prefix}switch $value: $enumType {', () {
          for (final case_ in cases) {
            final binding = case_.valueId == null ? '' : '(${case_.valueId})';
            writeBlock(
              '${case_.variant}$binding => {',
              () => writeBodyEntries(case_.body),
              '}',
            );
          }
        }, '}');
      case HirExpressionKind_Lambda(:final parameters, :final body):
        final parameterText = parameters
            .map((it) => '${it.id}: ${it.type} = ${it.name}')
            .join(', ');
        writeBlock(
          '$prefix($parameterText) => {',
          () => writeBodyEntries(body),
          '}',
        );
      case HirExpressionKind_Error():
        writeLine('$prefix<error>');
    }
  }

  @override
  String toString() => _buffer.toString();
}
