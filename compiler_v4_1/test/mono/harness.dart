import 'package:compiler_v4_1/mono/hir_to_mono.dart';
import 'package:compiler_v4_1/mono/mono/ids.dart';
import 'package:compiler_v4_1/mono/mono/mono.dart';
import 'package:supernova/supernova.dart';
import 'package:test/test.dart';

import '../hir/lowering/harness.dart';

/// Runs the whole pipeline, [preludeSource] included, up to monomorphization.
// ignore: avoid-top-level-members-in-tests
class MonoHarness {
  MonoHarness._(this.mono);

  factory MonoHarness.of(String source) {
    final harness = LoweringHarness.full('$preludeSource\n$source');
    harness.expectNoErrors();

    final mono = hirToMono(harness.hir);
    expect(mono, isNotNull, reason: 'The program has no `main` function.');
    return MonoHarness._(mono!);
  }

  final Mono mono;

  /// The instantiations of every function named [name], e.g. `identity[Int]`.
  List<String> instancesOf(String name) => mono.functions.keys
      .where((it) => it.declaration.name == name)
      .map(describeInstance)
      .sorted();

  MonoFunction instance(String description) {
    final matches = mono.functions.entries
        .where((it) => describeInstance(it.key) == description)
        .toList();
    expect(
      matches,
      hasLength(1),
      reason:
          'Expected exactly one `$description`, but the program has '
          '${mono.functions.keys.map(describeInstance).sorted()}.',
    );
    return matches.single.value;
  }

  /// The instantiations [description]'s body calls, in order, nested bodies
  /// included.
  List<String> callsIn(String description) {
    final calls = <String>[];

    void visitBody(MonoBody body) {
      for (final entry in body.expressions) {
        final kind = entry.expression.kind;
        if (kind is MonoExpressionKind_CallFunction) {
          calls.add(describeInstance(kind.function));
        } else if (kind is MonoExpressionKind_Switch) {
          for (final case_ in kind.cases) {
            visitBody(case_.body);
          }
        } else if (kind is MonoExpressionKind_Lambda) {
          visitBody(kind.body);
        }
      }
    }

    final body = instance(description).body;
    if (body is MonoBodyOrBuiltin_Body) visitBody(body.body);
    return calls;
  }

  /// The only switch in [description]'s body.
  MonoExpressionKind_Switch switchIn(String description) {
    final body = instance(description).body;
    expect(body, isA<MonoBodyOrBuiltin_Body>());

    final found = (body as MonoBodyOrBuiltin_Body).body.expressions
        .map((it) => it.expression.kind)
        .whereType<MonoExpressionKind_Switch>()
        .toList();
    expect(
      found,
      hasLength(1),
      reason: 'Expected exactly one switch in `$description`.',
    );
    return found.single;
  }

  /// The declared types, e.g. `Maybe[Int]`.
  List<String> get typeNames =>
      mono.typeDeclarations.keys.map(describeType).sorted();

  List<String> get assignmentOrder => mono.assignmentInitializationOrder
      .map((it) => mono.assignments[it]!.name)
      .toList();
}

/// `Maybe[Int]` rather than the mangled name.
// ignore: avoid-top-level-members-in-tests
String describeType(MonoType type) => switch (type) {
  MonoType_Named(:final declaration, :final typeArguments) =>
    typeArguments.isEmpty
        ? declaration.name
        : '${declaration.name}[${typeArguments.map(describeType).join(', ')}]',
  MonoType_Function(:final parameterTypes, :final returnType) =>
    '(${parameterTypes.map(describeType).join(', ')}) '
        '${describeType(returnType)}',
  MonoType_Dyn(:final trait, :final typeArguments) =>
    typeArguments.isEmpty
        ? trait.name
        : '${trait.name}[${typeArguments.map(describeType).join(', ')}]',
};

/// `identity[Int]`, or `$impl.toText[Int]` for a function declared in a trait
/// or impl.
// ignore: avoid-top-level-members-in-tests
String describeInstance(MonoFunctionId id) {
  final owner = id.declaration.parent;
  final disambiguator = owner == null || owner.disambiguator == 0
      ? ''
      : '#${owner.disambiguator}';
  final prefix = owner == null ? '' : '${owner.name}$disambiguator.';
  final name = '$prefix${id.declaration.name}';
  return id.typeArguments.isEmpty
      ? name
      : '$name[${id.typeArguments.map(describeType).join(', ')}]';
}
