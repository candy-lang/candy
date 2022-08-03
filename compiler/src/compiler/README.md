# The Compiler Pipeline

The compiler uses query-based compilation to do the least amount of work necessary.

These are the compiler stages:

* String: The literal source code
* RCST ("Raw Concrete Syntax Tree"): A tree that represents the syntax of the code, including every single character and whitespace.
* CST ("Concrete Syntax Tree"): Similar to RCST, but tree nodes also have IDs and know what ranges in the source file they correspond to.
* AST ("Abstract Syntax Tree"): A tree where unnecessary cruft is removed and some invariants are validated.
* HIR ("High-level Intermediate Representation"): The canonical representation of source code in single-static-assignment form (SSA).
* LIR ("Low-level Intermediate Representation"): An instruction code for a stack-based virtual machine.

Note that if an error occurs in a compilation stage, we don't immediately abort but rather just try to contain the error in a subtree of the code and emit an error node.
This means that even if you have a syntax error (missing parentheses, etc.), the tooling in other parts of the source still works – including autocompletion, edit-time evaluation, formatting, etc.
You can even *run* the code – it will simply panic during runtime if it encounters the part with the syntax error.
