import 'package:supernova/supernova.dart' hide IterableSorted;

import '../../hir/hir/builtins.dart';
import '../../hir/hir/ids.dart';
import '../../position.dart';
import '../hir_to_mono.dart';
import '../mono/ids.dart';
import '../mono/mono.dart';
import 'type_layout.dart';

String monoToC(Mono mono) {
  final context = _Context(mono);
  context._lowerMono();
  return context._c.toString();
}

class _Context {
  _Context(this.mono) : layouts = TypeLayouts.compute(mono.typeDeclarations);

  final Mono mono;
  final TypeLayouts layouts;
  final _c = StringBuffer();

  void _push(String s) => _c.write(s);

  void _pushDeclarationComment(String name, SourceSpan? span) {
    if (span == null) return;
    _push('// $name\n// ${span.startLocation}\n');
  }

  /// The un-mangled Candy type [type] was mangled from, e.g.
  /// `(Core:List[Core:Text], Core:Int, Core:Int) Core:List[Core:Text]`.
  String _readableType(MonoType type) => switch (type) {
    MonoType_Named(:final declaration, :final typeArguments) => _readableName(
      declaration,
      typeArguments,
    ),
    MonoType_Function(:final parameterTypes, :final returnType) =>
      '(${parameterTypes.map(_readableType).join(', ')}) '
          '${_readableType(returnType)}',
    MonoType_Dyn(:final trait, :final typeArguments) => _readableName(
      trait,
      typeArguments,
    ),
  };

  /// The un-mangled Candy name of a declaration instantiated with
  /// [typeArguments], e.g. `Core:print[Core:List[Core:Int]]`.
  String _readableName(
    DeclarationId declaration,
    List<MonoType> typeArguments,
  ) => typeArguments.isEmpty
      ? '$declaration'
      : '$declaration[${typeArguments.map(_readableType).join(', ')}]';

  /// The un-mangled Candy type of a lambda, e.g. `(Core:Int) Core:Int`.
  String _readableLambdaType(MonoExpressionKind_Lambda lambda) =>
      '(${lambda.parameters.map((it) => _readableType(it.type)).join(', ')}) '
      '${_readableType(lambda.body.returnType)}';

  Set<String> _boxedVariants(MonoType enumType) =>
      layouts.boxedVariants[enumType] ?? const {};

  /// Whether [type] is uninhabited, i.e. a zero-variant enum like `Never`.
  /// Expressions of such a type never produce a value, so the generated C
  /// never needs to store their result.
  bool _isUninhabited(MonoType type) {
    final declaration = mono.typeDeclarations[type];
    return declaration is MonoTypeDeclaration_Enum &&
        declaration.variants.isEmpty;
  }

  void _lowerMono() {
    _push('#include <errno.h>\n');
    _push('#include <stdint.h>\n');
    _push('#include <stdio.h>\n');
    _push('#include <stdlib.h>\n');
    _push('#include <string.h>\n\n');

    _push('#include <cairo.h>\n\n');
    _push('#include <SDL3/SDL.h>\n\n');
    // if (mono.typeDeclarations.keys.any(
    //   (it) => it is MonoType_Named && it.declaration.name == 'GlfwWindow',
    // )) {
    //   // _push('#include <GLFW/glfw3.h>\n\n');

    // }

    _push('/// Type Forward Declarations\n\n');
    _lowerTypeForwardDeclarations();
    _push('\n');

    _push('/// Type Declarations\n\n');
    _lowerTypeDeclarations();
    _push('\n');

    _push('/// Assignment Declarations\n\n');
    _lowerAssignmentDeclarations();
    _push('\n');

    _push('/// Function Declarations\n\n');
    _lowerFunctionDeclarations();
    _push('\n');

    _push('/// Vtables\n\n');
    _lowerVtables();
    _push('\n');

    _push('/// Assignment Definitions\n\n');
    _lowerAssignmentDefinitions();
    _push('\n');

    _push('/// Function Definitions\n\n');
    _lowerFunctionDefinitions();

    _push('int main() {\n');
    _push(r'''

    const int width = 800;
    const int height = 600;

    if (!SDL_Init(SDL_INIT_VIDEO)) {
        fprintf(stderr, "SDL_Init: %s\n", SDL_GetError());
        return 1;
    }

    SDL_Window *window = NULL;
    SDL_Renderer *renderer = NULL;

    if (!SDL_CreateWindowAndRenderer(
            "My language",
            width,
            height,
            SDL_WINDOW_RESIZABLE,
            &window,
            &renderer
    )) {
        fprintf(stderr, "SDL_CreateWindowAndRenderer: %s\n", SDL_GetError());
        SDL_Quit();
        return 1;
    }

    /*
     * Cairo renders into this memory buffer.
     */
    cairo_surface_t *surface =
        cairo_image_surface_create(CAIRO_FORMAT_ARGB32, width, height);

    cairo_t *cr = cairo_create(surface);

    /*
     * SDL texture receives Cairo's pixels.
     */
    SDL_Texture *texture = SDL_CreateTexture(
        renderer,
        SDL_PIXELFORMAT_ARGB8888,
        SDL_TEXTUREACCESS_STREAMING,
        width,
        height
    );
    if (!texture) {
        fprintf(stderr, "SDL_CreateTexture: %s\n", SDL_GetError());
        return 1;
    }

    bool running = true;

    while (running) {
        /*
         * 1. Handle input/events.
         */
        SDL_Event event;

        while (SDL_PollEvent(&event)) {
            switch (event.type) {
                case SDL_EVENT_QUIT:
                    running = false;
                    break;

                case SDL_EVENT_KEY_DOWN:
                    printf("key down: %s\n",
                           SDL_GetKeyName(event.key.key));
                    break;

                case SDL_EVENT_MOUSE_MOTION:
                    printf("mouse: %.1f %.1f\n",
                           event.motion.x,
                           event.motion.y);
                    break;

                case SDL_EVENT_MOUSE_BUTTON_DOWN:
                    printf("mouse button: %d\n",
                           event.button.button);
                    break;
            }
        }

        /*
         * 2. Render your display list with Cairo.
         */

        cairo_set_source_rgb(cr, 0.1, 0.1, 0.1);
        cairo_paint(cr);

        cairo_set_source_rgb(cr, 0.2, 0.5, 1.0);
        cairo_rectangle(cr, 100, 100, 300, 200);
        cairo_fill(cr);

        cairo_set_source_rgb(cr, 1.0, 1.0, 1.0);
        cairo_set_font_size(cr, 32);
        cairo_move_to(cr, 140, 220);
        cairo_show_text(cr, "Hello");

        cairo_surface_flush(surface);

        /*
         * 3. Copy Cairo's pixels into the SDL texture.
         */
        SDL_UpdateTexture(
            texture,
            NULL,
            cairo_image_surface_get_data(surface),
            cairo_image_surface_get_stride(surface)
        );

        /*
         * 4. Present texture in the window.
         */
        SDL_SetRenderDrawColor(renderer, 0, 0, 0, 255);
        SDL_RenderClear(renderer);

        SDL_RenderTexture(renderer, texture, NULL, NULL);

        SDL_RenderPresent(renderer);
    }

    cairo_destroy(cr);
    cairo_surface_destroy(surface);

    SDL_DestroyTexture(texture);
    SDL_DestroyRenderer(renderer);
    SDL_DestroyWindow(window);

    SDL_Quit();
''');
    for (final id in mono.assignmentInitializationOrder) {
      _push('${id.mangled}\$init();\n');
    }
    _push('return ${mono.mainFunction}().value;\n}\n');
  }

