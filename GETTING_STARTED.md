## Getting Started (≈10 minutes)

This guide gets you from a clean machine to a working Axis demo.

Axis v0.1 is a **conceptual proof-of-concept**.
The goal is to demonstrate the Core IR pipeline, not a polished language experience.

---

### Prerequisites

You need:

* **Rust** (stable)
* **Cap’n Proto** (schema compiler)

That’s it.

---

### 1. Install Rust

If you don’t already have Rust:

```bash
curl https://sh.rustup.rs -sSf | sh
```

Restart your shell, then verify:

```bash
rustc --version
cargo --version
```

---

### 2. Install Cap’n Proto

#### Linux (Debian/Ubuntu)

```bash
sudo apt install capnproto
```

#### macOS (Homebrew)

```bash
brew install capnp
```

#### Windows

* Download from: [https://capnproto.org/install.html](https://capnproto.org/install.html)
* Ensure `capnp` is on your `PATH`

Verify:

```bash
capnp --version
```

---

### 3. Build the toolchain

From the repo root:

```bash
./build_all.sh
```

This builds:

* `axis-compiler` (surface → Core IR)
* `axis-rust-bridge` (Core IR → executable)

You should see:

```
✓ Rust components built
```

Warnings are expected in v0.1.

---

### 4. Run the Hello World demo

Compile the example:

```bash
./compile_ax.sh examples/hello.ax
```

This will:

* parse the surface file
* validate semantics
* emit Core IR
* run the rust-bridge to produce an executable

If you see the hello executable in the root project directory.

To view the Core IR graph, run:

```bash
./core-compiler/target/release/axis-compiler --view-core-ir coreir/hello.coreir
```
---

### 5. Inspect the Core IR graph

This IR is the *primary artifact* of Axis.

---

### What’s next?

* Modify `examples/hello.ax`
* Re-run the compiler
* Observe how the Core IR changes

You are encouraged to explore the IR directly.

---

### What this release is (and isn’t)

**This is:**

* a semantics-first proof-of-concept
* a real compiler pipeline
* a concrete Core IR

**This is not:**

* a finished language
* a stable surface syntax
* production tooling

Future releases will focus on richer surfaces and exploration tools.

---

# `docs/dependencies.md`

## Dependencies

Axis v0.1 intentionally keeps dependencies minimal.

---

### Required

#### Rust (stable)

Used for:

* Core compiler
* Core IR handling
* bridge implementation

Install via `rustup`.

---

#### Cap’n Proto

Used for:

* Core IR serialization
* cross-language, cross-platform IR bundles

Only the **schema compiler** is required to build Axis.

Runtime support is provided by language bindings (e.g. Rust `capnp` crate).

---

### Platform support

Axis v0.1 has been tested on:

* Linux
* macOS
* Windows (via Rust + Cap’n Proto)

Core IR bundles are platform-independent.

**Windows is supported at the toolchain level**
  (Rust + Cap’n Proto + portable IR)
**Build scripts are currently Unix shell–based**
**Windows users can build manually**
**Automated Windows scripts are out of scope for v0.1**