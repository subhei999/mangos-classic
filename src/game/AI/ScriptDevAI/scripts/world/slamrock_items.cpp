/*
 * Slamrock (POC)
 * - Server-side item that can be used on gear (client targets an item via existing enchant spell cursor)
 * - For POC: applies a fixed +1 Strength via existing enchant spell (7782) enchant id
 * - Consumes one Slamrock on success
 */

#include "AI/ScriptDevAI/include/sc_common.h"

#include <algorithm>
#include <map>
#include <memory>
#include <string>
#include <vector>

#include "Database/DatabaseEnv.h"
#include "Entities/Item.h"
#include "Entities/Player.h"
#include "Entities/ItemPrototype.h"
#include "Log/Log.h"
#include "Server/DBCEnums.h"
#include "Server/DBCStores.h"
#include "Server/SQLStorages.h"
#include "Spells/SpellAuraDefines.h"
#include "Spells/Spell.h"
#include "Util/Util.h"

namespace
{
    // Use custom spell for client-side "use on item" targeting cursor (allows both weapons and armor).
    // Custom spell 50000 has EquippedItemClass = -1 to allow any item class.
    constexpr uint32 SLAMROCK_TARGETING_SPELL = 33394;

    // Store slamrock data in PROP slots so we don't overwrite PERM or TEMP enchants.
    // NOTE: These slots are used by RandomSuffix/RandomProperty items. We therefore reject such items.
    constexpr uint32 SLAMROCK_MARKER_ENCHANT_ID = 900000; // SpellItemEnchantment.dbc (client): "|cffff2020Slammed|r"
    constexpr EnchantmentSlot SLAMROCK_MARKER_SLOT = PROP_ENCHANTMENT_SLOT_0;
    constexpr EnchantmentSlot SLAMROCK_MODIFIER_SLOTS[] = { PROP_ENCHANTMENT_SLOT_1, PROP_ENCHANTMENT_SLOT_2, PROP_ENCHANTMENT_SLOT_3 };
    constexpr EnchantmentSlot SLAMROCK_ALL_PROP_SLOTS[] = { PROP_ENCHANTMENT_SLOT_0, PROP_ENCHANTMENT_SLOT_1, PROP_ENCHANTMENT_SLOT_2, PROP_ENCHANTMENT_SLOT_3 };
    constexpr uint32 SLAMROCK_MAX_MODIFIERS = 3;
    constexpr uint32 SLAMROCK_UPGRADE_CHANCE_PCT = 2;
    constexpr uint32 SLAMROCK_UPGRADE_MAX_ILVL_DELTA = 5;
    constexpr uint32 SLAMROCK_DOWNGRADE_CHANCE_PCT = 25;

    bool IsReasonableGearTarget(Item* target)
    {
        if (!target)
            return false;
        ItemPrototype const* proto = target->GetProto();
        if (!proto)
            return false;

        // Basic filter: must be equippable gear (exclude containers, consumables, etc.)
        return proto->InventoryType != INVTYPE_NON_EQUIP && proto->InventoryType != INVTYPE_BAG;
    }

    int32 ItemModToStatIndex(uint32 itemModType)
    {
        switch (itemModType)
        {
            case ITEM_MOD_STRENGTH:  return STAT_STRENGTH;
            case ITEM_MOD_AGILITY:   return STAT_AGILITY;
            case ITEM_MOD_STAMINA:   return STAT_STAMINA;
            case ITEM_MOD_INTELLECT: return STAT_INTELLECT;
            case ITEM_MOD_SPIRIT:    return STAT_SPIRIT;
            default:                 return -1;
        }
    }

    uint32 FindStatEnchantmentId(uint32 statType, int32 statValue)
    {
        if (statValue <= 0)
            return 0;

        // Vanilla DBCs commonly implement "stat enchants" as ITEM_ENCHANTMENT_TYPE_EQUIP_SPELL (3)
        // that casts a spell (EffectArg) which applies SPELL_AURA_MOD_STAT. Some DBCs use
        // ITEM_ENCHANTMENT_TYPE_STAT (5) directly. Support both.
        int32 statIndex = ItemModToStatIndex(statType);

        for (uint32 i = 0; i < sSpellItemEnchantmentStore.GetNumRows(); ++i)
        {
            SpellItemEnchantmentEntry const* ench = sSpellItemEnchantmentStore.LookupEntry(i);
            if (!ench)
                continue;

            for (int s = 0; s < 3; ++s)
            {
                // Direct stat enchant in DBC
                if (ench->type[s] == ITEM_ENCHANTMENT_TYPE_STAT)
                {
                    if (ench->spellid[s] == statType && int32(ench->amount[s]) == statValue)
                        return ench->ID;
                    continue;
                }

                // Equip-spell enchant in DBC
                if (ench->type[s] == ITEM_ENCHANTMENT_TYPE_EQUIP_SPELL && statIndex >= 0)
                {
                    uint32 spellId = ench->spellid[s];
                    if (!spellId)
                        continue;

                    SpellEntry const* spellInfo = sSpellTemplate.LookupEntry<SpellEntry>(spellId);
                    if (!spellInfo)
                        continue;

                    for (uint8 eff = 0; eff < MAX_EFFECT_INDEX; ++eff)
                    {
                        if (spellInfo->EffectApplyAuraName[eff] != SPELL_AURA_MOD_STAT)
                            continue;
                        if (spellInfo->EffectMiscValue[eff] != statIndex)
                            continue;
                        if (spellInfo->CalculateSimpleValue(SpellEffectIndex(eff)) != statValue)
                            continue;
                        return ench->ID;
                    }
                }
            }
        }
        return 0;
    }

