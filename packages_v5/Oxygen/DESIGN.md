# Candy GUI Framework — Programming Model Design

## Context

Candy is a pure, immutable, functional language, still early-stage: `Int`/`Text`/`List`/structs/enums/traits exist, but there is no windowing, graphics, or FFI. The goal of this work is **not** a working renderer — it is to design *how writing UI code in Candy should look and feel*, taking Flutter's declarative widget trees, Compose's conciseness, and Elm's pure update model, and adapting them to a language with no mutation.

The central design problem: Flutter's `RenderObject` model persists across frames and mutates its own stored fields during `layout()`/`paint()`. None of that can exist here. Everything — widget tree, per-widget local state, cached layout results, event routing — must be immutable values threaded through pure functions, while still feeling as concise as Flutter or Compose at the call site.

The deliverable is a design document with worked Candy sketches. No code ships to `packages_v5/` yet, because the model depends on two language features Candy does not have (below).

## Prerequisite language features

These are dependencies, not part of this design's scope. Each needs its own spec and compiler work in `compiler_v4_1/`.

1. **`dyn Trait` / trait objects** with implicit coercion, so `impl Widget` values nest directly (`Column(List.of(Label("hi"), Counter()))`) with no wrapping calls, plus the runtime type identity and checked downcast (`Any`) that come bundled with the same machinery. This was chosen over emulating erasure with a narrow box/unbox builtin, because that approach forces an explicit `anyWidget(...)` wrap at every nesting site — fatal to the conciseness goal. **Implemented**, written as the bare trait name (`children: List[Widget]`, `state: Any`); see "Trait objects and `Any`" in [`compiler_v4_1/LANGUAGE.md`](compiler_v4_1/LANGUAGE.md).
2. **Associated types on traits** — currently an explicit TODO in the stdlib ([`Core/_.candy:10`](packages_v5/Core/_.candy#L10), on `OperatorPlus`). Every author-facing widget trait needs `type State`.

## Core model

### The state tree is the widget tree

There is no side-table of elements. A widget's persistent state lives **in its parent's state**, as opaque `ChildElement` values. The host runtime holds exactly one root `ChildElement`; everything else is nested inside it.

```candy
struct ChildElement {
  widget: Widget,   # the dyn value that produced this subtree
  state: Any,       # that widget's own State, erased
  size: Size,
}

fun layoutChild(
  previous: Maybe[ChildElement],
  widget: Widget,
  constraints: Constraints,
  context: Context,
) ChildElement
fun paintChild(self: ChildElement, offset: Offset) List[DrawCommand]
fun handleChild(self: ChildElement, path: List[Int], message: Any) ChildElement
```

**Reconciliation happens inside `layoutChild`**, not as a separate tree-diffing pass: if `previous` exists and its `widget` has the same concrete type as the new `widget`, its `state` is reused; otherwise `initialState()` runs. Matching is by position within the parent plus concrete type — Flutter's default behavior. Explicit `key:` for reordering is deliberately out of scope (see Non-goals).

### `Widget` — the erased, uniform interface

For `Widget` to be a trait object its methods cannot mention `Self::State`, so state is erased at this boundary. **Authors never implement `Widget` directly**; only the framework's blanket impls do.

```candy
trait Widget {
  fun initialState(self: Self) Any
  fun layout(self: Self, state: Any, constraints: Constraints, context: Context) LayoutResult[Any]
  fun paint(self: Self, state: Any, size: Size) List[DrawCommand]
  fun handle(self: Self, state: Any, path: List[Int], message: Any) Any
}
struct LayoutResult[S] { size: Size, state: S }
```

`paint` deliberately receives no `Context` — anything visual is read during layout and baked into props or state by then.

### Author-facing traits

Four typed traits, split by child arity so leaf widgets never see child-threading machinery (mirroring Flutter's `Leaf`/`SingleChild`/`MultiChildRenderObjectWidget`). Each has a blanket `impl[T: X] T: Widget` doing the erasure and the `ChildElement` bookkeeping.

| Trait               | Children         | Author writes                                 |
| ------------------- | ---------------- | --------------------------------------------- |
| `LeafWidget`        | none             | `initialState`, `layout`, `paint`             |
| `SingleChildWidget` | one              | `initialState`, `child`, `layout`, `paint`    |
| `MultiChildWidget`  | list             | `initialState`, `children`, `layout`, `paint` |
| `CompositeWidget`   | builds a subtree | `initialState`, `update`, `build`             |

Because each blanket impl defines the state *wrapper*, the framework always knows where the `ChildElement`s are; the author's own state sits in an `own` field.

```candy
struct CompositeState[S] { own: S, child: Maybe[ChildElement] }

impl[T: CompositeWidget] T: Widget {
  fun initialState(self: T) Any {
    CompositeState(initialState(self), none())
  }

  fun layout(self: T, state: Any, constraints: Constraints, context: Context) LayoutResult[Any] {
    let state: CompositeState[T::State] = downcast(state)
    let child = layoutChild(state.child, build(self, state.own, context, dispatch), constraints, context)
    LayoutResult(child.size, CompositeState(state.own, some(child)))
  }

  fun paint(self: T, state: Any, size: Size) List[DrawCommand] {
    let state: CompositeState[T::State] = downcast(state)
    paintChild(unwrap(state.child), Offset(0, 0))
  }

  fun handle(self: T, state: Any, path: List[Int], message: Any) Any {
    let state: CompositeState[T::State] = downcast(state)
    switch isEmpty(path) {
      true => CompositeState(update(self, state.own, downcast[T::Msg](message)), state.child),
      false => CompositeState(state.own, some(handleChild(unwrap(state.child), path, message))),
    }
  }
}
```

## Local state: Elm-style, per widget

Each `CompositeWidget` has its own `State`, `Msg`, `update`, and `build`. A child's `Msg` type is private to that child — parents never see or forward it, unlike a single global `Msg` enum. This isolation is the main reason for per-widget rather than app-wide Elm.

```candy
struct Counter {}
struct CounterState { count: Int }
enum CounterMsg { increment, decrement }

impl Counter: CompositeWidget {
  type State = CounterState
  type Msg = CounterMsg

  fun initialState(self: Counter) CounterState { CounterState(0) }

  fun update(self: Counter, state: CounterState, msg: CounterMsg) CounterState {
    switch msg {
      increment => CounterState(state.count.add(1)),
      decrement => CounterState(state.count.subtract(1)),
    }
  }

  fun build(self: Counter, state: CounterState, context: Context, dispatch: (CounterMsg) Nothing) Widget {
    Row(List.of[Widget](
      Button("-", () { dispatch(CounterMsg.decrement()) }),
      Label("{state.count}", themeOf(context).body),
      Button("+", () { dispatch(CounterMsg.increment()) }),
    ))
  }
}
```

**Optional sugar** for one-off widgets that don't deserve a named struct — a generic framework widget implementing `CompositeWidget` from closures:

```candy
struct Stateful[S, M] {
  initial: S,
  update: (S, M) S,
  build: (S, Context, (M) Nothing) Widget,
}
```

Caveat worth documenting: since reconciliation matches on concrete type, two different `Stateful[Int, SomeMsg]` widgets at the same position would match each other. Named widget structs are the default; `Stateful` is for genuinely local one-offs.

## Layout and paint

Layout is a pure top-down recursion returning immutable results; there is nothing to write back. A widget's `State` doubles as its layout cache — which is exactly why `state` is threaded through both `layout` and `paint`.

```candy
struct Label { value: Text, style: TextStyle }
struct LabelState { shaped: Maybe[GlyphRun] }

impl Label: LeafWidget {
  type State = LabelState

  fun initialState(self: Label) LabelState { LabelState(none[GlyphRun]()) }

  fun layout(self: Label, state: LabelState, constraints: Constraints, context: Context) LayoutResult[LabelState] {
    # `state.shaped` is reusable when value/style/width are unchanged
    let shaped = shapeText(self.value, self.style, constraints.maxSize.width)
    LayoutResult(shaped.size, LabelState(some(shaped)))
  }

  fun paint(self: Label, state: LabelState, size: Size) List[DrawCommand] {
    List.of(DrawCommand.glyphs(unwrap(state.shaped), Offset(0, 0)))
  }
}
```

A container calls `layoutChild` per child, then stores the resulting `ChildElement`s and its computed offsets in its own state for `paint` to read:

```candy
struct Column { children: List[Widget] }
struct ColumnState { children: List[ChildElement], offsets: List[Offset] }

impl Column: MultiChildWidget {
  type State = ColumnState

  fun children(self: Self) List[Widget] { self.children }

  fun layout(self: Self, state: ColumnState, constraints: Constraints, context: Context) LayoutResult[ColumnState] {
    let laidOut = self.children.mapIndexed((index, child) {
      layoutChild(get(state.children, index), child, constraints.loosenHeight(), context)
    })
    let offsets = runningOffsets(laidOut)
    let size = Size(maxChildWidth(laidOut), totalChildHeight(laidOut))
    LayoutResult(size, ColumnState(laidOut, offsets))
  }

  fun paint(self: Self, state: ColumnState, size: Size) List[DrawCommand] {
    flatMap(indexes(state.children), (i: Int) {
      paintChild(unwrap(get(state.children, i)), unwrap(get(state.offsets, i)))
    })
  }
}
```

Note: the widget is `Label`, not `Text` — `Text` is Candy's builtin string type.

## Events and dispatch routing

`dispatch` closes over the node's path from the root, so invoking it yields a routed pair rather than performing an effect:

```candy
struct Routed { path: List[Int], message: Any }
```

Interactive draw commands (e.g. `Button`'s hit region) carry a `() Routed` callback. The host runtime hit-tests, then walks `handle` down `path` to the owning node, which downcasts the payload to its own `Msg` and applies `update`. Every blanket impl knows how to recurse to the child at a given index, so routing is uniform.

The host loop, in full:

1. `layoutChild(some(root), rootWidget, screenConstraints, emptyContext())` → new root `ChildElement`
2. `paintChild(root, Offset(0, 0))` → `List[DrawCommand]` → backend
3. On input: hit-test → `Routed` → `handleChild(root, routed.path, routed.message)` → back to step 1

## Inherited values (`InheritedWidget` equivalent)

An inherited value is a reader environment threaded alongside constraints. `layoutChild` forwards it unchanged by default.

```candy
struct Context { values: Map[TypeId, Any] }
fun get[T](self: Context) Maybe[T]
fun with[T](self: Context, value: T) Context

struct Provider[T] { value: T, child: Widget }
impl[T] Provider[T]: SingleChildWidget { ... }   # layouts child with `context.with(self.value)`
```

Providing and reading, Compose-`CompositionLocal`-style rather than Flutter's subclassing — no fifth author trait needed, and named accessors are plain functions:

```candy
fun themeOf(context: Context) Theme { unwrap(get[Theme](context)) }

Provider(Theme(primary: blue), Column(List.of[Widget](Label("hi", ...), Counter())))
```

**Dependency tracking is unnecessary for correctness here.** Flutter needs `updateShouldNotify` and a dependents set because its rebuilds are surgical; this model recomputes top-down every frame, so a changed provided value is simply seen by everyone below. It becomes relevant only once subtree caching is added — at which point `ChildElement` records which `TypeId`s its subtree read, and a changed key invalidates that entry.

## Deliverables

- **`gui.md`** at the repo root, alongside the existing `language.md` and `Candy v3, v4.md` — the design document, containing the sections above with their worked sketches.
- **No `packages_v5/Gui/` package yet.** It cannot compile until the two prerequisites land, and a broken package in `packages_v5/` (whose convention is `<Package>/_package.candy` + `<Package>/_.candy`, per the just-restructured `Core` and `Example`) would be dead weight. Creating it is the first task *after* the prerequisites.
- Two short follow-up notes in `gui.md` naming the prerequisite features, so they can be split into their own specs against `compiler_v4_1/`.

## Non-goals

- Concrete layout algorithm — `Constraints`/`LayoutResult` fix the shape; flex/box sizing rules are a follow-up spec.
- Rendering backend, windowing, FFI — needs Candy FFI, which does not exist.
- Subtree caching / incremental relayout — the model is defined as full top-down recompute; caching is a later optimization that does not change these signatures.
- Animations, async, timers, accessibility.
- Explicit `key:` for list reordering — known gap, deferred.
- The `dyn Trait` and associated-types features themselves.

## Verification

Nothing compiles yet, so verification is by hand-tracing rather than execution:

1. Trace `Counter` nested inside a `Column` inside a `Provider` through two full frames, confirming that (a) the counter's `count` survives frame two, (b) a sibling inserted *before* it correctly resets its state under position+type matching, and (c) `themeOf(context)` resolves.
2. Trace a dispatch from the `+` button: path construction during `build`, routing through `handle`, and the resulting state tree.
3. Confirm every `Any` downcast in the blanket impls is reachable only from a matching erasure site.

Optional but valuable before committing: **port the model to a ~300-line Dart prototype** under `compiler_v4_1/tool/`, since Dart has the trait-object-ish dynamism this design assumes. That makes reconciliation, state threading, and dispatch routing executable and testable *today*, validating the design before either language feature is built.
