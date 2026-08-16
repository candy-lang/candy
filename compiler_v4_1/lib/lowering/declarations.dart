import '../ast.dart';
import '../context.dart';
import '../error.dart';
import '../hir/hir/builtins.dart';
import '../hir/hir/ids.dart';
import '../lexer/token.dart';
import '../position.dart';
import 'modules.dart';

sealed class CollectedDeclaration {
  const CollectedDeclaration();

  DeclarationId get id;
  ModuleId get module => id.module;
  AstVisibility get visibility;
}

class CollectedStruct extends CollectedDeclaration {
  const CollectedStruct({required this.id, required this.ast});
  @override
  final DeclarationId id;
  final AstStruct ast;
  @override
  AstVisibility get visibility => ast.visibility;
}

class CollectedEnum extends CollectedDeclaration {
  const CollectedEnum({required this.id, required this.ast});
  @override
  final DeclarationId id;
  final AstEnum ast;
  @override
  AstVisibility get visibility => ast.visibility;
}

class CollectedTrait extends CollectedDeclaration {
  const CollectedTrait({
    required this.id,
    required this.ast,
    required this.associatedTypes,
    required this.functions,
  });
  @override
  final DeclarationId id;
  final AstTrait ast;
  final List<CollectedAssociatedType> associatedTypes;
  final List<CollectedFunction> functions;
  @override
  AstVisibility get visibility => ast.visibility;
}

class CollectedImpl extends CollectedDeclaration {
  const CollectedImpl({
    required this.id,
    required this.ast,
    required this.associatedTypes,
    required this.functions,
  });
  @override
  final DeclarationId id;
  final AstImpl ast;
  final List<CollectedAssociatedType> associatedTypes;
  final List<CollectedFunction> functions;

  /// Impls are never named, so they are never imported.
  @override
  AstVisibility get visibility => .private;
}

class CollectedAssociatedType extends CollectedDeclaration {
  const CollectedAssociatedType({required this.id, required this.ast});
  @override
  final DeclarationId id;
  final AstAssociatedType ast;

  /// Members of a trait or impl are never named from the outside.
  @override
  AstVisibility get visibility => .private;
}

class CollectedFunction extends CollectedDeclaration {
  const CollectedFunction({required this.id, required this.ast});
  @override
  final DeclarationId id;
  final AstFunction ast;
  @override
  AstVisibility get visibility => ast.visibility;
}

class CollectedAssignment extends CollectedDeclaration {
  const CollectedAssignment({required this.id, required this.ast});
  @override
  final DeclarationId id;
  final AstAssignment ast;
  @override
  AstVisibility get visibility => ast.visibility;
}

class CollectedDeclarations {
  CollectedDeclarations({
    required this.modules,
    required this.scopes,
    required this.declarations,
    required this.wellKnownTypes,
  });

  final List<Module> modules;
  final Map<ModuleId, ImportScope> scopes;
  final Map<DeclarationId, CollectedDeclaration> declarations;
  final WellKnownTypes wellKnownTypes;

  Iterable<T> ofKind<T extends CollectedDeclaration>() =>
      declarations.values.whereType<T>();

  ImportScope scopeOf(ModuleId module) => scopes[module]!;
}

CollectedDeclarations collectDeclarations(
  Context context,
  List<Module> modules,
) => _Collector(context)._collect(modules);

class _Collector {
  _Collector(this.context);

  final Context context;
  final _declarations = <DeclarationId, CollectedDeclaration>{};
  final _scopes = <ModuleId, ImportScope>{};

  void _error(SourceSpan span, String message) =>
      context.addError(CompilerError(span: span, message: message));

  CollectedDeclarations _collect(List<Module> modules) {
    for (final module in modules) {
      _scopes[module.id] = ImportScope(
        moduleDeclarations: _collectModule(module),
        imports: {},
        prelude: {},
      );
    }

    final byId = {for (final module in modules) module.id: module};
    final knownPackages = {
      ...byId.keys.map((it) => it.packageName),
      ModuleId.builtinPackageName,
    };
    for (final module in modules) {
      _resolveImports(module, byId, knownPackages);
    }
    _fillPrelude(modules);

    return CollectedDeclarations(
      modules: modules,
      scopes: _scopes,
      declarations: _declarations,
      wellKnownTypes: _findWellKnownTypes(),
    );
  }

