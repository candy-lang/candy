# Our thinking behind some of our technical architecture decisions

It already happened a bunch of times that we considered multiple options and then quickly decided on one and went ahead implementing it.
Turns out, that's not the most sustainable approach – we had to re-visit multiple decisions.

The documents in this folder will explain or thinking behind some of the technical architecture decisions of the compiler.

## Codegen: Trait objects save direct impls instead of transitive functions

When generating a trait object in Dart, callers need to be able to call functions of super traits.

```candy
trait Foo: Bar {
  ...
}
```

Here, if you have a `Foo`, you should also be able to call `Bar`'s functions on it.
Note that inside the `Foo` trait, the default implementation of `Bar` functions may also be overriden.

In the generated Dart code for the `Foo` trait object, we need to be able to access the trait methods.
This are the two options we considered:

* Save all transitively inherited trait functions directly in the trait object:
  
  ```dart
  class Value$Named$Trait$Foo implements Value$Named$Trait {
    const Value$Named$Trait$Foo._(
      this.inlineType,
      this.value,
      ..., // own functions
      ..., // inherited functions
    );

    @override
    final InlineType inlineType;
    final Value value;

    // Own functions
    ...

    // Inherited functions
    ...
  }
  ```
  
  On the call side, you can use these functions directly:

  ```dart
  myFoo.someBarFunction()
  ```
* Only save impls to the super traits:
  
  ```dart
  class Value$Named$Trait$Foo implements Value$Named$Trait {
    const Value$Named$Trait$Foo._(
      this.inlineType,
      this.value,
      this.impl$MangledBar,
    );

    @override
    final InlineType inlineType;
    final Value value;

    // Own functions

    // Inherited functions
    final Value Function() impl$MangledBar;
  }
  ```
  
  ```dart
  (myFoo.impl$MangledBar() as Value$Named$Trait$Foo).someBarFunction()
  ```

The second option pushes complexity from the trait generation to the call-side.

We chose to go with the second option.
It makes all trait definitions shorter.
At call-side, we already know the exact function that gets called, so we can follow the impl chain, transforming the `myFoo` value while we do so.