  void _lowerTypeForwardDeclarations() {
    for (final type in mono.typeDeclarations.keys) {
      _push('typedef struct $type $type;\n');
      if (type is MonoType_Dyn) {
        _push('typedef struct ${_vtableType(type)} ${_vtableType(type)};\n');
      }
    }
  }

  String _vtableType(MonoType_Dyn type) => '$type\$vtable';

  void _lowerTypeDeclarations() {
    for (final type in layouts.declarationOrder) {
      final declaration = mono.typeDeclarations[type]!;
      if (declaration is MonoTypeDeclaration_Function ||
          declaration is MonoTypeDeclaration_Dyn) {
        // Lambda types and trait objects have no declaration in the source to
        // point to, so print the type they were mangled from instead.
        _push('// ${_readableType(type)}\n');
      } else {
        _pushDeclarationComment(_readableType(type), declaration.span);
      }
      _push('struct $type {\n');
      switch (declaration) {
        case MonoTypeDeclaration_Builtin(
          type: final builtinType,
          :final typeArguments,
        ):
          switch (builtinType) {
            case .int:
              _push('int64_t value;\n');
            case .list:
              _push('uint64_t length;\n');
              _push('${typeArguments.first}* values;\n');
            case .text:
              _push('char* value;\n');
          }
          _push('};\n');
        case MonoTypeDeclaration_Struct(:final fields):
          for (final field in fields) {
            _push('// ${field.name}: ${_readableType(field.type)}\n');
            _push('${field.type} ${field.name};\n');
          }
          _push('};\n');
        case MonoTypeDeclaration_Enum(:final variants):
          if (variants.isNotEmpty) {
            _push('enum {');
            for (final variant in variants) {
              _push('${type}_${variant.name},');
            }
            _push('} variant;\n');
          }

          _push('union {\n');
          final boxedVariants = _boxedVariants(type);
          for (final variant in variants) {
            final valueType = variant.type;
            if (valueType != null) {
              final pointer = boxedVariants.contains(variant.name) ? '*' : '';
              _push('// ${variant.name}: ${_readableType(valueType)}\n');
              _push('$valueType$pointer ${variant.name};\n');
            }
          }
          _push('} value;\n};\n');
        case MonoTypeDeclaration_Function(
          :final parameterTypes,
          :final returnType,
        ):
          _push('void* closure;\n');
          _push('$returnType (*function)(void*');
          for (final parameterType in parameterTypes) {
            _push(', $parameterType');
          }
          _push(');\n');
          _push('};\n');
        case MonoTypeDeclaration_Dyn(:final slots):
          final vtableType = _vtableType(type as MonoType_Dyn);
          _push('void* data;\n');
          _push('const $vtableType* vtable;\n');
          _push('};\n');

          // The type ID comes first in every vtable, so any of them can be
          // read as an `Any` vtable.
          _push('struct $vtableType {\n');
          _push('int64_t typeId;\n');
          for (final (index, slot) in slots.indexed) {
            _push('// ${slot.name}\n');
            _push('${slot.returnType} (*slot$index)(void*');
            for (final parameterType in slot.parameterTypes) {
              _push(', $parameterType');
            }
            _push(');\n');
          }
          _push('};\n');
      }
    }
  }

  void _lowerAssignmentDeclarations() {
    for (final id in mono.assignmentInitializationOrder) {
      final assignment = mono.assignments[id]!;
      _lowerLambdaDeclarationsIn(id.mangled, assignment.body);
      _pushDeclarationComment('$id', assignment.span);
      _push('${assignment.type} ${id.mangled};\n');
    }
  }