  void _resolveImports(
    Module module,
    Map<ModuleId, Module> byId,
    Set<String> knownPackages,
  ) {
    final imports = _scopes[module.id]!.imports;
    final seen = <ModuleId>{};

    for (final use in module.ast.uses) {
      if (use.path.isEmpty) continue;

      final span = use.useKeyword.span;
      final root = use.path.first;
      final packageName = root.kind is TokenKind_PackageKeyword
          ? module.id.packageName
          : root.rawText;
      final target = ModuleId(
        packageName: packageName,
        path: [for (final segment in use.path.skip(1)) segment.rawText],
      );

      if (target == module.id) {
        _error(span, "A module can't import itself.");
        continue;
      }
      if (target == ModuleId.builtin) {
        if (!seen.add(target)) {
          _error(span, '`${_pathText(use)}` is already imported.');
          continue;
        }
        for (final MapEntry(key: name, value: declarations)
            in buildBuiltins().entries) {
          _merge(imports, name, declarations);
        }
        continue;
      }
      if (!knownPackages.contains(packageName)) {
        _error(span, 'Unknown package: `$packageName`.');
        continue;
      }
      if (!byId.containsKey(target)) {
        _error(span, 'Unknown module: `${_pathText(use)}`.');
        continue;
      }
      if (!seen.add(target)) {
        _error(span, '`${_pathText(use)}` is already imported.');
        continue;
      }

      _importFrom(imports, target, module.id.packageName);
    }
  }

  /// Core's public declarations are ambient everywhere except inside Core,
  /// which would otherwise merge its own overload sets into themselves.
  void _fillPrelude(List<Module> modules) {
    final core = <String, NamedDeclarations>{};
    for (final module in modules) {
      if (module.id.packageName != ModuleId.corePackageName) continue;

      for (final MapEntry(key: name, value: declarations)
          in _scopes[module.id]!.moduleDeclarations.entries) {
        final visible = _visibleOnly(declarations, false);
        if (visible != null) _merge(core, name, visible);
      }
    }
    if (core.isEmpty) return;

    for (final module in modules) {
      if (module.id.packageName == ModuleId.corePackageName) continue;
      _scopes[module.id]!.prelude.addAll(core);
    }
  }

  String _pathText(AstUse use) => use.path.map((it) => it.rawText).join('.');

  void _importFrom(
    Map<String, NamedDeclarations> imports,
    ModuleId target,
    String importingPackage,
  ) {
    final samePackage = target.packageName == importingPackage;
    for (final MapEntry(key: name, value: declarations)
        in _scopes[target]!.moduleDeclarations.entries) {
      final visible = _visibleOnly(declarations, samePackage);
      if (visible != null) _merge(imports, name, visible);
    }
  }

  NamedDeclarations? _visibleOnly(
    NamedDeclarations declarations,
    bool samePackage,
  ) {
    bool isVisible(DeclarationId id) => switch (_declarations[id]!.visibility) {
      .private => false,
      .package => samePackage,
      .public => true,
    };

    switch (declarations) {
      case NamedFunctions(:final ids):
        final visible = ids.where(isVisible).toList();
        return visible.isEmpty ? null : NamedFunctions(visible);
      case NamedType(:final id):
      case NamedTrait(:final id):
      case NamedAssignment(:final id):
        return isVisible(id) ? declarations : null;
      case NamedAmbiguous():
        // A module's own declarations are never ambiguous.
        return null;
    }
  }

  List<DeclarationId> _idsOf(NamedDeclarations declarations) =>
      switch (declarations) {
        NamedFunctions(:final ids) || NamedAmbiguous(:final ids) => ids,
        NamedType(:final id) ||
        NamedTrait(:final id) ||
        NamedAssignment(:final id) => [id],
      };

  void _merge(
    Map<String, NamedDeclarations> imports,
    String name,
    NamedDeclarations incoming,
  ) {
    final existing = imports[name];
    if (existing == null) {
      imports[name] = incoming;
      return;
    }
    if (existing is NamedFunctions && incoming is NamedFunctions) {
      imports[name] = NamedFunctions([...existing.ids, ...incoming.ids]);
      return;
    }
    imports[name] = NamedAmbiguous([..._idsOf(existing), ..._idsOf(incoming)]);
  }

