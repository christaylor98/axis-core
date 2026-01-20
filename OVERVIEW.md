
# Axis — Overview

Axis is exploring what happens if we place semantics at the centre of a programming system, rather than treating them as something inferred indirectly from syntax or execution.

Most programming systems today start with text, optimise for execution, and reconstruct meaning along the way. Axis inverts that relationship. It treats meaning as something that should be explicit, stable, and inspectable, and treats execution as a projection of that meaning.

We think that treating semantics as the core of the system may unlock forms of reuse, inspection, and evolution that are hard to achieve when meaning is implicit or scattered across layers.

This repository takes a step towards exploring that idea.

The idea behind Axis is that if program meaning is explicit and stable, then everything else can evolve independently.

Syntax can change, execution targets can change, tooling can change, while the meaning stays static.

Axis is not trying to prove that this approach will succeed in its current form. It is trying to discover where and if it creates real value, where it does not, and what kinds of systems become possible once meaning is treated as a first-class artifact.


## What Axis v0.1 is (and is not)

This release is intentionally exploratory.

It exists to test ideas, not to defend conclusions. There is no claim that the current model is complete, optimal, or production-ready. In fact, much of it is deliberately minimal or unfinished.

The goal is discovery.

If this approach turns out to be unhelpful in certain areas, that is still a useful outcome. If it opens up unexpected directions, even better.

## Core components

Axis is built around a small set of components that exist to uphold the separation between meaning, representation, and execution.

### Core IR and the compiler

At the centre of the system is the Core IR (Core Intermediate Representation).

The job of the compiler in v0.1 is not to produce executable code. Its job is to take a surface language, validate it, and lower it into a canonical semantic form.

That semantic form is the Core IR.

The Core IR is graph-shaped rather than textual. It represents values, control flow, binding structure, and effects explicitly. Once a program has been lowered into Core IR, its meaning is considered fixed. Everything downstream operates on that representation.

In this sense, the compiler is best thought of as a semantic normaliser rather than a traditional code generator.

The surface language used in v0.1 is intentionally minimal. It exists only to exercise the pipeline and the Core IR. It is not meant to be ergonomic, stable, or expressive. Future releases will focus more heavily on surface exploration.

### The Registry

The registry defines the semantic vocabulary of the system.

It describes what functions exist, how they can be called, and what kinds of effects or behaviours they represent. This information is shared across the compiler, the Core IR, and the bridge.  In this release we are not deeply exploring the relationship between effects, the registry, and bridges. This will become more important as bridge models grow more complex, and currently exists as a placeholder in the system.

Rather than being implicit runtime knowledge, behaviour is declared, checked, and referenced explicitly. This keeps meaning consistent across the system and avoids semantic drift between stages.

### Bridges

A bridge is responsible for taking Core IR and connecting it to a concrete execution environment.

Bridges do not reinterpret meaning. They assume the Core IR is authoritative and focus only on how that meaning is realised in a particular context.

The bridge provided in v0.1 is deliberately minimal. It exists only to demonstrate that Core IR can drive real execution end-to-end. It is incomplete, limited, and not intended to represent a production runtime.

Future releases will focus more heavily on bridge development, including different kinds of bridges for different purposes — not all of them necessarily execution-oriented.

Note that the bridge is where much of the real complexity — and many of the possibilities — sit. This asymmetry is intentional. Bridges are expected to hide complexity behind hardened implementations, which in turn simplifies surface languages and reduces the burden on code authors.  

## A function is a function is a function.

Axis does not treat a program as a privileged unit.  In reality a program is just a collection of functions that are linked together, with one special entry point.

Instead, it treats functions as the primary unit of meaning. Core IR represents collections of semantic units rather than monolithic programs with a single entry point. Execution context is supplied by the bridge rather than being baked into the language.  The bridge is free to determine how the entry point is defined and executed.  The core compiler does not care.

This makes it possible to think about systems composed at the function level rather than the program level, and to reuse the same semantic artifacts in different contexts.

## Multiple surfaces, shared meaning

Because meaning is captured in Core IR, surface syntax becomes flexible.

Different surface languages can lower into the same Core IR. Existing bridges continue to work unchanged. This decoupling is foundational rather than incidental.

Axis v0.1 only scratches the surface here, but establishing this invariant is one of the primary goals of the release.

## How the pieces fit together

At a high level, the flow through the system looks like this:

```
Surface Code
     |
     v
+----------------+
|  Core Compiler |
|  (lowering)    |
+----------------+
     |
     v
  Core IR Graph
     |
     +-------------------+
     |                   |
     v                   v
  Registry           IR Viewers
 (semantics)        (inspection)
     |
     v
+----------------+
|     Bridge     |
| (execution or  |
|  other targets)|
+----------------+
```

The Core IR sits at the centre. Everything else either produces it, consumes it, or reasons about it.

We have created a simple shell script that provides the experience of an end-to-end compiler by joining these components into a single flow.

## Direction of future releases

The next release will focus primarily on **language surfaces** — richer syntax, alternative designs, and experimentation around how different surfaces map onto shared semantics.

Bridge development will become a larger focus in subsequent releases. Different bridge types, execution models, and non-execution uses of Core IR are all areas of interest.

There is no fixed end state being aimed at. Axis is evolving by exploration rather than convergence.

## In closing

Axis v0.1 is not a finished language.

It is a worked example of a different way of structuring a programming system — one that treats meaning as explicit, inspectable, and reusable.

Whether that approach proves broadly useful remains an open question.