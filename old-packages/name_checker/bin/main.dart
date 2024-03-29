import 'dart:convert';
import 'dart:io';
import 'dart:math';

import 'package:http/http.dart' as http;
import 'package:retry/retry.dart';

// ignore_for_file: avoid_print, avoid_dynamic_calls

// ignore: unreachable_from_main
const names = {
  //
  /* 'aaaaa', */ 'avocado',
  'banana', 'bonbon', 'bort', 'butter',
  'cake', 'chocolate', 'cobalt', 'crepe',
  'dash', 'density', 'dig', 'dust',
  'fir', 'fos',
  /* 'gut', */
  'koalang', 'kom',
  'lustrous',
  'mandel', 'maracuja', 'moss',
  'nuss', 'nut',
  'oase',
  'palm', 'party', 'pecan', 'phos', 'pistachio', 'plan', 'pretzl',
  'sahara', /* 'sand', */ 'sick', 'sphene', 'stig', 'stuff',
  /* 'suricate', */ 'syrup',
  'thing',
  'walnut', /* 'whyyy', */ 'wit', 'wurzel',
  'yam',
};

Future<void> main(List<String> args) async {
  final resultFile = File('result.md');
  if (!resultFile.existsSync()) {
    await resultFile.writeAsString(
      '# Names for Candy\n'
      '\n'
      '| Name | Domain | Domain `-lang` | GitHub | GitHub `-lang` |\n'
      '| :- | :-: | :-: | :-: | :-: |\n',
    );
  }

  final names = <String>{};
  for (final name in _generateNames(200)) {
    if (names.contains(name)) continue;
    names.add(name);

    print('Checking $name…');

    final results = await Future.wait([
      isDomainAvailable(name),
      isDomainAvailable('$name-lang'),
      isGitHubOrganizationAvailable(name),
      isGitHubOrganizationAvailable('$name-lang'),
    ]);
    final domainShort = results[0];
    final domainLong = results[1];
    final gitHubShort = results[2];
    final gitHubLong = results[3];

    await resultFile.writeAsString(
      '| $name | $domainShort | $domainLong | $gitHubShort | $gitHubLong |\n',
      mode: FileMode.append,
    );
  }
}

Iterable<String> _generateNames(int count) sync* {
  final vowels = ['a', 'e', 'i', 'o', 'u'];
  final consonants = [
    'b',
    'c',
    'd',
    'f',
    'g',
    'h',
    'j',
    'k',
    'l',
    'm',
    'n',
    'p',
    'q',
    'r',
    's',
    't',
    'v',
    'w',
    'x',
    'y',
    'z',
  ];

  for (var i = 0; i < count; i++) {
    yield [
      consonants.randomElement,
      vowels.randomElement,
      consonants.randomElement,
    ].join();
  }
}

extension ListExtension<T> on List<T> {
  T get randomElement => this[Random().nextInt(length)];
}

Future<String> isDomainAvailable(String name) async {
  final body = await retry(() async {
    final rawResponse = await http.post(
      Uri.https('domains.google.com', '/v1/Main/FeSearchService/Search'),
      headers: {
        'accept': 'application/json',
        'content-type': 'application/json',
      },
      body: jsonEncode({
        'clientFilters': <String, dynamic>{
          'onlyShowAvailable': true,
          'onlyShowTlds': ['DEV']
        },
        'clientUserSpec': {
          'countryCode': 'US',
          'currencyCode': 'USD',
          'sessionId': '-1725695941',
        },
        'debugType': 'DEBUG_TYPE_NONE',
        'query': '$name.dev',
      }),
    );
    final body = jsonDecode(rawResponse.body.substring(4).trim())
        as Map<String, dynamic>;

    if (body['httpStatus'] == 429) throw Exception('Too many requests');

    return body;
  });

  final result = (body['searchResponse']['results']['result'] as List<dynamic>)
      .cast<Map<String, dynamic>>()
      .singleWhere((it) => it['domainName']['sld'] == name);

  if (result['supportedResultInfo']['availabilityInfo']['availability'] !=
      'AVAILABILITY_AVAILABLE') {
    return '❌';
  }

  final price = (result['supportedResultInfo']['purchaseInfo']['pricing']
          as Map<String, dynamic>)
      .values
      .first['renewPrice']['units'] as String;
  return '$price \$';
}

Future<String> isGitHubOrganizationAvailable(String name) async {
  final response = await http.head(Uri.https('github.com', '/$name'));

  switch (response.statusCode) {
    case HttpStatus.ok:
      return '❌';
    case HttpStatus.notFound:
      return '✅';
    default:
      stderr.writeln('Invalid response: $response');
      return '❓';
  }
}
