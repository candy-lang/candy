import 'package:supernova/supernova.dart';

import 'ids.dart';
import 'mono.dart';

/// Renders [Mono] for `mono_check --dump-mono`, in a stable order.
String monoToText(Mono mono) {
  final builder = _TextBuilder();

  for (final type in mono.typeDeclarations.keys.sorted()) {
    builder.writeTypeDeclaration(type, mono.typeDeclarations[type]!);
  }
  for (final id in mono.assignmentInitializationOrder) {
    builder.writeAssignment(mono.assignments[id]!);
  }
  for (final id in mono.functions.keys.sorted()) {
    builder.writeFunction(mono.functions[id]!);
  }
  for (final id in mono.vtables.keys.sorted()) {
    builder.writeVtable(id, mono.vtables[id]!);
  }

  builder.writeLine('main: ${mono.mainFunction}');
  return builder.toString();
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

  void writeTypeDeclaration(MonoType type, MonoTypeDeclaration declaration) {
    switch (declaration) {
      case MonoTypeDeclaration_Builtin(type: final builtin):
        writeLine('struct $type = builtin($builtin)');
      case MonoTypeDeclaration_Function(
        :final parameterTypes,
        :final returnType,
      ):
        writeLine(
          'fun type $type = (${parameterTypes.join(', ')}) $returnType',
        );
      case MonoTypeDeclaration_Struct(:final fields):
        if (fields.isEmpty) {
          writeLine('struct $type {}');
          return;
        }
        writeBlock('struct $type {', () {
          for (final field in fields) {
            writeLine('${field.name}: ${field.type},');
          }
        }, '}');
      case MonoTypeDeclaration_Enum(:final variants):
        if (variants.isEmpty) {
          writeLine('enum $type {}');
          return;
        }
        writeBlock('enum $type {', () {
          for (final variant in variants) {
            final type = variant.type;
            writeLine(
              type == null ? '${variant.name},' : '${variant.name}: $type,',
            );
          }
        }, '}');
      case MonoTypeDeclaration_Dyn(:final slots):
        if (slots.isEmpty) {
          writeLine('dyn $type {}');
          return;
        }
        writeBlock('dyn $type {', () {
          for (final slot in slots) {
            writeLine(
              '${slot.name}(${slot.parameterTypes.join(', ')}) '
              '${slot.returnType},',
            );
          }
        }, '}');
    }
  }

  void writeVtable(MonoVtableId id, MonoVtable vtable) {
    writeBlock('vtable $id (type ID ${vtable.typeId}) {', () {
      for (final entry in vtable.entries) {
        writeLine('$entry,');
      }
    }, '}');
  }

  void writeAssignment(MonoAssignment assignment) {
    writeBlock(
      '${assignment.name}@${assignment.id.mangled}: ${assignment.type} = {',
      () => writeBodyEntries(assignment.body),
      '}',
    );
  }

  void writeFunction(MonoFunction function) {
    final parameters = function.parameters
        .map((it) => '${it.id}: ${it.type} = ${it.name}')
        .join(', ');
    final header = 'fun ${function.id}($parameters) ${function.returnType}';
    switch (function.body) {
      case MonoBodyOrBuiltin_Builtin(:final function, :final substitutions):
        final arguments = substitutions.entries
            .map((it) => '${it.key} = ${it.value}')
            .join(', ');
        writeLine(
          '$header = ${function.name}'
          '${substitutions.isEmpty ? '' : '[$arguments]'}',
        );
      case MonoBodyOrBuiltin_Body(:final body):
        writeBlock('$header {', () => writeBodyEntries(body), '}');
      case MonoBodyOrBuiltin_DynDispatch(:final slot):
        writeLine('$header = vtable slot $slot');
    }
  }

  void writeBodyEntries(MonoBody body) {
    for (final entry in body.expressions) {
      final name = entry.name == null ? '' : '${entry.name} = ';
      final prefix = '${entry.id}: ${entry.expression.type} = $name';
      writeExpression(prefix, entry.expression.kind);
    }
  }

  void writeExpression(String prefix, MonoExpressionKind kind) {
    switch (kind) {
      case MonoExpressionKind_Int(:final value):
        writeLine('$prefix$value');
      case MonoExpressionKind_Text(:final value):
        writeLine('$prefix"$value"');
      case MonoExpressionKind_CreateStruct(:final fields):
        writeLine('$prefix{${fields.join(', ')}}');
      case MonoExpressionKind_StructAccess(:final struct, :final field):
        writeLine('$prefix$struct.$field');
      case MonoExpressionKind_CreateEnum(:final variant, :final value):
        writeLine('$prefix.$variant${value == null ? '' : '($value)'}');
      case MonoExpressionKind_LocalReference(:final id):
        writeLine('$prefix$id');
      case MonoExpressionKind_GlobalAssignmentReference(:final id):
        writeLine('$prefix${id.mangled}');
      case MonoExpressionKind_CallFunction(:final function, :final arguments):
        writeLine('$prefix$function(${arguments.join(', ')})');
      case MonoExpressionKind_CallLambda(:final lambda, :final arguments):
        writeLine('$prefix$lambda(${arguments.join(', ')})');
      case MonoExpressionKind_Switch(
        :final value,
        :final enumType,
        :final cases,
      ):
        writeBlock('${prefix}switch $value: $enumType {', () {
          for (final case_ in cases) {
            final binding = switch (case_.value) {
              null => '',
              (final id, final type) => '($id: $type)',
            };
            writeBlock(
              '${case_.variant}$binding => {',
              () => writeBodyEntries(case_.body),
              '}',
            );
          }
        }, '}');
      case MonoExpressionKind_Lambda(:final parameters, :final body):
        final parameterText = parameters
            .map((it) => '${it.id}: ${it.type} = ${it.name}')
            .join(', ');
        writeBlock(
          '$prefix($parameterText) => {',
          () => writeBodyEntries(body),
          '}',
        );
      case MonoExpressionKind_ToDyn(:final value, :final vtable):
        writeLine(
          '${prefix}toDyn $value${vtable == null ? '' : ' via $vtable'}',
        );
    }
  }

  @override
  String toString() => _buffer.toString();
}