  void _lowerAssignmentDefinitions() {
    for (final id in mono.assignmentInitializationOrder) {
      final assignment = mono.assignments[id]!;
      _lowerLambdaDefinitionsIn(id.mangled, const [], assignment.body);

      _pushDeclarationComment('$id', assignment.span);
      _push('void ${id.mangled}\$init() {\n');
      _lowerBodyExpressions(id.mangled, assignment.body);
      _push('${id.mangled} = ${assignment.body.returnValueId};\n}\n\n');
    }
  }

  void _lowerFunctionDeclarations() {
    for (final MapEntry(key: id, value: function) in mono.functions.entries) {
      final body = function.body;
      if (body is MonoBodyOrBuiltin_Body) {
        _lowerLambdaDeclarationsIn(id.toString(), body.body);
      }

      _pushDeclarationComment(
        _readableName(id.declaration, id.typeArguments),
        function.span,
      );
      _lowerFunctionSignature(id.toString(), function);
      _push(';\n');
    }
  }

  void _lowerLambdaDeclarationsIn(String declarationName, MonoBody body) {
    _visitLambdasInsideBody(body, (id, lambda) {
      _push(
        'typedef struct $declarationName\$lambda${id}_closure '
        '$declarationName\$lambda${id}_closure;\n',
      );

      _push('// ${_readableLambdaType(lambda)}\n');
      _lowerLambdaSignature(declarationName, id, lambda);
      _push(';\n');
    });
  }

  void _lowerVtables() {
    final ids = mono.vtables.keys.sorted();
    for (final id in ids) {
      _push(
        'static const ${_vtableType(id.dynType)} $id;'
        ' // ${_readableType(id.concreteType)} as '
        '${_readableType(id.dynType)}\n',
      );
    }
    for (final id in ids) {
      _lowerVtable(id, mono.vtables[id]!);
    }
  }

  /// Each slot gets a thunk that unpacks the receiver from its pointer, and
  /// boxes a `Self` result back into the object.
  void _lowerVtable(MonoVtableId id, MonoVtable vtable) {
    final declaration =
        mono.typeDeclarations[id.dynType]! as MonoTypeDeclaration_Dyn;
    final concreteType = id.concreteType;

    for (final (index, slot) in declaration.slots.indexed) {
      final entry = vtable.entries[index];
      final parameters = [
        for (final (index, type) in slot.parameterTypes.indexed)
          (type, 'parameter$index'),
      ];
      final call =
          '$entry(*($concreteType*) self'
          '${parameters.map((it) => ', ${it.$2}').join()})';

      _push('static ${slot.returnType} $id\$slot$index(void* self');
      for (final (type, name) in parameters) {
        _push(', $type $name');
      }
      _push(') {\n');
      if (slot.returnType == id.dynType &&
          mono.functions[entry]!.returnType != id.dynType) {
        _push('''
$concreteType* data = malloc(sizeof($concreteType));
*data = $call;
return (${id.dynType}) {.data = data, .vtable = &$id};
''');
      } else {
        _push('return $call;\n');
      }
      _push('}\n');
    }

    _push(
      'static const ${_vtableType(id.dynType)} $id = {\n'
      '.typeId = ${vtable.typeId},\n',
    );
    for (var index = 0; index < declaration.slots.length; index++) {
      _push('.slot$index = $id\$slot$index,\n');
    }
    _push('};\n\n');
  }

  void _lowerFunctionDefinitions() {
    for (final MapEntry(key: id, value: function) in mono.functions.entries) {
      final body = function.body;
      if (body is MonoBodyOrBuiltin_Body) {
        _lowerLambdaDefinitionsIn(
          id.toString(),
          function.parameters,
          body.body,
        );
      }

      _pushDeclarationComment(
        _readableName(id.declaration, id.typeArguments),
        function.span,
      );
      _lowerFunctionSignature(id.toString(), function);
      _push(' {\n');
      _lowerBodyOrBuiltin(id.toString(), function);
      _push('}\n\n');
    }
  }

  void _lowerLambdaDefinitionsIn(
    String declarationName,
    List<MonoParameter> declarationParameters,
    MonoBody body,
  ) {
    _visitLambdasInsideBody(body, (id, lambda) {
      final closure = _closureWithTypes(lambda, declarationParameters, body);

      _push('struct $declarationName\$lambda${id}_closure {\n');
      closure.forEach(
        (id, type) => _push('$type $id; // $id: ${_readableType(type)}\n'),
      );
      _push('};\n');

      _push('// ${_readableLambdaType(lambda)}\n');
      _lowerLambdaSignature(declarationName, id, lambda);
      _push(' {\n');
      _push('$declarationName\$lambda${id}_closure* closure = raw_closure;\n');
      closure.forEach((id, type) => _push('$type $id = closure->$id;\n'));
      _lowerBody(declarationName, lambda.body);
      _push('}\n');
    });
  }

  void _lowerFunctionSignature(String name, MonoFunction function) {
    _push('${function.returnType} $name(');
    for (final (index, parameter) in function.parameters.indexed) {
      if (index > 0) _push(', ');
      _push('${parameter.type} ${parameter.id}');
    }
    _push(')');
  }