    struct StatEnchantCandidate
    {
        uint32 enchantId;
        int32  statValue;
    };

    bool IsWeaponTarget(Item* target)
    {
        if (!target)
            return false;
        ItemPrototype const* proto = target->GetProto();
        if (!proto)
            return false;
        return proto->Class == ITEM_CLASS_WEAPON;
    }

    bool EnchantHasUsableEffectForTarget(SpellItemEnchantmentEntry const* ench, bool isWeaponTarget)
    {
        if (!ench)
            return false;

        for (int s = 0; s < 3; ++s)
        {
            uint32 type = ench->type[s];
            uint32 arg = ench->spellid[s];

            switch (type)
            {
                case ITEM_ENCHANTMENT_TYPE_NONE:
                    break;
                case ITEM_ENCHANTMENT_TYPE_STAT:
                case ITEM_ENCHANTMENT_TYPE_RESISTANCE:
                    return true;
                case ITEM_ENCHANTMENT_TYPE_DAMAGE:
                case ITEM_ENCHANTMENT_TYPE_TOTEM:
                    if (isWeaponTarget)
                        return true;
                    break;
                case ITEM_ENCHANTMENT_TYPE_EQUIP_SPELL:
                case ITEM_ENCHANTMENT_TYPE_COMBAT_SPELL:
                    if (arg && sSpellTemplate.LookupEntry<SpellEntry>(arg))
                        return true;
                    break;
                default:
                    break;
            }
        }

        return false;
    }

    // --- Whitelist table (World DB) ---
    // Goal: let you prune/tune the roll table in HeidiSQL without rebuilding.
    // NOTE: The table is created/populated via DB updates (not by this script).
    constexpr char const* SLAMROCK_WHITELIST_TABLE = "slamrock_enchant_whitelist";

    struct SlamrockWhitelistRow
    {
        uint32 enchantId;
        std::string groupKey;
        uint16 rank;
        uint16 minIlvl;
        uint16 weight;
    };

    struct SlamrockWhitelistCache
    {
        bool attemptedLoad = false;
        bool loaded = false;
        std::vector<SlamrockWhitelistRow> weapon;
        std::vector<SlamrockWhitelistRow> armor;
    };

    SlamrockWhitelistCache& GetWhitelistCache()
    {
        static SlamrockWhitelistCache cache;
        return cache;
    }

    void LoadWhitelistFromDB()
    {
        SlamrockWhitelistCache& cache = GetWhitelistCache();
        cache.weapon.clear();
        cache.armor.clear();

        // Load enabled rows only, filtered by target flags.
        auto loadSide = [&](bool isWeapon, std::vector<SlamrockWhitelistRow>& out)
        {
            std::unique_ptr<QueryResult> result(WorldDatabase.PQuery(
                "SELECT enchant_id, group_key, rank, min_ilvl, weight "
                "FROM %s "
                "WHERE enabled=1 AND %s=1",
                SLAMROCK_WHITELIST_TABLE,
                isWeapon ? "can_apply_to_weapon" : "can_apply_to_armor"));

            if (!result)
                return;

            out.reserve(result->GetRowCount());
            do
            {
                Field* f = result->Fetch();
                SlamrockWhitelistRow row;
                row.enchantId = f[0].GetUInt32();
                row.groupKey = f[1].GetCppString();
                row.rank = uint16(f[2].GetUInt16());
                row.minIlvl = uint16(f[3].GetUInt16());
                row.weight = uint16(f[4].GetUInt16());
                out.push_back(row);
            }
            while (result->NextRow());
        };

        loadSide(true, cache.weapon);
        loadSide(false, cache.armor);
        cache.loaded = true;
    }

    static std::string GetEffectiveGroupKey(SlamrockWhitelistRow const& row)
    {
        // If group_key is blank, treat the enchant itself as its own group.
        if (!row.groupKey.empty())
            return row.groupKey;
        return std::to_string(row.enchantId);
    }

