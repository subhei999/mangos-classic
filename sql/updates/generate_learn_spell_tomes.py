#!/usr/bin/env python3
"""
Generate SQL to create "spell tome" items by cloning an existing template row
from `item_template` (default: 18600 Tome of Arcane Brilliance) and overriding
only a few fields (entry, name, spells, ScriptName).

Why this exists:
- Manually writing full `INSERT ... SELECT` with all columns is error-prone.
- For thousands of spells, you want: a simple list -> many INSERTs.

Input format (one per line, comments allowed with #):
  spell_id
  spell_id<TAB>item_name
  entry_id<TAB>spell_id<TAB>item_name

Example:
  143\tTome of Fireball (Rank 2)
  116\tTome of Frostbolt (Rank 1)

Usage:
  python3 sql/updates/generate_learn_spell_tomes.py --input spells.tsv --start-entry 90000 > sql/updates/mangos/zXXXX_XX_mangos_custom_tomes.sql
"""

from __future__ import annotations

import argparse
import datetime as _dt
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, List, Optional, Tuple


ITEM_TEMPLATE_CREATE_RE = re.compile(r"^\s*CREATE TABLE\s+`item_template`\s*\(\s*$")
COLUMN_RE = re.compile(r"^\s*`(?P<name>[^`]+)`\s+")


@dataclass(frozen=True)
class TomeSpec:
    entry: int
    spell_id: int
    name: str
    required_level: Optional[int] = None


def _sql_quote(s: str) -> str:
    # MySQL string literal escaping: backslash and single-quote
    return "'" + s.replace("\\", "\\\\").replace("'", "\\'") + "'"


def parse_item_template_columns(schema_sql: Path) -> List[str]:
    """
    Extract column names from the `item_template` CREATE TABLE in sql/base/mangos.sql.
    """
    txt = schema_sql.read_text(encoding="utf-8", errors="replace").splitlines()
    in_table = False
    cols: List[str] = []

    for line in txt:
        if not in_table:
            if ITEM_TEMPLATE_CREATE_RE.search(line):
                in_table = True
            continue

        # End when we hit keys/constraints
        if line.lstrip().startswith("PRIMARY KEY") or line.lstrip().startswith("KEY ") or line.lstrip().startswith(")"):
            break

        m = COLUMN_RE.match(line)
        if m:
            cols.append(m.group("name"))

    if not cols:
        raise RuntimeError(f"Failed to parse item_template columns from {schema_sql}")

    return cols


def parse_input_lines(lines: Iterable[str], start_entry: int) -> List[TomeSpec]:
    """
    Parse input lines into TomeSpec. Supports:
      spell_id
      spell_id<TAB>name
      spell_id<TAB>name<TAB>required_level
      entry_id<TAB>spell_id<TAB>name
      entry_id<TAB>spell_id<TAB>name<TAB>required_level
    """
    specs: List[TomeSpec] = []
    next_entry = start_entry
    seen_spell_ids: set[int] = set()
    seen_names: set[str] = set()
    skipped_dupe_spellid = 0
    skipped_dupe_name = 0
    skipped_zz = 0
    skipped_lvl0 = 0

    for raw in lines:
        line = raw.strip()
        if not line or line.startswith("#"):
            continue

        parts = [p.strip() for p in line.split("\t")]
        if len(parts) == 1:
            spell_id = int(parts[0])
            entry = next_entry
            next_entry += 1
            name = f"Tome of Spell {spell_id}"
            required_level: Optional[int] = None
        elif len(parts) == 2:
            spell_id = int(parts[0])
            entry = next_entry
            next_entry += 1
            name = parts[1]
            required_level = None
        elif len(parts) == 3:
            # spell_id, name, required_level
            spell_id = int(parts[0])
            entry = next_entry
            next_entry += 1
            name = parts[1]
            required_level = int(parts[2]) if parts[2] != "" else None
        elif len(parts) >= 4:
            entry = int(parts[0])
            spell_id = int(parts[1])
            name = parts[2]
            required_level = int(parts[3]) if parts[3] != "" else None
        else:
            raise ValueError(f"Unrecognized input line: {raw!r}")

        # Filter out unwanted names (common "zzOLD..." / placeholder entries in some DBs)
        if name.lower().startswith("tome of zz"):
            skipped_zz += 1
            continue

        # Filter out level 0 rows when TSV provides a required_level column.
        # (If required_level is absent/unknown, we keep the row.)
        if required_level is not None and required_level <= 0:
            skipped_lvl0 += 1
            continue

        # De-duplicate by tome name: keep first occurrence of each name (case-insensitive).
        # This is especially useful when TSV doesn't include rank text and multiple spells
        # would otherwise generate identical item names.
        norm_name = name.strip().lower()
        if norm_name in seen_names:
            skipped_dupe_name += 1
            continue
        seen_names.add(norm_name)

        # Also de-duplicate by spell_id for safety (should be rare in clean inputs).
        if spell_id in seen_spell_ids:
            skipped_dupe_spellid += 1
            continue
        seen_spell_ids.add(spell_id)

        specs.append(TomeSpec(entry=entry, spell_id=spell_id, name=name, required_level=required_level))

    if skipped_zz or skipped_dupe_name or skipped_dupe_spellid:
        sys.stderr.write(
            "Filtered input: "
            f"skipped {skipped_zz} 'Tome of zz*' rows, "
            f"skipped {skipped_dupe_name} duplicate name rows, "
            f"skipped {skipped_dupe_spellid} duplicate spell_id rows.\n"
        )
    if skipped_lvl0:
        sys.stderr.write(f"Filtered input: skipped {skipped_lvl0} rows with required_level=0.\n")

    return specs