  Map<String, NamedDeclarations> _collectModule(Module module) {
    final names = <String, NamedDeclarations>{};
    final functionCounts = <String, int>{};
    var implCount = 0;

    bool claim(
      String name,
      SourceSpan span,
      NamedDeclarations Function() declarations,
    ) {
      final existing = names[name];
      if (existing != null) {
        _error(span, switch (existing) {
          NamedType() || NamedTrait() => 'Duplicate type name: `$name`.',
          NamedAssignment() =>
            'A top-level declaration named `$name` already exists.',
          NamedFunctions() =>
            "A top-level declaration can't have the same name as a "
                'top-level function: `$name`.',
          NamedAmbiguous() =>
            'A top-level declaration named `$name` already exists.',
        });
        return false;
      }
      names[name] = declarations();
      return true;
    }

    DeclarationId? declare(
      AstErrorOr<Token> name,
      NamedDeclarations Function(DeclarationId) declarations,
    ) {
      final token = name.toNullable();
      if (token == null) return null;

      final id = DeclarationId(module: module.id, name: token.rawText);
      if (!claim(token.rawText, token.span, () => declarations(id))) {
        return null;
      }
      return id;
    }

    for (final declaration in module.ast.declarations) {
      switch (declaration) {
        case AstDeclaration_Struct(:final struct):
          final id = declare(struct.name, NamedType.new);
          if (id != null) {
            _declarations[id] = CollectedStruct(id: id, ast: struct);
          }

        case AstDeclaration_Enum(:final enum_):
          final id = declare(enum_.name, NamedType.new);
          if (id != null) {
            _declarations[id] = CollectedEnum(id: id, ast: enum_);
          }

        case AstDeclaration_Trait(:final trait):
          final id = declare(trait.name, NamedTrait.new);
          if (id != null) {
            final members = _collectMembers(
              id,
              trait.associatedTypes,
              trait.functions,
            );
            _declarations[id] = CollectedTrait(
              id: id,
              ast: trait,
              associatedTypes: members.associatedTypes,
              functions: members.functions,
            );
          }

        case AstDeclaration_Impl(:final impl):
          final id = DeclarationId(
            module: module.id,
            name: DeclarationId.implName,
            disambiguator: implCount++,
          );
          final members = _collectMembers(
            id,
            impl.associatedTypes,
            impl.functions,
          );
          _declarations[id] = CollectedImpl(
            id: id,
            ast: impl,
            associatedTypes: members.associatedTypes,
            functions: members.functions,
          );

        case AstDeclaration_Assignment(:final assignment):
          final id = declare(assignment.name, NamedAssignment.new);
          if (id != null) {
            _declarations[id] = CollectedAssignment(id: id, ast: assignment);
          }

        case AstDeclaration_Function(:final function):
          final token = function.name.toNullable();
          if (token == null) continue;

          final name = token.rawText;
          final existing = names[name];
          if (existing != null && existing is! NamedFunctions) {
            _error(
              token.span,
              "A top-level function can't have the same name as a top-level "
              'declaration: `$name`.',
            );
            continue;
          }

          final id = DeclarationId(
            module: module.id,
            name: name,
            disambiguator: functionCounts.update(
              name,
              (it) => it + 1,
              ifAbsent: () => 0,
            ),
          );
          names[name] = NamedFunctions([
            if (existing is NamedFunctions) ...existing.ids,
            id,
          ]);
          _declarations[id] = CollectedFunction(id: id, ast: function);
      }
    }

    return names;
  }

  ({
    List<CollectedAssociatedType> associatedTypes,
    List<CollectedFunction> functions,
  })
  _collectMembers(
    DeclarationId parent,
    List<AstAssociatedType> associatedTypes,
    List<AstFunction> functions,
  ) {
    final collectedAssociatedTypes = <CollectedAssociatedType>[];
    final takenNames = <String>{};
    for (final associatedType in associatedTypes) {
      final token = associatedType.name.toNullable();
      if (token == null) continue;

      final name = token.rawText;
      if (!takenNames.add(name)) {
        _error(token.span, 'Duplicate associated type name: `$name`.');
        continue;
      }
      final collected = CollectedAssociatedType(
        id: parent.child(name),
        ast: associatedType,
      );
      _declarations[collected.id] = collected;
      collectedAssociatedTypes.add(collected);
    }

    final counts = <String, int>{};
    final collectedFunctions = <CollectedFunction>[];
    for (final function in functions) {
      final token = function.name.toNullable();
      if (token == null) continue;

      final name = token.rawText;
      if (takenNames.contains(name)) {
        _error(
          token.span,
          'An associated type named `$name` already exists.',
        );
        continue;
      }
      final id = parent.child(
        name,
        disambiguator: counts.update(name, (it) => it + 1, ifAbsent: () => 0),
      );
      final collectedFunction = CollectedFunction(id: id, ast: function);
      _declarations[id] = collectedFunction;
      collectedFunctions.add(collectedFunction);
    }

    return (
      associatedTypes: collectedAssociatedTypes,
      functions: collectedFunctions,
    );
  }

  /// Well-known types can live in any module until `use` exists, so this scans
  /// every module and takes the first match.
  WellKnownTypes _findWellKnownTypes() {
    final found = <String, DeclarationId>{};
    for (final declaration in _declarations.values) {
      if (declaration is! CollectedStruct && declaration is! CollectedEnum) {
        continue;
      }
      if (declaration.module.packageName != ModuleId.corePackageName) continue;

      final name = declaration.id.name;
      if (WellKnownTypes.names.contains(name)) {
        found.putIfAbsent(name, () => declaration.id);
      }
    }

    return WellKnownTypes(
      intId: found['Int'],
      textId: found['Text'],
      listId: found['List'],
      nothingId: found['Nothing'],
      neverId: found['Never'],
      maybeId: found['Maybe'],
      orderingId: found['Ordering'],
      resultId: found['Result'],
      boolId: found['Bool'],
    );
  }
}