    bool PickWeightedGroupFromWhitelist(std::vector<SlamrockWhitelistRow> const& pool, uint32 itemLevel, std::vector<std::string> const& excludeGroups, std::string& outGroupKey)
    {
        // Group weight is the max row.weight inside the group, so multiple ranks don't multiply odds.
        std::map<std::string, uint32> groupWeights;
        for (SlamrockWhitelistRow const& row : pool)
        {
            if (row.weight == 0 || row.minIlvl > itemLevel)
                continue;

            std::string key = GetEffectiveGroupKey(row);
            if (!excludeGroups.empty())
            {
                bool excluded = false;
                for (std::string const& ex : excludeGroups)
                {
                    if (ex == key)
                    {
                        excluded = true;
                        break;
                    }
                }
                if (excluded)
                    continue;
            }

            auto itr = groupWeights.find(key);
            if (itr == groupWeights.end())
                groupWeights[key] = row.weight;
            else if (row.weight > itr->second)
                itr->second = row.weight;
        }

        uint32 totalWeight = 0;
        for (auto const& kv : groupWeights)
            totalWeight += kv.second;

        if (totalWeight == 0 || groupWeights.empty())
            return false;

        uint32 roll = urand(1, totalWeight);
        uint32 running = 0;
        for (auto const& kv : groupWeights)
        {
            running += kv.second;
            if (roll <= running)
            {
                outGroupKey = kv.first;
                return true;
            }
        }

        return false;
    }

    bool PickRandomEligibleRankEnchantForGroup(std::vector<SlamrockWhitelistRow> const& pool, uint32 itemLevel, std::string const& groupKey, uint32& outEnchantId)
    {
        // "1 chance per eligible rank": choose uniformly among eligible rows in the group.
        // Eligibility is controlled by min_ilvl (and weight > 0).
        uint32 eligibleCount = 0;
        for (SlamrockWhitelistRow const& row : pool)
        {
            if (row.weight == 0 || row.minIlvl > itemLevel)
                continue;

            if (GetEffectiveGroupKey(row) != groupKey)
                continue;

            ++eligibleCount;
        }

        if (eligibleCount == 0)
            return false;

        uint32 roll = urand(1, eligibleCount);
        uint32 seen = 0;
        for (SlamrockWhitelistRow const& row : pool)
        {
            if (row.weight == 0 || row.minIlvl > itemLevel)
                continue;

            if (GetEffectiveGroupKey(row) != groupKey)
                continue;

            ++seen;
            if (seen >= roll)
            {
                outEnchantId = row.enchantId;
                return true;
            }
        }

        return false;
    }

    std::vector<uint32> const& GetAllEnchantCandidatesForTarget(Item* targetItem)
    {
        static bool initialized = false;
        static std::vector<uint32> weaponEnchants;
        static std::vector<uint32> armorEnchants;

        if (!initialized)
        {
            weaponEnchants.reserve(2048);
            armorEnchants.reserve(2048);

            for (uint32 i = 0; i < sSpellItemEnchantmentStore.GetNumRows(); ++i)
            {
                SpellItemEnchantmentEntry const* ench = sSpellItemEnchantmentStore.LookupEntry(i);
                if (!ench)
                    continue;

                // Reserve this enchant for the "Slammed" marker only.
                if (ench->ID == SLAMROCK_MARKER_ENCHANT_ID)
                    continue;

                if (EnchantHasUsableEffectForTarget(ench, true))
                    weaponEnchants.push_back(ench->ID);
                if (EnchantHasUsableEffectForTarget(ench, false))
                    armorEnchants.push_back(ench->ID);
            }

            initialized = true;
        }

        return IsWeaponTarget(targetItem) ? weaponEnchants : armorEnchants;
    }

    struct ItemTypeKey
    {
        uint32 itemClass;
        uint32 subClass;
        uint32 inventoryType;

        bool operator<(ItemTypeKey const& other) const
        {
            if (itemClass != other.itemClass) return itemClass < other.itemClass;
            if (subClass != other.subClass) return subClass < other.subClass;
            return inventoryType < other.inventoryType;
        }
    };