def build_insert_sql(
    cols: List[str],
    spec: TomeSpec,
    template_entry: int,
    dummy_spell_id: int,
    script_name: str,
    allowable_class: int,
    allowable_race: Optional[int],
    required_level: Optional[int],
) -> str:
    overrides = {
        "entry": str(spec.entry),
        "name": _sql_quote(spec.name),
        # Remove requirements by default (script is intended to bypass restrictions)
        "AllowableClass": str(allowable_class),
        "spellid_1": str(dummy_spell_id),
        "spelltrigger_1": "0",
        "spellcharges_1": "-1",
        "spellid_2": str(spec.spell_id),
        "spelltrigger_2": "0",
        "spellcharges_2": "0",
        "ScriptName": _sql_quote(script_name),
    }
    if allowable_race is not None:
        overrides["AllowableRace"] = str(allowable_race)
    # Per-row RequiredLevel (from TSV) wins; otherwise use global flag if provided.
    if spec.required_level is not None:
        overrides["RequiredLevel"] = str(spec.required_level)
    elif required_level is not None:
        overrides["RequiredLevel"] = str(required_level)

    insert_cols = ", ".join(f"`{c}`" for c in cols)
    select_exprs: List[str] = []
    for c in cols:
        if c in overrides:
            select_exprs.append(f"{overrides[c]} AS `{c}`")
        else:
            select_exprs.append(f"`{c}`")

    select_list = ", ".join(select_exprs)
    return (
        f"INSERT INTO `item_template` ({insert_cols})\n"
        f"SELECT {select_list}\n"
        f"FROM `item_template`\n"
        f"WHERE `entry` = {template_entry};"
    )


def main(argv: Optional[List[str]] = None) -> int:
    p = argparse.ArgumentParser(description="Generate SQL for learned-spell tome items.")
    p.add_argument("--input", type=Path, help="TSV input file. If omitted, reads from stdin.")
    p.add_argument(
        "--output",
        type=Path,
        default=Path("sql/updates/mangos/z2831_01_mangos_mage_tomes.sql"),
        help="Output .sql path. Use '-' to write to stdout (default: sql/updates/mangos/z2831_01_mangos_mage_tomes.sql).",
    )
    p.add_argument("--schema-sql", type=Path, default=Path("sql/base/mangos.sql"), help="Path to schema file containing item_template definition.")
    p.add_argument("--template-entry", type=int, default=18600, help="Existing item_template entry to clone from (default: 18600).")
    p.add_argument("--start-entry", type=int, default=91000, help="Starting entry ID when input lines do not specify an entry ID (default: 91000).")
    p.add_argument("--dummy-spell-id", type=int, default=483, help="Client-recognized dummy spell used to allow item use (default: 483).")
    p.add_argument("--script-name", type=str, default="item_hardcore_ability", help="Item ScriptName to handle teaching (default: item_hardcore_ability).")
    p.add_argument("--allowable-class", type=int, default=-1, help="AllowableClass override (default: -1 = all classes).")
    p.add_argument("--allowable-race", type=int, default=None, help="AllowableRace override (default: leave unchanged).")
    p.add_argument("--required-level", type=int, default=None, help="RequiredLevel override (default: leave unchanged).")
    args = p.parse_args(argv)

    cols = parse_item_template_columns(args.schema_sql)

    if args.input:
        lines = args.input.read_text(encoding="utf-8", errors="replace").splitlines()
    else:
        lines = sys.stdin.read().splitlines()

    specs = parse_input_lines(lines, start_entry=args.start_entry)
    if not specs:
        sys.stderr.write("No specs found in input.\n")
        return 2

    out = sys.stdout
    out_fh = None
    if str(args.output) != "-":
        out_fh = args.output.open("w", encoding="utf-8", newline="\n")
        out = out_fh

    try:
        now = _dt.datetime.now(_dt.timezone.utc).strftime("%Y-%m-%d %H:%M:%SZ")
        print(f"-- Generated by sql/updates/generate_learn_spell_tomes.py at {now}", file=out)
        print(f"-- Template entry: {args.template_entry}", file=out)
        print(f"-- ScriptName: {args.script_name}", file=out)
        print(f"-- Dummy spell: {args.dummy_spell_id}", file=out)
        print(f"-- AllowableClass: {args.allowable_class}", file=out)
        print(f"-- AllowableRace: {args.allowable_race if args.allowable_race is not None else '(unchanged)'}", file=out)
        print(f"-- RequiredLevel: {args.required_level if args.required_level is not None else '(unchanged)'}", file=out)
        print(file=out)

        entries = ", ".join(str(s.entry) for s in specs)
        print(f"DELETE FROM `item_template` WHERE `entry` IN ({entries});", file=out)
        print(file=out)

        for spec in specs:
            print(f"-- {spec.entry}: {spec.name} (learns spell {spec.spell_id})", file=out)
            print(
                build_insert_sql(
                    cols,
                    spec,
                    args.template_entry,
                    args.dummy_spell_id,
                    args.script_name,
                    allowable_class=args.allowable_class,
                    allowable_race=args.allowable_race,
                    required_level=args.required_level,
                ),
                file=out,
            )
            print(file=out)
    finally:
        if out_fh is not None:
            out_fh.close()

    return 0


if __name__ == "__main__":
    raise SystemExit(main())


