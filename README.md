# Axis Core

Axis Core is the **semantic core and canonical intermediate representation (IR)**
of the Axis system.

It exists to make **program meaning explicit, inspectable, and stable** —
independent of surface syntax, runtime, or execution strategy.

This repository contains:
- the Core IR model
- the compiler that produces it
- tooling to inspect and reason about semantic structure

It is **not a general-purpose programming language** and **not a complete platform**.
Higher-level language surfaces and tools are layered on top of this core.

---

## Conceptual layout

```text
Surface Language(s)
        │
        ▼
   IR Compiler
        │
        ▼
    Axis Core IR
        │
        ▼
     Bridge(s)
   (Rust, etc.)
````

* **IR Compiler**
  Lowers surface syntax into the canonical Axis Core IR.

* **Axis Core IR**
  A stable, structured representation of program meaning.

* **Bridges**
  Project Core IR into executable or target-specific forms.

This separation is deliberate: execution is treated as a *projection* of meaning,
not its source.

---

## Getting started

If you want to **compile something and inspect the Core IR**, start here:

👉 **[Getting Started](./GETTING_STARTED.md)**

This walks through building the compiler, running it locally,
and examining the IR output.

If you want to understand **why Axis Core is structured this way**, read:

👉 **[Overview](./OVERVIEW.md)**

This explains the design goals, constraints, and architectural decisions
behind Axis Core.

---

## Usage

Axis Core is currently built and invoked locally.

After building the compiler (for example via Cargo),
the executable will be available in the build output directory:

```bash
./core-compiler/target/debug/axis-compiler \
  --sources <file.ax> \
  --registries <registry.axreg>
```

By default, Core IR is emitted into `./coreir/`.

Use `--out` to override the output location.

Use `--view-core-ir` to inspect a textual representation
of the Core IR graph emitted by Axis.

---

## `compile_ax.sh`

`compile_ax.sh` is a small convenience script used during development to
build and run the Axis Core compiler with common arguments.

It exists to:

* simplify local experimentation
* provide a repeatable compile flow
* make IR inspection easier during development

It is **not part of a stable interface** and may change as the core evolves.

---

## Status

Axis Core is an **early, exploratory system**.

Pre-1.0 versions are expected to change substantially as the core model evolves.
The current implementation exists to prove and refine the semantic approach,
not to provide long-term compatibility guarantees.

---

## License

This project is licensed under the MIT License.