    uint32 PickDowngradeEntry(ItemPrototype const* targetProto, uint32* outSameTypeTotal = nullptr, uint32* outLowerCount = nullptr)
    {
        if (!targetProto || targetProto->ItemLevel == 0)
            return 0;

        static bool initialized = false;
        static std::map<ItemTypeKey, std::vector<std::pair<uint32, uint32>>> byType; // (ItemLevel, entry)

        if (!initialized)
        {
            for (uint32 entry = 1; entry < sItemStorage.GetMaxEntry(); ++entry)
            {
                ItemPrototype const* proto = sItemStorage.LookupEntry<ItemPrototype>(entry);
                if (!proto)
                    continue;

                // Only items that can be equipped as "gear".
                if (proto->InventoryType == INVTYPE_NON_EQUIP || proto->InventoryType == INVTYPE_BAG)
                    continue;

                ItemTypeKey key{ proto->Class, proto->SubClass, proto->InventoryType };
                byType[key].push_back({ proto->ItemLevel, proto->ItemId });
            }

            for (auto& kv : byType)
            {
                auto& vec = kv.second;
                std::sort(vec.begin(), vec.end(), [](auto const& a, auto const& b) { return a.first < b.first; });
            }

            initialized = true;
        }

        ItemTypeKey key{ targetProto->Class, targetProto->SubClass, targetProto->InventoryType };
        auto itr = byType.find(key);
        if (itr == byType.end())
            return 0;

        auto const& vec = itr->second;
        if (outSameTypeTotal)
            *outSameTypeTotal = uint32(vec.size());
        if (vec.empty())
            return 0;

        // Collect candidates with strictly lower item level.
        std::vector<uint32> candidates;
        candidates.reserve(64);
        for (auto const& it : vec)
        {
            if (it.first >= targetProto->ItemLevel)
                break;
            if (it.second == targetProto->ItemId)
                continue;
            candidates.push_back(it.second);
        }

        if (candidates.empty())
            return 0;

        if (outLowerCount)
            *outLowerCount = uint32(candidates.size());

        return candidates[urand(0, candidates.size() - 1)];
    }

    uint32 PickUpgradeEntrySameClass(ItemPrototype const* targetProto, uint32 maxIlvlDelta, uint32* outSameClassTotal = nullptr, uint32* outEligibleCount = nullptr)
    {
        if (!targetProto)
            return 0;

        uint32 targetIlvl = targetProto->ItemLevel;
        uint32 maxIlvl = targetIlvl + maxIlvlDelta;

        static bool initialized = false;
        static std::map<uint32, std::vector<std::pair<uint32, uint32>>> byClass; // (ItemLevel, entry)

        if (!initialized)
        {
            for (uint32 entry = 1; entry < sItemStorage.GetMaxEntry(); ++entry)
            {
                ItemPrototype const* proto = sItemStorage.LookupEntry<ItemPrototype>(entry);
                if (!proto)
                    continue;

                if (proto->InventoryType == INVTYPE_NON_EQUIP || proto->InventoryType == INVTYPE_BAG)
                    continue;

                byClass[proto->Class].push_back({ proto->ItemLevel, proto->ItemId });
            }

            for (auto& kv : byClass)
            {
                auto& vec = kv.second;
                std::sort(vec.begin(), vec.end(), [](auto const& a, auto const& b) { return a.first < b.first; });
            }

            initialized = true;
        }

        auto itr = byClass.find(targetProto->Class);
        if (itr == byClass.end())
            return 0;

        auto const& vec = itr->second;
        if (outSameClassTotal)
            *outSameClassTotal = uint32(vec.size());
        if (vec.empty())
            return 0;

        std::vector<uint32> candidates;
        candidates.reserve(64);
        for (auto const& it : vec)
        {
            if (it.first < targetIlvl)
                continue;
            if (it.first > maxIlvl)
                break;
            if (it.second == targetProto->ItemId)
                continue;
            candidates.push_back(it.second);
        }

        if (candidates.empty())
            return 0;

        if (outEligibleCount)
            *outEligibleCount = uint32(candidates.size());

        return candidates[urand(0, candidates.size() - 1)];
    }

    bool TryDowngradeItemInPlace(Player* player, Item* targetItem, uint32 newEntry, InventoryResult* outFailReason = nullptr, bool* outWasEquipped = nullptr)
    {
        if (!player || !targetItem || !newEntry)
            return false;

        uint8 bag = targetItem->GetBagSlot();
        uint8 slot = targetItem->GetSlot();
        if (outWasEquipped)
            *outWasEquipped = player->IsEquipmentPos(bag, slot);

        // Pre-check we can create/store/equip the replacement before destroying the old item.
        if (player->IsEquipmentPos(bag, slot))
        {
            uint16 dest;
            // Allow "swap" so checks can pass even though the slot is currently occupied.
            InventoryResult msg = player->CanEquipNewItem(slot, dest, newEntry, true);
            if (msg != EQUIP_ERR_OK)
            {
                if (outFailReason)
                    *outFailReason = msg;
                return false;
            }

            player->DestroyItem(bag, slot, true);
            return player->EquipNewItem(dest, newEntry, true) != nullptr;
        }

        if (player->IsInventoryPos(bag, slot))
        {
            // We can't check "store into exact slot" while the old item still occupies it.
            // First verify we can store somewhere, then after destroying try to store back to the same position.
            ItemPosCountVec destAny;
            InventoryResult msg = player->CanStoreNewItem(NULL_BAG, NULL_SLOT, destAny, newEntry, 1);
            if (msg != EQUIP_ERR_OK)
            {
                if (outFailReason)
                    *outFailReason = msg;
                return false;
            }

            player->DestroyItem(bag, slot, true);

            ItemPosCountVec destSame;
            msg = player->CanStoreNewItem(bag, slot, destSame, newEntry, 1);
            if (msg == EQUIP_ERR_OK)
                return player->StoreNewItem(destSame, newEntry, true, Item::GenerateItemRandomPropertyId(newEntry)) != nullptr;

            // Fallback: store wherever we already proved we can.
            return player->StoreNewItem(destAny, newEntry, true, Item::GenerateItemRandomPropertyId(newEntry)) != nullptr;
        }

        return false;
    }

