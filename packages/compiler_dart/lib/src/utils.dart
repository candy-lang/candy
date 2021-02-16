import 'package:code_builder/code_builder.dart';
// ignore: implementation_imports
import 'package:code_builder/src/visitors.dart';
import 'package:meta/meta.dart';

String mangleName(String name) {
  if (name == 'toString') return 'toString_';
  if (name == 'do') return 'do_';
  return name;
}

class FancyDartEmitter extends DartEmitter {
  FancyDartEmitter(Allocator allocator) : super(allocator);

  static bool _isLambdaBody(Code code) =>
      code is ToCodeExpression && !code.isStatement;
  static bool _isLambdaMethod(Method method) =>
      method.lambda ?? _isLambdaBody(method.body);

  StringSink visitMixin(Mixin spec, [StringSink output]) {
    output ??= StringBuffer();
    output.write('mixin ${spec.name}');
    visitTypeParameters(spec.types.map((r) => r.type), output);
    if (spec.on.isNotEmpty) {
      output
        ..write(' on ')
        ..writeAll(spec.on.map<StringSink>((it) => it.type.accept(this)), ',');
    }
    if (spec.implements.isNotEmpty) {
      output
        ..write(' implements ')
        ..writeAll(
            spec.implements.map<StringSink>((it) => it.type.accept(this)), ',');
    }
    output.write(' {');
    for (final m in spec.methods) {
      visitMethod(m, output);
      if (_isLambdaMethod(m)) {
        output.write(';');
      }
      output.writeln();
    }
    output.writeln(' }');
    return output;
  }

  StringSink visitExtension(Extension spec, [StringSink output]) {
    output ??= StringBuffer();
    output.write('extension');
    if (spec.name != null) output.write(' ${spec.name}');
    visitTypeParameters(spec.types.map((r) => r.type), output);
    output..write(' on ')..write(spec.on.type.accept(this))..write(' {');
    for (final m in spec.methods) {
      visitMethod(m, output);
      if (_isLambdaMethod(m)) {
        output.write(';');
      }
      output.writeln();
    }
    output.writeln(' }');
    return output;
  }
}

class Mixin extends Spec {
  Mixin({
    @required this.name,
    this.types = const [],
    this.on = const [],
    this.implements = const [],
    this.methods = const [],
  });

  final String name;
  final List<Reference> types;

  final List<Reference> on;
  final List<Reference> implements;

  final List<Method> methods;

  @override
  R accept<R>(SpecVisitor<R> visitor, [R context]) {
    if (visitor is! FancyDartEmitter) {
      throw UnimplementedError(
          'Mixin only accepts FancyDartEmitter, not ${visitor.runtimeType}.');
    }
    if (R != StringSink) {
      throw UnimplementedError(
          'Mixin visitor should have StringSink as result type, not $R.');
    }
    return (visitor as FancyDartEmitter).visitMixin(this, context as StringSink)
        as R;
  }
}

class Extension extends Spec {
  Extension({
    this.name,
    this.types = const [],
    @required this.on,
    this.methods = const [],
  });

  final String name;
  final List<Reference> types;
  final Reference on;

  final List<Method> methods;

  @override
  R accept<R>(SpecVisitor<R> visitor, [R context]) {
    if (visitor is! FancyDartEmitter) {
      throw UnimplementedError(
          'Extension only accepts FancyDartEmitter, not ${visitor.runtimeType}.');
    }
    if (R != StringSink) {
      throw UnimplementedError(
          'Extension visitor should have StringSink as result type, not $R.');
    }
    return (visitor as FancyDartEmitter)
        .visitExtension(this, context as StringSink) as R;
  }
}