  void _lowerLambdaSignature(
    String declarationName,
    HirId id,
    MonoExpressionKind_Lambda lambda,
  ) {
    _push(
      // ignore: missing_whitespace_between_adjacent_strings
      '${lambda.body.returnType} $declarationName\$lambda${id}_function('
      'void* raw_closure',
    );
    for (final parameter in lambda.parameters) {
      _push(', ${parameter.type} ${parameter.id}');
    }
    _push(')');
  }

  static void _visitLambdasInsideBody(
    MonoBody body,
    void Function(HirId id, MonoExpressionKind_Lambda lambda) visitor,
  ) {
    for (final entry in body.expressions) {
      switch (entry.expression.kind) {
        case MonoExpressionKind_Int():
        case MonoExpressionKind_Text():
        case MonoExpressionKind_CreateStruct():
        case MonoExpressionKind_StructAccess():
        case MonoExpressionKind_CreateEnum():
        case MonoExpressionKind_GlobalAssignmentReference():
        case MonoExpressionKind_LocalReference():
        case MonoExpressionKind_CallFunction():
        case MonoExpressionKind_CallLambda():
        case MonoExpressionKind_ToDyn():
          break;
        case MonoExpressionKind_Switch(:final cases):
          for (final case_ in cases) {
            _visitLambdasInsideBody(case_.body, visitor);
          }
        case final MonoExpressionKind_Lambda lambda:
          _visitLambdasInsideBody(lambda.body, visitor);
          visitor(entry.id, lambda);
      }
    }
  }

  void _lowerBodyOrBuiltin(String declarationName, MonoFunction function) {
    final body = function.body;
    switch (body) {
      case MonoBodyOrBuiltin_Builtin(
        function: final builtinFunction,
        :final substitutions,
      ):
        _push('// builtin function\n');
        _lowerBuiltinFunction(builtinFunction, function, substitutions);
      case MonoBodyOrBuiltin_Body(:final body):
        _lowerBody(declarationName, body);
      case MonoBodyOrBuiltin_DynDispatch(:final slot):
        final [self, ...arguments] = function.parameters;
        _push('return ${self.id}.vtable->slot$slot(${self.id}.data');
        for (final argument in arguments) {
          _push(', ${argument.id}');
        }
        _push(');');
    }
  }

  void _lowerBody(String declarationName, MonoBody body) {
    _lowerBodyExpressions(declarationName, body);
    _push('return ${body.returnValueId};');
  }

  void _lowerBodyExpressions(String declarationName, MonoBody body) {
    for (final entry in body.expressions) {
      if (entry.name != null) _push('// ${entry.name}\n');

      _lowerExpression(declarationName, entry.id, entry.expression);
      if (_isUninhabited(entry.expression.type)) {
        _push('// Returns `Never`\n');
      }
      _push('\n\n');
    }
  }

  void _lowerExpression(
    String declarationName,
    HirId id,
    MonoExpression expression,
  ) {
    final type = expression.type;
    switch (expression.kind) {
      case MonoExpressionKind_Int(:final value):
        _push('$type $id = {.value = $value };');
      case MonoExpressionKind_Text(:final value):
        _push('$type $id = {.value = "${_escapeCString(value)}"};');
      case MonoExpressionKind_CreateStruct(:final fields):
        final declaration =
            mono.typeDeclarations[type]! as MonoTypeDeclaration_Struct;
        _push('$type $id = {');
        for (final (typeField, valueId) in declaration.fields.zip(fields)) {
          _push('\n.${typeField.name} = $valueId,');
        }
        _push('};');
      case MonoExpressionKind_StructAccess(:final struct, :final field):
        _push('$type $id = $struct.$field;');
      case MonoExpressionKind_CreateEnum(:final variant, :final value):
        if (_boxedVariants(type).contains(variant)) {
          final declaration =
              mono.typeDeclarations[type]! as MonoTypeDeclaration_Enum;
          final valueType = declaration.variants
              .firstWhere((it) => it.name == variant)
              .type!;
          _push('''
$valueType* ${id}_value_boxed = malloc(sizeof($valueType*));
*${id}_value_boxed = $value;
$type $id = {
  .variant = ${type}_$variant,
  .value.$variant = ${id}_value_boxed,
};''');
        } else {
          _push('$type $id = {.variant = ${type}_$variant');
          if (value != null) _push('\n,.value.$variant = $value');
          _push('};');
        }
      case MonoExpressionKind_GlobalAssignmentReference(id: final referenced):
        _push('$type $id = ${referenced.mangled};');
      case MonoExpressionKind_LocalReference(id: final referencedId):
        _push('$type $id = $referencedId;');
      case MonoExpressionKind_CallFunction(:final function, :final arguments):
        _push('$type $id = $function(${arguments.join(', ')});');
      case MonoExpressionKind_CallLambda(:final lambda, :final arguments):
        _push('$type $id = $lambda.function($lambda.closure');
        for (final argument in arguments) {
          _push(', $argument');
        }
        _push(');');
      case MonoExpressionKind_Switch(
        :final value,
        :final enumType,
        :final cases,
      ):
        _push('$type $id;\n');

        _push('switch ($value.variant) {');
        for (final case_ in cases) {
          _push('case ${enumType}_${case_.variant}:\n');
          final binding = case_.value;
          if (binding != null) {
            final (valueId, valueType) = binding;
            final pointer = _boxedVariants(enumType).contains(case_.variant)
                ? '*'
                : '';
            _push(
              '$valueType $valueId = $pointer$value.value.${case_.variant};\n',
            );
          }

          _lowerBodyExpressions(declarationName, case_.body);
          if (_isUninhabited(case_.body.returnType)) {
            _push('// Returns `Never`\n');
          } else {
            _push('$id = ${case_.body.returnValueId};\n');
          }

          _push('break;');
        }
        _push('}');
      case final MonoExpressionKind_Lambda lambda:
        _push(
          '$declarationName\$lambda${id}_closure* ${id}_closure = '
          'malloc(sizeof($declarationName\$lambda${id}_closure));\n',
        );
        for (final referencedId in _closure(lambda).sorted()) {
          _push('${id}_closure->$referencedId = $referencedId;\n');
        }
        _push('''
$type $id = {
  .closure = ${id}_closure,
  .function = $declarationName\$lambda${id}_function,
};''');
      case MonoExpressionKind_ToDyn(:final value, vtable: null):
        _push(
          '$type $id = {.data = $value.data, '
          '.vtable = (const ${_vtableType(type as MonoType_Dyn)}*) '
          '$value.vtable};',
        );
      case MonoExpressionKind_ToDyn(:final value, vtable: final vtable?):
        final concreteType = vtable.concreteType;
        _push('''
$concreteType* ${id}_data = malloc(sizeof($concreteType));
*${id}_data = $value;
$type $id = {.data = ${id}_data, .vtable = &$vtable};''');
    }
  }

