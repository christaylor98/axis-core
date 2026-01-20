# Core IR 0.1 — Canonical Interchange Format

**Status:** **Canonical for Core IR version 0.1. Implementations MUST conform.**

---

## Overview

Core IR 0.1 is the **canonical, serialized semantic authority** for Axis program semantics and the interchange format consumed by bridges and tooling. It is intentionally **minimal**, **versioned**, and designed for **reliable consumption by downstream consumers**.

---

## Authority and Intent

* **Core IR is universal and authoritative** for consumers that accept Core IR bundles.
* **Core IR is inert data**: it carries **no execution semantics**, **no optimization semantics**, and **no operational behaviour**.
* **Core IR is bridge-consumable**: bridges ingest Core IR bundles as **immutable input** and use them for target-specific lowering, code generation, or analysis.

---

## Node Set (0.1)

The Core IR 0.1 node set is **fixed and closed**.

The **only** node kinds in Core IR 0.1 are:

* `CIntLit`
* `CBoolLit`
* `CUnitLit`
* `CLam`
* `CLet`
* `CIf`
* `CVar`
* `CApp`

All higher-level Axis constructs **MUST** be fully desugared into this node set **before** a Core IR 0.1 bundle is emitted.

No other node kinds exist in Core IR 0.1; consumers **MUST NOT** expect or rely on additional node kinds.

---

## Bundle and Encoding Rules

* Top-level bundles **MUST** include:

  * a `version` field (string) identifying the Core IR version
  * a `core_term` field containing the term root

* Common auxiliary fields (for tooling) include:

  * `entrypoint_name`
  * `entrypoint_id`
  * `string_table`

  Consumers **MUST** treat `core_term` as authoritative for semantics.

* Core IR 0.1 uses a **tagged representation**:

  * Every node object **MUST** include a `tag` field naming the node kind
  * `tag` is the **canonical node discriminator** for version 0.1

* For compatibility:

  * Consumers **MAY** accept `kind` as an alias for `tag`
  * If both `tag` and `kind` are present, consumers **MUST** treat `tag` as authoritative

* Field names in the bundle are **stable and authoritative** for the format.

* Consumers **MUST** ignore unknown fields encountered in a bundle or node.
  Unknown fields carry **no semantic authority** in version 0.1.

---

## Versioning Rules

* This document defines **Core IR version `"0.1"`**.
* Each Core IR bundle **MUST** include a top-level `version` field.
* Consumers **MUST** reject bundles with an **unknown major version**.

Compatibility for minor or patch-level changes is **out of scope** for this document and must be handled by version negotiation policies in later revisions.

---

## Spans and Diagnostics

* `span` (source-location metadata) is **optional** on nodes.
* Spans are **diagnostic only** and **MUST NOT** affect program semantics.
* Consumers may use spans to produce diagnostics, but spans carry **no semantic authority** and **MUST NOT** be relied on for correctness.

---

## Bridge Constraints and Consumer Responsibilities

* Bridges and other consumers **MUST NOT** validate, execute, or reinterpret Core IR semantics beyond what the node kinds express.
* Bridges **MUST** treat Core IR bundles as **immutable input** and are responsible for any target-specific lowering or code generation.
* Consumers **MUST** ignore unknown fields and optional metadata unless explicitly documented for a compatible version.
* Consumers **MUST** enforce the **closed node set** and the **versioning rules** described above.

---

## Section B — Explicit Non-Goals for Core IR 0.1

* **Not an execution format**
  Core IR 0.1 is not intended to be executed or to define runtime semantics.

* **Not an optimizer IR**
  Core IR 0.1 does not specify optimization passes or transformation semantics.

* **Not extensible within 0.1**
  Adding node kinds requires a **new major version**.

* **Not a validation contract**
  Core IR 0.1 is not a schema-driven validator for higher-level well-formedness beyond the node, encoding, and versioning rules above.

---

## Section C — Notes on Future Evolution (Non-Binding)

* Future versions may extend the node set or metadata model; such changes will require a **major-version increment** and explicit compatibility guidance.
* A formal, machine-readable schema may be provided for later versions to aid robust parsing and validation.
* Version negotiation and compatibility policies can be specified in later revisions if automated compatibility is required.