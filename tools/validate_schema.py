#!/usr/bin/env python3
"""Validate the composable schema domains against dynograph-foundation's rules.

Mirrors the checks in `dynograph-foundation/crates/dynograph-core/src/schema.rs`
(`Schema::from_multiple_yamls` → `merge` → `validate`) so we can confirm the
domains load and merge cleanly WITHOUT a Rust toolchain. The two hard rules
`validate()` enforces are:

  1. every edge `from`/`to` endpoint is `*` or a declared node type (after merge);
  2. `fulltext: true` is only valid on `string` properties.

Plus serde-level constraints: property `type` ∈ the PropertyType enum, and
`resolution.strategy` ∈ the ResolutionStrategy enum. Everything else (extra keys
like `description`) is accepted, matching serde's non-strict parse.

If dynograph-core's schema.rs changes, update this to match.

Usage:  python3 tools/validate_schema.py [schema_dir]   (default: ./schema)
Exit 0 = valid, 1 = errors.
"""
from __future__ import annotations

import glob
import os
import sys

try:
    import yaml
except ModuleNotFoundError:
    sys.exit("PyYAML required: pip install pyyaml")

# From dynograph-core/src/schema.rs — PropertyType (serde rename_all lowercase +
# the explicit list:string rename) and ResolutionStrategy (snake_case).
VALID_TYPES = {"string", "int", "float", "bool", "datetime", "enum", "list:string"}
VALID_STRATEGIES = {"fuzzy_then_vector", "exact", "fuzzy_only", "vector_only"}


def endpoints(v):
    return v if isinstance(v, list) else [v]


def main() -> int:
    schema_dir = sys.argv[1] if len(sys.argv) > 1 else "schema"
    files = sorted(glob.glob(os.path.join(schema_dir, "*.yaml")))
    if not files:
        print(f"No schema files in {schema_dir}/")
        return 1

    nodes: dict[str, str] = {}   # node type -> first domain file
    edges: dict[str, tuple] = {} # edge type -> (file, def)
    errors: list[str] = []
    warns: list[str] = []
    domains = []

    for f in files:
        doc = yaml.safe_load(open(f))
        s = doc.get("schema", doc)  # unwrap top-level `schema:` key
        domains.append((os.path.basename(f), s.get("name"), s.get("version")))

        for n, d in (s.get("node_types") or {}).items():
            if n in nodes:
                errors.append(f"{f}: duplicate node type '{n}' (also in {nodes[n]})")
            nodes[n] = f
            for pn, pd in (d.get("properties") or {}).items():
                t = pd.get("type", "string")
                if t not in VALID_TYPES:
                    errors.append(f"{f}: {n}.{pn} invalid type '{t}'")
                if pd.get("fulltext") and t != "string":
                    errors.append(f"{f}: {n}.{pn} fulltext:true on non-string type '{t}'")
                if t == "enum" and not pd.get("values"):
                    warns.append(f"{f}: {n}.{pn} enum without values")
            r = d.get("resolution")
            if r and r.get("strategy") not in VALID_STRATEGIES:
                errors.append(f"{f}: {n} resolution.strategy '{r.get('strategy')}' invalid")
            ef = d.get("embedding_field")
            if ef and ef not in (d.get("properties") or {}):
                errors.append(f"{f}: {n} embedding_field '{ef}' is not a declared property")

        for e, d in (s.get("edge_types") or {}).items():
            if e in edges:
                errors.append(f"{f}: duplicate edge type '{e}' (also in {edges[e][0]})")
            edges[e] = (f, d)
            for pn, pd in (d.get("properties") or {}).items():
                t = pd.get("type", "string")
                if t not in VALID_TYPES:
                    errors.append(f"{f}: {e}.{pn} invalid edge-property type '{t}'")
                if pd.get("fulltext") and t != "string":
                    errors.append(f"{f}: {e}.{pn} fulltext:true on non-string edge property")

    # Endpoint check runs AFTER all node types are collected (post-merge), exactly
    # like dynograph's from_multiple_yamls → validate.
    for e, (f, d) in edges.items():
        for side in ("from", "to"):
            for nm in endpoints(d.get(side)):
                if nm != "*" and nm not in nodes:
                    errors.append(
                        f"{f}: edge '{e}' {side} endpoint references unknown node type '{nm}'"
                    )

    print("Domains merged:")
    for fn, nm, ver in domains:
        print(f"  - {fn:16} name={nm!r:14} v{ver}")
    print(f"\nMerged schema: {len(nodes)} node types, {len(edges)} edge types")
    print("Node types:", ", ".join(sorted(nodes)))
    print("Edge types:", ", ".join(sorted(edges)))

    if warns:
        print("\nWarnings:")
        for w in warns:
            print("  -", w)
    if errors:
        print("\nERRORS:")
        for x in errors:
            print("  -", x)
        print(f"\nFAILED: {len(errors)} error(s).")
        return 1
    print("\nOK: schema merges and validates against dynograph-core rules.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