  // Closure analysis

  Set<HirId> _closure(MonoExpressionKind_Lambda lambda) {
    final defined = {for (final parameter in lambda.parameters) parameter.id};
    final referenced = <HirId>{};
    _collectDefinedAndReferencedIds(lambda.body, defined, referenced);
    referenced.removeWhere(defined.contains);
    return referenced;
  }

  Map<HirId, MonoType> _closureWithTypes(
    MonoExpressionKind_Lambda lambda,
    List<MonoParameter> declarationParameters,
    MonoBody declarationBody,
  ) {
    return {
      for (final id in _closure(lambda))
        id: _typeOfDeclarationId(id, declarationParameters, declarationBody),
    };
  }

  MonoType _typeOfDeclarationId(
    HirId id,
    List<MonoParameter> declarationParameters,
    MonoBody declarationBody,
  ) {
    for (final parameter in declarationParameters) {
      if (parameter.id == id) return parameter.type;
    }
    final found = _findExpression(declarationBody, id);
    if (found == null) {
      throw StateError(
        "Couldn't find expression $id in declaration body $declarationBody",
      );
    }
    return found;
  }

  void _collectDefinedAndReferencedIds(
    MonoBody body,
    Set<HirId> definedIds,
    Set<HirId> referencedIds,
  ) {
    for (final entry in body.expressions) {
      definedIds.add(entry.id);
      switch (entry.expression.kind) {
        case MonoExpressionKind_Int():
        case MonoExpressionKind_Text():
          break;
        case MonoExpressionKind_CreateStruct(:final fields):
          referencedIds.addAll(fields);
        case MonoExpressionKind_StructAccess(:final struct):
          referencedIds.add(struct);
        case MonoExpressionKind_CreateEnum(:final value):
          if (value != null) referencedIds.add(value);
        case MonoExpressionKind_GlobalAssignmentReference():
          break;
        case MonoExpressionKind_LocalReference(:final id):
          referencedIds.add(id);
        case MonoExpressionKind_ToDyn(:final value):
          referencedIds.add(value);
        case MonoExpressionKind_CallFunction(:final arguments):
          referencedIds.addAll(arguments);
        case MonoExpressionKind_CallLambda(:final lambda, :final arguments):
          referencedIds.add(lambda);
          referencedIds.addAll(arguments);
        case MonoExpressionKind_Switch(:final value, :final cases):
          referencedIds.add(value);
          for (final case_ in cases) {
            final binding = case_.value;
            if (binding != null) referencedIds.add(binding.$1);
            _collectDefinedAndReferencedIds(
              case_.body,
              definedIds,
              referencedIds,
            );
          }
        case final MonoExpressionKind_Lambda lambda:
          definedIds.addAll(lambda.parameters.map((it) => it.id));
          _collectDefinedAndReferencedIds(
            lambda.body,
            definedIds,
            referencedIds,
          );
      }
    }
  }

  /// The type of the expression (or switch-case binding, or lambda
  /// parameter) with the given [id], searching recursively through nested
  /// switch cases and lambdas.
  MonoType? _findExpression(MonoBody body, HirId id) {
    for (final entry in body.expressions) {
      if (entry.id == id) return entry.expression.type;

      switch (entry.expression.kind) {
        case MonoExpressionKind_Switch(:final cases):
          for (final case_ in cases) {
            final binding = case_.value;
            if (binding != null && binding.$1 == id) return binding.$2;
            final found = _findExpression(case_.body, id);
            if (found != null) return found;
          }
        case final MonoExpressionKind_Lambda lambda:
          for (final parameter in lambda.parameters) {
            if (parameter.id == id) return parameter.type;
          }
          final found = _findExpression(lambda.body, id);
          if (found != null) return found;
        default:
          break;
      }
    }
    return null;
  }

  // Builtin functions

  final _builtinTypeCache = <BuiltinType, MonoType>{};

  /// The program's single declaration of a builtin type (e.g. `Text`), for
  /// use in codegen that fabricates a value out of thin air (an error
  /// message, a loop index) rather than reusing a parameter's or the
  /// function's own type.
  MonoType _builtinType(BuiltinType builtin) => _builtinTypeCache.putIfAbsent(
    builtin,
    () => mono.typeDeclarations.entries.firstWhere((entry) {
      final declaration = entry.value;
      return declaration is MonoTypeDeclaration_Builtin &&
          declaration.type == builtin;
    }).key,
  );

