/* Hardcore Mode - Ability Training Items
 * This script allows items to teach any spell in the game regardless of class/race.
 * Items store the target spell ID in their spellid_2 field.
 *
 * Part of the Hardcore PvP Mode feature set.
 */

#include "AI/ScriptDevAI/include/sc_common.h"
#include "Entities/Item.h"
#include "Entities/Player.h"
#include "Spells/SpellMgr.h"

/*
 * Item: Hardcore Ability Item
 * Uses spellid_2 from item_template to determine which spell to teach
 * Bypasses all class/race/level restrictions
 *
 * DB SETUP INSTRUCTIONS:
 * To avoid "Target Required" errors for targeted spells (like Pyroblast):
 * 1. Set spellid_1 (Index 0) to a Dummy/Visual spell (e.g., 483). Set spelltrigger_1 = 0 (ON_USE).
 * 2. Set spellid_2 (Index 1) to the Ability Spell ID you want to teach.
 * 3. The script reads spellid_2 to learn. The client uses spellid_1 checks (which passes).
 */
bool ItemUse_hardcore_ability_item(Player* pPlayer, Item* pItem, const SpellCastTargets& /*pTargets*/)
{
    if (!pPlayer || !pItem)
        return false;

    const ItemPrototype* proto = pItem->GetProto();
    if (!proto)
        return false;

    // Get the spell to learn from spellid_2 (index 1)
    uint32 spellToLearn = proto->Spells[1].SpellId;

    if (!spellToLearn)
    {
        // Fallback: try spellid_1 if spellid_2 is not set
        spellToLearn = proto->Spells[0].SpellId;
    }

    if (!spellToLearn)
    {
        pPlayer->GetSession()->SendNotification("This item has no spell to teach!");
        return true; // Return true to prevent default item use
    }

    // Check if spell exists
    SpellEntry const* spellInfo = sSpellTemplate.LookupEntry<SpellEntry>(spellToLearn);
    if (!spellInfo)
    {
        pPlayer->GetSession()->SendNotification("Invalid spell ID: %u", spellToLearn);
        return true;
    }

    // Check if player already knows this spell
    if (pPlayer->HasSpell(spellToLearn))
    {
        pPlayer->GetSession()->SendNotification("You already know %s!", spellInfo->SpellName[0]);
        return true;
    }

    // Learn the spell directly, bypassing all restrictions
    pPlayer->learnSpell(spellToLearn, false);

    // Send success message
    pPlayer->GetSession()->SendNotification("You have learned %s!", spellInfo->SpellName[0]);

    // Play a visual effect (learning spell visual)
    pPlayer->PlaySpellVisual(362); // Standard learn visual

    // Destroy one charge of the item
    pPlayer->DestroyItemCount(pItem->GetEntry(), 1, true);

    return true; // Return true to indicate we handled the item use
}

void AddSC_hardcore_ability_items()
{
    Script* pNewScript = new Script;
    pNewScript->Name = "item_hardcore_ability";
    pNewScript->pItemUse = &ItemUse_hardcore_ability_item;
    pNewScript->RegisterSelf();
}
