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
      .replaceAllMapped(
        RegExp(
          '("(?:path|value|name|identifier|keyword|punctuation|content)":) ([^"{\\d][^}"]*|["{](?=[,}]))',
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
            (dynamic it) => it is! Map<String, dynamic> || it['name'] != 'None')
        .map<dynamic>(simplify)
        .toList();
  } else if (json is Map<String, dynamic>) {
    if (json['name'] is String) {
      final name = json['name'] as String;

      final typeParameters = json['typeParameters'] as Map<String, dynamic>;
      final properties = json['properties'] as Map<String, dynamic>;
      switch (name) {
        case 'None':
          return 'None<${typeParameters['Value']}>';
        case 'Some':
          return <String, dynamic>{
            'name': 'Some<${typeParameters['Value']}>',
            'properties': simplify(properties),
          };
        case 'ArrayList':
          return simplify(properties['items']);
        default:
          if (name != null && name.endsWith('Id')) return '<some-id>';
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