    void CollectStatEnchantCandidates(uint32 statType, int32 minValue, int32 maxValue, std::vector<StatEnchantCandidate>& out)
    {
        if (minValue > maxValue)
            return;

        // Vanilla DBCs commonly implement "stat enchants" as ITEM_ENCHANTMENT_TYPE_EQUIP_SPELL (3)
        // that casts a spell (EffectArg) which applies SPELL_AURA_MOD_STAT. Some DBCs use
        // ITEM_ENCHANTMENT_TYPE_STAT (5) directly. Support both.
        int32 statIndex = ItemModToStatIndex(statType);

        for (uint32 i = 0; i < sSpellItemEnchantmentStore.GetNumRows(); ++i)
        {
            SpellItemEnchantmentEntry const* ench = sSpellItemEnchantmentStore.LookupEntry(i);
            if (!ench)
                continue;

            for (int s = 0; s < 3; ++s)
            {
                // Direct stat enchant in DBC
                if (ench->type[s] == ITEM_ENCHANTMENT_TYPE_STAT && ench->spellid[s] == statType)
                {
                    int32 value = int32(ench->amount[s]);
                    if (value >= minValue && value <= maxValue)
                        out.push_back({ench->ID, value});
                    continue;
                }

                // Equip-spell enchant in DBC
                if (ench->type[s] == ITEM_ENCHANTMENT_TYPE_EQUIP_SPELL && statIndex >= 0)
                {
                    uint32 spellId = ench->spellid[s];
                    if (!spellId)
                        continue;

                    SpellEntry const* spellInfo = sSpellTemplate.LookupEntry<SpellEntry>(spellId);
                    if (!spellInfo)
                        continue;

                    for (uint8 eff = 0; eff < MAX_EFFECT_INDEX; ++eff)
                    {
                        if (spellInfo->EffectApplyAuraName[eff] != SPELL_AURA_MOD_STAT)
                            continue;
                        if (spellInfo->EffectMiscValue[eff] != statIndex)
                            continue;

                        int32 value = spellInfo->CalculateSimpleValue(SpellEffectIndex(eff));
                        if (value >= minValue && value <= maxValue)
                            out.push_back({ench->ID, value});
                        break;
                    }
                }
            }
        }
    }

    void ClearSlamrockEnchants(Player* player, Item* targetItem)
    {
        if (!player || !targetItem)
            return;

        // Remove effects first (if equipped), then clear.
        if (targetItem->GetSlot() < EQUIPMENT_SLOT_END)
        {
            for (EnchantmentSlot slot : SLAMROCK_ALL_PROP_SLOTS)
                player->ApplyEnchantment(targetItem, slot, false);

            // Some enchant display types (notably ITEM_ENCHANTMENT_TYPE_TOTEM / Rockbiter) rely on weapon damage refresh
            // for the change to be immediately reflected.
            if (targetItem->GetSlot() == EQUIPMENT_SLOT_MAINHAND)
                player->UpdateDamagePhysical(BASE_ATTACK);
            else if (targetItem->GetSlot() == EQUIPMENT_SLOT_OFFHAND)
                player->UpdateDamagePhysical(OFF_ATTACK);
            else if (targetItem->GetSlot() == EQUIPMENT_SLOT_RANGED)
                player->UpdateDamagePhysical(RANGED_ATTACK);
        }

        for (EnchantmentSlot slot : SLAMROCK_ALL_PROP_SLOTS)
            targetItem->ClearEnchantment(slot);

    if (targetItem->GetEnchantmentId(PERM_ENCHANTMENT_SLOT) == SLAMROCK_MARKER_ENCHANT_ID)
        targetItem->ClearEnchantment(PERM_ENCHANTMENT_SLOT);

    if (targetItem->GetSlot() < EQUIPMENT_SLOT_END)
        player->SetVisibleItemSlot(targetItem->GetSlot(), targetItem);
    }
}

