# Creating Tome Items - What We Learned

## Key Findings

### 1. **Tome of Arcane Brilliance Works WITHOUT a Script**
- Uses spell 23030 with `Effect1 = 36` (LEARN_SPELL)
- `EffectTriggerSpell1 = 23028` (the spell being learned)
- `ScriptName = ''` (empty - no script needed)
- This works because spell 23030 exists in client DBC files

### 2. **Custom Spell IDs (100000+) Don't Work**
- Client doesn't recognize spell IDs not in DBC files
- Items using custom spell IDs appear "static" (unusable)
- Tried: 100001, 23031 - both failed
- **Solution**: Use script approach instead

### 3. **Script Approach Works**
- Use existing script: `item_hardcore_ability`
- Script file: `src/game/AI/ScriptDevAI/scripts/world/hardcore_ability_items.cpp`
- Script reads `spellid_2` (item_template `spellid_2` / index 1) to determine which spell to teach
- Script **consumes the item itself** (`DestroyItemCount`) and returns `true` (it handles the use)

### 4. **Required Item Template Fields**

```sql
class = 9                    -- Book (not 0 = Consumable)
subclass = 0                 -- Other
displayid = 1103             -- Tome icon (not 1644 = shield)
Quality = 3                  -- Rare
Flags = 64                   -- Match Arcane Brilliance
InventoryType = 0            -- Non-equippable
spellid_1 = 483              -- Learning spell (client recognizes this)
spelltrigger_1 = 0           -- ON_USE
spellcharges_1 = -1          -- Single use (consumes item)
spellid_2 = 143              -- Actual spell to learn (Fireball Rank 2)
spelltrigger_2 = 0           -- ON_USE (script reads this but doesn't trigger)
spellcharges_2 = 0           -- No charges (script handles consumption)
ScriptName = 'item_hardcore_ability'  -- MUST match exactly (case-sensitive)
```

### 5. **Critical Points**

- **spelltrigger_2 cannot be -1** - must be 0 or other valid value
- **ScriptName must match exactly** - case-sensitive, no spaces
- **spellid_1 must be client-recognized** - use 483 (Learning spell)
- **Server restart required** - not just reload
- **Remove and re-add item** after changes
- **The spell must exist server-side** - the script validates the spell ID and will reject invalid IDs

### 6. **Script Registration**

- Script is registered in: `src/game/AI/ScriptDevAI/system/ScriptLoader.cpp`
- Line 23: `extern void AddSC_hardcore_ability_items();`
- Line 229: `AddSC_hardcore_ability_items();`
- Script must be compiled with ScriptDevAI enabled

### 7. **Template for Creating New Tome Items**

```sql
-- Step 1: Create the item (copy from Tome of Arcane Brilliance structure)
INSERT INTO `item_template` 
SELECT 
    90002 AS `entry`,                    -- Your item ID
    `class`, `subclass`,                 -- Keep class=9, subclass=0
    'Tome of [Spell Name]' AS `name`,   -- Your item name
    `displayid`,                         -- Keep displayid=1103
    `Quality`, `Flags`, `BuyCount`, `BuyPrice`, `SellPrice`,
    `InventoryType`, `AllowableClass`, `AllowableRace`,
    `ItemLevel`, `RequiredLevel`,
    -- ... (copy all other fields from entry 18600)
    483 AS `spellid_1`,                 -- Learning spell (client recognizes)
    0 AS `spelltrigger_1`,              -- ON_USE
    -1 AS `spellcharges_1`,             -- Single use
    [SPELL_ID_TO_LEARN] AS `spellid_2`, -- The spell to teach
    0 AS `spelltrigger_2`,              -- ON_USE
    0 AS `spellcharges_2`,              -- No charges
    -- ... (copy remaining fields)
    'item_hardcore_ability' AS `ScriptName`  -- Use the script!
FROM `item_template`
WHERE `entry` = 18600;
```

## Bulk Creation (100s / 1000s of tomes)

Writing the full `INSERT ... SELECT` for `item_template` is intentionally verbose because the table has many columns. For scale, generate the SQL from a simple spell list:

- **Generator**: `sql/updates/generate_learn_spell_tomes.py`
- **Input**: TSV file (one per line)
  - `spell_id`
  - `spell_id<TAB>item_name`
  - `entry_id<TAB>spell_id<TAB>item_name`

Example `spells.tsv`:

```text
# spell_id<TAB>name
143	Tome of Fireball (Rank 2)
116	Tome of Frostbolt (Rank 1)
```

Generate SQL (writes to stdout):

```bash
python3 sql/updates/generate_learn_spell_tomes.py --input spells.tsv --start-entry 90000 > sql/updates/mangos/zXXXX_XX_mangos_custom_tomes.sql
```

One-off (single item) template:

- `sql/updates/template_create_learn_spell_tome.sql`

## Summary

**Why Tome of Arcane Brilliance works without a script:**
- Uses spell 23030 which exists in client DBC files
- Client recognizes and validates the spell
- Server executes the learn effect

**Why custom spells don't work:**
- Client DBC files don't contain custom spell IDs
- Client rejects items with unknown spells
- Item appears "static" (unusable)

**Why script approach works:**
- Uses spell 483 (client-recognized learning spell)
- Client validates spell 483 and allows item use
- Script intercepts and teaches spell from `spellid_2`
- Bypasses all client-side restrictions

## Files Created

- `template_create_learn_spell_tome.sql` - One-off SQL template for creating tomes
- `generate_learn_spell_tomes.py` - Bulk SQL generator (spell list -> many tomes)
- `fix_item_90001_use_script.sql` - Working solution
- `verify_item_90001_matches_18600.sql` - Verification queries
- Various debug/fix queries for troubleshooting

