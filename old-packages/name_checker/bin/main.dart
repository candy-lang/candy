import 'dart:convert';
import 'dart:io';

import 'package:http/http.dart' as http;

const names = {
  // 'aaaaa',
  'avocado',
  'banana',
  'bonbon',
  'bort',
  'butter',
  'cake',
  'chocolate',
  'cobalt',
  'crepe',
  'dash',
  'density',
  'dig',
  'dust',
  'fir',
  'fos',
  // 'gut',
  'koalang',
  'kom',
  'lustrous',
  'mandel',
  'maracuja',
  'moss',
  'nuss',
  'nut',
  'oase',
  'palm',
  'party',
  'pecan',
  'phos',
  'pistachio',
  'plan',
  'pretzl',
  'sahara',
  // 'sand',
  'sick',
  'sphene',
  'stig',
  'stuff',
  // 'suricate',
  'syrup',
  'thing',
  'walnut',
  // 'whyyy',
  'wit',
  'wurzel',
  'yam',
};

Future<void> main(List<String> args) async {
  final result = StringBuffer()
    ..writeln('# Names for Candy')
    ..writeln()
    ..writeln('| Name | Domain | Domain `-lang` | GitHub | GitHub `-lang` |')
    ..writeln('| :- | :-: | :-: | :-: | :-: |');

  for (final name in names) {
    print('Checking $name…');
    result
      ..write('| $name | ')
      ..write(await isDomainAvailable(name))
      ..write(' | ')
      ..write(await isDomainAvailable('$name-lang'))
      ..write(' | ')
      ..write(await isGitHubOrganizationAvailable(name))
      ..write(' | ')
      ..write(await isGitHubOrganizationAvailable('$name-lang'))
      ..writeln(' |');
  }

  await File('result.md').writeAsString(result.toString());
}

Future<String> isDomainAvailable(String name) async {
  final rawResponse = await http.post(
    'https://domains.google.com/v1/Main/FeSearchService/Search?authuser=0',
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
  if (rawResponse.statusCode != HttpStatus.ok) {
    stderr.writeln(rawResponse.body);
    return '❓';
  }

  final response =
      jsonDecode(rawResponse.body.substring(4).trim()) as Map<String, dynamic>;
  final result =
      (response['searchResponse']['results']['result'] as List<dynamic>)
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
  final response = await http.head('https://github.com/$name');

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