  /// The backend fabricates calls to `panic` for cases the program itself
  /// never reaches (e.g. an out-of-bounds list access), so [hirToMono] force
  /// includes it even when nothing calls it.
  String get _panicFunction => BuiltinFunction.panic.id.mangled;

  /// A `Maybe.some` of [maybeType] holding [value], boxed if the variant is.
  String _some(MonoType maybeType, MonoType valueType, String value) {
    if (!_boxedVariants(maybeType).contains('some')) {
      return '($maybeType) {.variant = ${maybeType}_some, '
          '.value.some = $value}';
    }
    return '({ $valueType* boxed = malloc(sizeof($valueType)); '
        '*boxed = $value; '
        '($maybeType) {.variant = ${maybeType}_some, .value.some = boxed}; })';
  }

  void _lowerBuiltinFunction(
    BuiltinFunction builtinFunction,
    MonoFunction function,
    Map<TypeParameterId, MonoType> substitutions,
  ) {
    MonoType itemType() => substitutions.values.single;
    final parameters = function.parameters;
    final returnType = function.returnType;
    final textType = _builtinType(BuiltinType.text);
    final intType = _builtinType(BuiltinType.int);

    switch (builtinFunction) {
      //       case .glfwInit:
      //         _push('''
      // if (!glfwInit()) {
      //     $_panicFunction((($textType) {.value = "Failed to initialize GLFW."}));
      // }

      // glfwSetErrorCallback((GLFWerrorfun) (int error, const char* description) {
      //     char* message_format = "GLFW error %d: %s";
      //     int length = snprintf(NULL, 0, message_format, error, description);
      //     $textType message = {.value = malloc(length + 1)};
      //     snprintf(message.value, length + 1, message_format, error, description);
      //     ${BuiltinFunction.print.id.mangled}(message);
      // });
      // ''');
      //       case .glfwCreateWindow:
      //         _push('''
      // glfwWindowHint(GLFW_CONTEXT_VERSION_MAJOR, 3);
      // glfwWindowHint(GLFW_CONTEXT_VERSION_MINOR, 3);
      // glfwWindowHint(GLFW_OPENGL_PROFILE, GLFW_OPENGL_CORE_PROFILE);
      // GLFWwindow* window = glfwCreateWindow(
      //     ${parameters[0].id}.value,
      //     ${parameters[1].id}.value,
      //     ${parameters[2].id}.value,
      //     ${parameters[3].id}.value
      // );
      // if (window == NULL) {
      //     $_panicFunction((($textType) {.value = "Failed to create GLFW window."}));
      // }

      // glfwMakeContextCurrent(window);
      // return window;''');
      case .typeId:
        _push('''
$returnType result = {{.value = ${mono.typeIds[itemType()]}}};
return result;''');
      case .typeIdOf:
        final value = parameters.single.id;
        final id = itemType() is MonoType_Dyn
            ? '$value.vtable->typeId'
            : '${mono.typeIds[itemType()]}';
        _push('''
$returnType result = {{.value = $id}};
return result;''');
      case .downcast:
        final target = substitutions.entries
            .firstWhere((it) => it.key.name == 'T')
            .value;
        final source = parameters.single.type;
        final value = parameters.single.id;
        final none = '($returnType) {.variant = ${returnType}_none}';
        if (source == target) {
          _push('return ${_some(returnType, target, '$value')};');
        } else if (source is MonoType_Dyn) {
          _push('''
if ($value.vtable->typeId == ${mono.typeIds[target]}) {
    return ${_some(returnType, target, '*($target*) $value.data')};
}
return $none;''');
        } else {
          _push('return $none;');
        }
      case .intAdd:
        _push('''
$returnType result = {.value = ${parameters[0].id}.value + ${parameters[1].id}.value};
return result;''');
      case .intBitwiseAnd:
        _push('''
$returnType result = {.value = ${parameters[0].id}.value & ${parameters[1].id}.value};
return result;''');
      case .intBitwiseOr:
        _push('''
$returnType result = {.value = ${parameters[0].id}.value | ${parameters[1].id}.value};
return result;''');
      case .intBitwiseXor:
        _push('''
$returnType result = {.value = ${parameters[0].id}.value ^ ${parameters[1].id}.value};
return result;''');
      case .intCompareTo:
        final a = parameters[0].id;
        final b = parameters[1].id;
        _push('''
$returnType result = {.variant = $a.value < $b.value    ? ${returnType}_less
                              : $a.value == $b.value ? ${returnType}_equal
                                                       : ${returnType}_greater};
return result;''');
      case .intDivideTruncating:
        _push('''
$returnType result = {.value = ${parameters[0].id}.value / ${parameters[1].id}.value};
return result;''');
      case .intMultiply:
        _push('''
$returnType result = {.value = ${parameters[0].id}.value * ${parameters[1].id}.value};
return result;''');
      case .intParse:
        final text = parameters[0].id;
        _push('''
$returnType result;
char *end_pointer;
errno = 0;
uint64_t value = strtol($text.value, &end_pointer, 10);
if (errno == ERANGE) {
    result = ($returnType) {
        .variant = ${returnType}_error,
        .value.error = {.value = "Value is out of range."},
    };
} else if (end_pointer == $text.value) {
    result = ($returnType) {
        .variant = ${returnType}_error,
        .value.error = {.value = "Text is empty."},
    };
} else if (*end_pointer != '\\0') {
    char* message_format = "Non-numeric character \\"%c\\" at index %ld.";
    int length = snprintf(NULL, 0, message_format, *end_pointer, end_pointer - $text.value);
    char *message = malloc(length + 1);
    snprintf(message, length + 1, message_format, *end_pointer, end_pointer - $text.value);
    result = ($returnType) {
        .variant = ${returnType}_error,
        .value.error = {.value = message},
    };
} else {
    result = ($returnType) {
        .variant = ${returnType}_ok,
        .value.ok = {.value = value},
    };
}
return result;''');
      case .intRemainder:
        _push('''
$returnType result = {.value = ${parameters[0].id}.value % ${parameters[1].id}.value};
return result;''');
      case .intShiftLeft:
        _push('''
$returnType result = {.value = ${parameters[0].id}.value << ${parameters[1].id}.value};
return result;''');
      case .intShiftRight:
        _push('''
$returnType result = {.value = ${parameters[0].id}.value >> ${parameters[1].id}.value};
return result;''');
      case .intSubtract:
        _push('''
$returnType result = {.value = ${parameters[0].id}.value - ${parameters[1].id}.value};
return result;''');
      case .intToText:
        final int_ = parameters[0].id;
        _push('''
int length = snprintf(NULL, 0, "%ld", $int_.value);
$returnType result = {.value = malloc(length + 1)};
snprintf(result.value, length + 1, "%ld", $int_.value);
return result;''');
      case .listFilled:
        final length = parameters[0].id;
        final item = parameters[1].id;
        _push('''
if ($length.value < 0) {
    char* message_format = "List length must not be negative; was %ld.";
    int length = snprintf(NULL, 0, message_format, $length.value);
    $textType message = {.value = malloc(length + 1)};
    snprintf(message.value, length + 1, message_format, $length.value);
    $_panicFunction(message);
}

$returnType result = {
    .length = $length.value,
    .values = malloc($length.value * sizeof(${itemType()})),
};
for (uint64_t i = 0; i < $length.value; i++) {
    result.values[i] = $item;
}
return result;''');
      case .listGenerate:
        final length = parameters[0].id;
        final itemGetter = parameters[1].id;
        _push('''
if ($length.value < 0) {
    char* message_format = "List length must not be negative; was %ld.";
    int length = snprintf(NULL, 0, message_format, $length.value);
    $textType message = {.value = malloc(length + 1)};
    snprintf(message.value, length + 1, message_format, $length.value);
    $_panicFunction(message);
}

$returnType result = {
    .length = $length.value,
    .values = malloc($length.value * sizeof(${itemType()}*)),
};
for (uint64_t i = 0; i < $length.value; i++) {
    $intType index = {.value = i};
    result.values[i] = $itemGetter.function($itemGetter.closure, index);
}
return result;''');
      case .listGet:
        final list = parameters[0].id;
        final index = parameters[1].id;
        _push('''
$returnType result;
if (0 <= $index.value && $index.value < $list.length) {
    result = ($returnType) {
        .variant = ${returnType}_some,
        .value.some = $list.values[$index.value],
    };
} else {
    result = ($returnType) {.variant = ${returnType}_none};
}
return result;''');
      case .listInsert:
        final list = parameters[0].id;
        final index = parameters[1].id;
        final item = parameters[2].id;
        _push('''
if (0 > $index.value || $index.value > $list.length) {
    char* message_format = "Index out of bounds: Tried inserting at index %ld in list of length %ld.";
    int length = snprintf(NULL, 0, message_format, $index.value, $list.length);
    $textType message = {.value = malloc(length + 1)};
    snprintf(message.value, length + 1, message_format, $index.value, $list.length);
    $_panicFunction(message);
}

uint64_t length = $list.length + 1;
$returnType result = {
    .length = length,
    .values = malloc(length * sizeof(${itemType()})),
};
memcpy(result.values, $list.values, $index.value * sizeof(${itemType()}*));
result.values[$index.value] = $item;
memcpy(result.values + $index.value + 1, $list.values + $index.value, ($list.length - $index.value) * sizeof(${itemType()}));
return result;''');
      case .listLength:
        _push('''
$returnType result = {.value = ${parameters[0].id}.length};
return result;''');
      case .listOf0:
        _push('''
$returnType result = {
    .length = 0,
    .values = NULL,
};
return result;''');
      case .listOf1:
        _push('''
$returnType result = {
    .length = 1,
    .values = malloc(sizeof(${itemType()})),
};
result.values[0] = ${parameters[0].id};
return result;''');
      case .listOf2:
        _push('''
$returnType result = {
    .length = 2,
    .values = malloc(2 * sizeof(${itemType()})),
};
result.values[0] = ${parameters[0].id};
result.values[1] = ${parameters[1].id};
return result;''');
      case .listOf3:
        _push('''
$returnType result = {
    .length = 3,
    .values = malloc(3 * sizeof(${itemType()})),
};
result.values[0] = ${parameters[0].id};
result.values[1] = ${parameters[1].id};
result.values[2] = ${parameters[2].id};
return result;''');
      case .listOf4:
        _push('''
$returnType result = {
    .length = 4,
    .values = malloc(4 * sizeof(${itemType()})),
};
result.values[0] = ${parameters[0].id};
result.values[1] = ${parameters[1].id};
result.values[2] = ${parameters[2].id};
result.values[3] = ${parameters[3].id};
return result;''');
      case .listOf5:
        _push('''
$returnType result = {
    .length = 5,
    .values = malloc(5 * sizeof(${itemType()})),
};
result.values[0] = ${parameters[0].id};
result.values[1] = ${parameters[1].id};
result.values[2] = ${parameters[2].id};
result.values[3] = ${parameters[3].id};
result.values[4] = ${parameters[4].id};
return result;''');
      case .listRemoveAt:
        final list = parameters[0].id;
        final index = parameters[1].id;
        _push('''
if (0 > $index.value || $index.value >= $list.length) {
    char* message_format = "Index out of bounds: Tried removing item at index %ld from list of length %ld.";
    int length = snprintf(NULL, 0, message_format, $index.value, $list.length);
    $textType message = {.value = malloc(length + 1)};
    snprintf(message.value, length + 1, message_format, $index.value, $list.length);
    $_panicFunction(message);
}

uint64_t length = $list.length - 1;
$returnType result = {
    .length = length,
    .values = malloc(length * sizeof(${itemType()})),
};
memcpy(result.values, $list.values, $index.value * sizeof(${itemType()}*));
memcpy(result.values + $index.value, $list.values + $index.value + 1, ($list.length - $index.value - 1) * sizeof(${itemType()}*));
return result;''');
      case .listReplace:
        final list = parameters[0].id;
        final index = parameters[1].id;
        final newItem = parameters[2].id;
        _push('''
if (0 > $index.value || $index.value >= $list.length) {
    char* message_format = "Index out of bounds: Tried replacing index %ld in list of length %ld.";
    int length = snprintf(NULL, 0, message_format, $index.value, $list.length);
    $textType message = {.value = malloc(length + 1)};
    snprintf(message.value, length + 1, message_format, $index.value, $list.length);
    $_panicFunction(message);
}

$returnType result = {
    .length = $list.length,
    .values = malloc($list.length * sizeof(${itemType()})),
};
memcpy(result.values, $list.values, $list.length * sizeof(${itemType()}*));
result.values[$index.value] = $newItem;
return result;''');
      case .panic:
        _push('''
fputs(${parameters[0].id}.value, stderr);
exit(1);''');
      case .print:
        _push('''
puts(${parameters[0].id}.value);
$returnType _1 = {};
return _1;''');
      case .textCompareTo:
        final a = parameters[0].id;
        final b = parameters[1].id;
        _push('''
int raw_result = strcmp($a.value, $b.value);
$returnType result = {.variant = raw_result < 0    ? ${returnType}_less
                            : raw_result == 0 ? ${returnType}_equal
                                              : ${returnType}_greater};
return result;''');
      case .textConcat:
        final a = parameters[0].id;
        final b = parameters[1].id;
        _push('''
size_t lengthA = strlen($a.value);
size_t lengthB = strlen($b.value);
$returnType result = {.value = malloc(lengthA + lengthB + 1)};
memcpy(result.value, $a.value, lengthA);
memcpy(result.value + lengthA, $b.value, lengthB + 1);
return result;''');
      case .textGetRange:
        final text = parameters[0].id;
        final startInclusive = parameters[1].id;
        final endExclusive = parameters[2].id;
        _push('''
size_t text_length = strlen($text.value);
if (0 > $startInclusive.value || $startInclusive.value > text_length
    || 0 > $endExclusive.value || $endExclusive.value > text_length) {
    char* message_format = "Index out of bounds: Tried getting range %ld..%ld from text that is only %ld long.";
    int length = snprintf(NULL, 0, message_format, $startInclusive.value, $endExclusive.value, text_length);
    $textType message = {.value = malloc(length + 1)};
    snprintf(message.value, length + 1, message_format, $startInclusive.value, $endExclusive.value, text_length);
    $_panicFunction(message);
} else if ($startInclusive.value > $endExclusive.value) {
    char* message_format = "Invalid range %ld..%ld: `start_inclusive` must be less than or equal to `end_exclusive`.";
    int length = snprintf(NULL, 0, message_format, $startInclusive.value, $endExclusive.value);
    $textType message = {.value = malloc(length + 1)};
    snprintf(message.value, length + 1, message_format, $startInclusive.value, $endExclusive.value);
    $_panicFunction(message);
}

size_t length = $endExclusive.value - $startInclusive.value;
$returnType result = {.value = malloc(length + 1)};
memcpy(result.value, $text.value + $startInclusive.value, length);
return result;''');
      case .textIndexOf:
        final a = parameters[0].id;
        final b = parameters[1].id;
        _push('''
$returnType result;
char* result_raw = strstr($a.value, $b.value);
if (result_raw == NULL) {
    result = ($returnType) {.variant = ${returnType}_none};
} else {
    result = ($returnType) {
        .variant = ${returnType}_some,
        .value.some = {.value = result_raw - $a.value},
    };
}
return result;''');
      case .textLength:
        _push('''
$returnType result = {.value = strlen(${parameters[0].id}.value)};
return result;''');
    }
  }
}

String _escapeCString(String text) {
  final buffer = StringBuffer();
  for (final rune in text.runes) {
    switch (rune) {
      case 0x22:
        buffer.write(r'\"');
      case 0x5C:
        buffer.write(r'\\');
      case 0x0A:
        buffer.write(r'\n');
      case 0x0D:
        buffer.write(r'\r');
      case 0x09:
        buffer.write(r'\t');
      default:
        buffer.writeCharCode(rune);
    }
  }
  return buffer.toString();
}

extension<T> on List<T> {
  Iterable<(T, U)> zip<U>(List<U> other) sync* {
    for (var i = 0; i < length && i < other.length; i++) {
      yield (this[i], other[i]);
    }
  }
}
