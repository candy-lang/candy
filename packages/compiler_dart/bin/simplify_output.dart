import 'dart:convert';
import 'dart:io';

// A small utility to simplify `toString()` outputs of compiled Candy types.
// Very helpful when working with large CST or AST dumps.
//
// Usage: dart ./bin/simplify_output.dart <output-file>

Future<void> main(List<String> args) async {
  final file = File(args.first);
  var content = await file.readAsString();
  content = content
      .trim()
      .replaceAll('\r', '')
      .replaceAll('\n', '\\n')
      .replaceAll('\\', '\\\\') // Separators from paths on Windows.
      .replaceAllMapped(
        RegExp(
          '("(?:path|value|name|identifier|keyword|punctuation|content|type)":) ([^"{\\d][^,}"]*|["{](?=[,}]))',
        ),
        (it) => '${it[1]} "${it[2]}"',
      )
      .replaceAll('"""', '"\\""');
  final dynamic json = jsonDecode(content);
  final dynamic simplified = simplify(json);
  await file.writeAsString(JsonEncoder.withIndent('  ').convert(simplified));
}

dynamic simplify(dynamic json) {
  if (json is List<dynamic>) {
    return json
        .where(
          (dynamic it) =>
              it is! Map<String, dynamic> ||
              (it['_type'] as String)?.startsWith('None') != true,
        )
        .map<dynamic>(simplify)
        .toList();
  } else if (json is Map<String, dynamic>) {
    if (json['_type'] is String) {
      final type = json['_type'] as String;
      final className =
          type.contains('<') ? type.substring(0, type.indexOf('<')) : type;

      switch (className) {
        case 'None':
          return type;
        case 'Some':
          return simplify(json['value']);
        case 'ArrayList':
          return simplify(json['items']);
        default:
          if (type != null && type.endsWith('Id')) return '<some-id>';
      }
    }

    return json.map<String, dynamic>(
      (k, dynamic v) => MapEntry<String, dynamic>(k, simplify(v)),
    )..removeWhere(
        (key, dynamic value) =>
            key == 'typeParameters' &&
            value is Map<String, dynamic> &&
            value.isEmpty,
      );
  } else {
    return json;
  }
}