bool ItemUse_item_slamrock(Player* pPlayer, Item* pItem, SpellCastTargets const& targets)
{
    if (!pPlayer || !pItem)
        return false;

    // Script is bound by ScriptName in DB; no entry check needed.

    sLog.outBasic("SLAMROCK: ItemUse fired by player=%s guid=%u item_guid=%u",
        pPlayer->GetName(), pPlayer->GetGUIDLow(), pItem->GetGUIDLow());

    SpellEntry const* targetingSpell = sSpellTemplate.LookupEntry<SpellEntry>(SLAMROCK_TARGETING_SPELL);
    if (!targetingSpell)
        sLog.outError("SLAMROCK: targeting spell %u not found", SLAMROCK_TARGETING_SPELL);

    Item* targetItem = targets.getItemTarget();
    // POC diagnostics: confirm script is invoked and whether an item target was received.
    if (!targetItem)
        pPlayer->GetSession()->SendNotification("Slamrock: script invoked, but no item target received.");
    else
        pPlayer->GetSession()->SendNotification("Slamrock: targeting item entry %u (guid %u).", targetItem->GetEntry(), targetItem->GetGUIDLow());

    if (!targetItem || !IsReasonableGearTarget(targetItem))
    {
        sLog.outBasic("SLAMROCK: rejected target (missing=%s entry=%u inv_type=%u slot=%u)",
            targetItem ? "false" : "true",
            targetItem ? targetItem->GetEntry() : 0,
            targetItem && targetItem->GetProto() ? targetItem->GetProto()->InventoryType : 0,
            targetItem ? targetItem->GetSlot() : 0);
        // Tell client we rejected the target (also prevents the used item staying "stuck" grey)
        pPlayer->SendEquipError(EQUIP_ERR_NONE, pItem, nullptr);
        if (targetingSpell)
            Spell::SendCastResult(pPlayer, targetingSpell, SPELL_FAILED_BAD_TARGETS);
        return true;
    }

    // Must be owned by the player (no trade-slot enchanting for POC)
    if (targetItem->GetOwner() != pPlayer)
    {
        sLog.outBasic("SLAMROCK: rejected target not owned by player (target_owner=%s)",
            targetItem->GetOwner() ? targetItem->GetOwner()->GetName() : "<null>");
        pPlayer->SendEquipError(EQUIP_ERR_NONE, pItem, nullptr);
        if (targetingSpell)
            Spell::SendCastResult(pPlayer, targetingSpell, SPELL_FAILED_BAD_TARGETS);
        return true;
    }

    // Reject items with random suffix/properties, because they use PROP enchant slots.
    if (targetItem->GetItemRandomPropertyId() != 0)
    {
        pPlayer->GetSession()->SendNotification("Slamrock: cannot empower items with random suffix/properties.");
        pPlayer->SendEquipError(EQUIP_ERR_NONE, pItem, nullptr);
        if (targetingSpell)
            Spell::SendCastResult(pPlayer, targetingSpell, SPELL_FAILED_BAD_TARGETS);
        return true;
    }

    // 2% chance: upgrade the target item into another item of the same class, +0..+5 ilvl.
    if (urand(1, 100) <= SLAMROCK_UPGRADE_CHANCE_PCT)
    {
        ItemPrototype const* targetProto = targetItem->GetProto();
        uint32 sameClassTotal = 0;
        uint32 eligibleCount = 0;
        uint32 newEntry = PickUpgradeEntrySameClass(targetProto, SLAMROCK_UPGRADE_MAX_ILVL_DELTA, &sameClassTotal, &eligibleCount);

        sLog.outBasic("SLAMROCK: upgrade roll for target entry=%u ilvl=%u class=%u bag=%u slot=%u sameClassTotal=%u eligible=%u picked=%u",
            targetProto ? targetProto->ItemId : 0,
            targetProto ? targetProto->ItemLevel : 0,
            targetProto ? targetProto->Class : 0,
            targetItem->GetBagSlot(),
            targetItem->GetSlot(),
            sameClassTotal,
            eligibleCount,
            newEntry);

        if (newEntry)
        {
            ItemPrototype const* newProto = ObjectMgr::GetItemPrototype(newEntry);
            InventoryResult failReason = EQUIP_ERR_OK;
            bool wasEquipped = false;
            if (TryDowngradeItemInPlace(pPlayer, targetItem, newEntry, &failReason, &wasEquipped))
            {
                pPlayer->DestroyItemCount(pItem->GetEntry(), 1, true);
                pPlayer->GetSession()->SendNotification("Slamrock: your item upgrades into %s (entry %u).",
                    newProto && newProto->Name1 ? newProto->Name1 : "a different item",
                    newEntry);
                return true;
            }

            sLog.outBasic("SLAMROCK: upgrade replace failed for newEntry=%u (wasEquipped=%s failReason=%u)",
                newEntry, wasEquipped ? "true" : "false", uint32(failReason));
        }
        // If no upgrade target exists or replacement fails, fall through to downgrade/normal behavior.
    }

    // 25% chance: downgrade the target item into another (lower item level) item of the same type.
    if (urand(1, 100) <= SLAMROCK_DOWNGRADE_CHANCE_PCT)
    {
        ItemPrototype const* targetProto = targetItem->GetProto();
        uint32 sameTypeTotal = 0;
        uint32 lowerCount = 0;
        uint32 newEntry = PickDowngradeEntry(targetProto, &sameTypeTotal, &lowerCount);

        sLog.outBasic("SLAMROCK: downgrade roll for target entry=%u ilvl=%u class=%u sub=%u inv=%u bag=%u slot=%u sameTypeTotal=%u lowerCandidates=%u picked=%u",
            targetProto ? targetProto->ItemId : 0,
            targetProto ? targetProto->ItemLevel : 0,
            targetProto ? targetProto->Class : 0,
            targetProto ? targetProto->SubClass : 0,
            targetProto ? targetProto->InventoryType : 0,
            targetItem->GetBagSlot(),
            targetItem->GetSlot(),
            sameTypeTotal,
            lowerCount,
            newEntry);

        if (newEntry)
        {
            ItemPrototype const* newProto = ObjectMgr::GetItemPrototype(newEntry);
            InventoryResult failReason = EQUIP_ERR_OK;
            bool wasEquipped = false;
            if (TryDowngradeItemInPlace(pPlayer, targetItem, newEntry, &failReason, &wasEquipped))
            {
                pPlayer->DestroyItemCount(pItem->GetEntry(), 1, true);
                pPlayer->GetSession()->SendNotification("Slamrock: your item transforms into %s (entry %u).",
                    newProto && newProto->Name1 ? newProto->Name1 : "a different item",
                    newEntry);
                return true;
            }

            sLog.outBasic("SLAMROCK: downgrade replace failed for newEntry=%u (wasEquipped=%s failReason=%u)",
                newEntry, wasEquipped ? "true" : "false", uint32(failReason));
        }
        // If no downgrade target exists or replacement fails, fall through to normal empower behavior.
    }

    // Roll 1..3 modifiers from DB whitelist (maintained via DB updates; editable in HeidiSQL).
    SlamrockWhitelistCache& cache = GetWhitelistCache();
    if (!cache.loaded && !cache.attemptedLoad)
    {
        cache.attemptedLoad = true;
        LoadWhitelistFromDB();
    }

    ItemPrototype const* targetProto = targetItem->GetProto();
    uint32 itemLevel = targetProto ? targetProto->ItemLevel : 0;
    bool isWeapon = IsWeaponTarget(targetItem);
    std::vector<SlamrockWhitelistRow> const& pool = isWeapon ? cache.weapon : cache.armor;
    if (pool.empty())
    {
        sLog.outError("SLAMROCK: whitelist empty/missing for %s (table=%s).",
            isWeapon ? "weapon" : "armor",
            SLAMROCK_WHITELIST_TABLE);
        pPlayer->GetSession()->SendNotification("Slamrock: whitelist table is empty/missing (cannot roll).");
        pPlayer->SendEquipError(EQUIP_ERR_NONE, pItem, nullptr);
        if (targetingSpell)
            Spell::SendCastResult(pPlayer, targetingSpell, SPELL_FAILED_ERROR);
        return true;
    }

    uint32 modifierCount = urand(1, SLAMROCK_MAX_MODIFIERS);
    std::vector<uint32> rolled;
    rolled.reserve(modifierCount);

    // Pick without replacement (avoid duplicates) for better UX.
    // If the candidate pool is tiny, cap the modifier count.
    if (!pool.empty())
    {
        // Only cap if there are no eligible groups at all.
        std::map<std::string, bool> groups;
        for (SlamrockWhitelistRow const& row : pool)
        {
            if (row.weight == 0 || row.minIlvl > itemLevel)
                continue;
            groups[GetEffectiveGroupKey(row)] = true;
        }
        if (groups.empty())
            modifierCount = 0;
    }
    // pool is guaranteed non-empty above

    if (modifierCount == 0)
    {
        sLog.outError("SLAMROCK: no eligible enchants (item=%u ilvl=%u isWeapon=%u)", targetItem->GetEntry(), itemLevel, isWeapon ? 1u : 0u);
        pPlayer->GetSession()->SendNotification("Slamrock: no eligible enchantments found (cannot apply).");
        pPlayer->SendEquipError(EQUIP_ERR_NONE, pItem, nullptr);
        if (targetingSpell)
            Spell::SendCastResult(pPlayer, targetingSpell, SPELL_FAILED_ERROR);
        return true;
    }

    while (rolled.size() < modifierCount)
    {
        uint32 picked = 0;
        std::string groupKey;
        static std::vector<std::string> noExclusions;
        if (!PickWeightedGroupFromWhitelist(pool, itemLevel, noExclusions, groupKey))
            break;
        if (!PickRandomEligibleRankEnchantForGroup(pool, itemLevel, groupKey, picked))
            break;
        rolled.push_back(picked);
    }

    if (rolled.empty())
    {
        sLog.outError("SLAMROCK: failed to roll any enchantments (item=%u ilvl=%u isWeapon=%u)", targetItem->GetEntry(), itemLevel, isWeapon ? 1u : 0u);
        pPlayer->GetSession()->SendNotification("Slamrock: no eligible enchantments found (cannot apply).");
        pPlayer->SendEquipError(EQUIP_ERR_NONE, pItem, nullptr);
        if (targetingSpell)
            Spell::SendCastResult(pPlayer, targetingSpell, SPELL_FAILED_ERROR);
        return true;
    }

    // Clear previous slamrock enchants (safe because we reject RandomPropertyId != 0).
    ClearSlamrockEnchants(pPlayer, targetItem);

    // Apply marker + modifiers.
    // Prefer perm slot for the marker when free (better link/trade visibility).
    // If perm is already occupied, fall back to prop marker slot.
    bool markerInPerm = false;
    if (targetItem->GetEnchantmentId(PERM_ENCHANTMENT_SLOT) == 0)
    {
        targetItem->SetEnchantment(PERM_ENCHANTMENT_SLOT, SLAMROCK_MARKER_ENCHANT_ID, 0, 0, pPlayer->GetObjectGuid());
        markerInPerm = true;
    }
    if (!markerInPerm)
        targetItem->SetEnchantment(SLAMROCK_MARKER_SLOT, SLAMROCK_MARKER_ENCHANT_ID, 0, 0, pPlayer->GetObjectGuid());

    for (uint32 i = 0; i < rolled.size() && i < SLAMROCK_MAX_MODIFIERS; ++i)
        targetItem->SetEnchantment(SLAMROCK_MODIFIER_SLOTS[i], rolled[i], 0, 0, pPlayer->GetObjectGuid());


    targetItem->SetState(ITEM_CHANGED, pPlayer);
    sLog.outBasic("SLAMROCK: set %u enchants on target_item_guid=%u (e0=%u e1=%u e2=%u)",
        uint32(rolled.size()),
        targetItem->GetGUIDLow(),
        rolled.size() > 0 ? rolled[0] : 0,
        rolled.size() > 1 ? rolled[1] : 0,
        rolled.size() > 2 ? rolled[2] : 0);

    if (targetItem->GetSlot() < EQUIPMENT_SLOT_END)
    {
        // Apply marker first, then modifiers
        pPlayer->ApplyEnchantment(targetItem, SLAMROCK_MARKER_SLOT, true);
        for (uint32 i = 0; i < rolled.size() && i < SLAMROCK_MAX_MODIFIERS; ++i)
            pPlayer->ApplyEnchantment(targetItem, SLAMROCK_MODIFIER_SLOTS[i], true);

        // Ensure weapon damage is refreshed immediately for enchant types that affect weapon damage (e.g. Rockbiter/Totem).
        if (targetItem->GetSlot() == EQUIPMENT_SLOT_MAINHAND)
            pPlayer->UpdateDamagePhysical(BASE_ATTACK);
        else if (targetItem->GetSlot() == EQUIPMENT_SLOT_OFFHAND)
            pPlayer->UpdateDamagePhysical(OFF_ATTACK);
        else if (targetItem->GetSlot() == EQUIPMENT_SLOT_RANGED)
            pPlayer->UpdateDamagePhysical(RANGED_ATTACK);

        pPlayer->SetVisibleItemSlot(targetItem->GetSlot(), targetItem);
    }

    // Consume the Slamrock
    pPlayer->DestroyItemCount(pItem->GetEntry(), 1, true);
    sLog.outBasic("SLAMROCK: consumed item for player=%s", pPlayer->GetName());

    pPlayer->GetSession()->SendNotification("Slamrock: empowered with %u modifiers (e0=%u e1=%u e2=%u).",
        uint32(rolled.size()),
        rolled.size() > 0 ? rolled[0] : 0,
        rolled.size() > 1 ? rolled[1] : 0,
        rolled.size() > 2 ? rolled[2] : 0);
    return true; // handled
}

void AddSC_slamrock_items()
{
    Script* pNewScript = new Script;
    pNewScript->Name = "item_slamrock";
    pNewScript->pItemUse = &ItemUse_item_slamrock;
    pNewScript->RegisterSelf();
}


