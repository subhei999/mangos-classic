/*
 * This file is part of the CMaNGOS Project. See AUTHORS file for Copyright information
 *
 * This program is free software; you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation; either version 2 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program; if not, write to the Free Software
 * Foundation, Inc., 59 Temple Place, Suite 330, Boston, MA  02111-1307  USA
 */

#include "Common.h"
#include "Database/DatabaseEnv.h"
#include "World/World.h"
#include "Entities/Player.h"
#include "Entities/Item.h"
#include "Entities/ItemPrototype.h"
#include "Server/Opcodes.h"
#include "Server/SQLStorages.h"
#include "Chat/Chat.h"
#include "Globals/ObjectAccessor.h"
#include "Globals/ObjectMgr.h"
#include "Maps/MapManager.h"
#include "Tools/Language.h"
#include "Accounts/AccountMgr.h"
#include "SystemConfig.h"
#include "revision.h"
#include "Util/Util.h"
#include "Globals/SharedDefines.h"
#include "Server/DBCStores.h"
#include <algorithm>
#include <cctype>
#include <map>
#include <set>
#include <vector>

namespace
{
enum class DraftPhase : uint8
{
    None = 0,
    Dungeon = 1,
    Spec = 2,
    Gear = 3,
    Done = 4
};

struct DraftOption
{
    std::string id;
    std::string name;
    std::string description;
    int32 randomPropertyId = 0;
    std::string itemString;
};

struct DraftSession
{
    std::string dungeonId;
    uint32 mapId = 0;
    uint32 level = 1;
    uint8 draftedClass = 0;
    DraftPhase phase = DraftPhase::None;
    std::vector<DraftOption> dungeonPool;
    std::vector<DraftOption> dungeonOptions;
    uint32 dungeonPage = 0;
    std::vector<DraftOption> specOptions;
    std::vector<uint32> gearSlots;
    uint32 gearSlotIndex = 0;
    std::vector<DraftOption> gearOptions;
    std::string selectedSpec;
    std::vector<uint32> selectedGearItemIds;
    std::vector<int32> selectedGearRandomPropertyIds;
};

std::map<uint32, DraftSession> s_draftSessions;

static const uint32 DRAFT_SLOT_MAINHAND = 1001;
static const uint32 DRAFT_SLOT_OFFHAND = 1002;
static const uint32 DRAFT_SLOT_RANGED = 1003;
static const uint32 DRAFT_DUNGEON_PAGE_SIZE = 3;

std::string ToLowerCopy(std::string value)
{
    std::transform(value.begin(), value.end(), value.begin(), [](unsigned char c) { return std::tolower(c); });
    return value;
}

std::vector<DraftOption> PickOptions(std::vector<DraftOption> pool, uint32 count)
{
    std::vector<DraftOption> result;
    result.reserve(count);

    while (!pool.empty() && result.size() < count)
    {
        uint32 idx = urand(0, uint32(pool.size() - 1));
        result.push_back(pool[idx]);
        pool.erase(pool.begin() + idx);
    }

    return result;
}

struct DungeonTemplate
{
    char const* id;
    char const* name;
    uint32 mapId;
    uint32 levelMin;
    uint32 levelMax;
};

std::vector<DungeonTemplate> GetDungeonTemplates()
{
    return {
        { "rfc", "Ragefire Chasm", 389, 13, 18 },
        { "wc", "Wailing Caverns", 43, 17, 24 },
        { "dm", "The Deadmines", 36, 17, 24 },
        { "sfk", "Shadowfang Keep", 33, 22, 30 },
        { "bfd", "Blackfathom Deeps", 48, 24, 32 },
        { "stocks", "The Stockade", 34, 22, 30 },
        { "gnomer", "Gnomeregan", 90, 29, 38 },
        { "rfk", "Razorfen Kraul", 47, 29, 38 },
        { "sm", "Scarlet Monastery", 189, 34, 44 },
        { "ulda", "Uldaman", 70, 38, 48 },
        { "zf", "Zul'Farrak", 209, 44, 54 },
        { "mara", "Maraudon", 349, 45, 55 },
        { "st", "Sunken Temple", 109, 50, 58 },
        { "brd", "Blackrock Depths", 230, 52, 60 },
        { "lbrs", "Lower Blackrock Spire", 229, 55, 60 },
        { "dmn", "Dire Maul", 429, 55, 60 },
        { "scholo", "Scholomance", 289, 57, 60 },
        { "strat", "Stratholme", 329, 58, 60 }
    };
}

bool ResolveDungeonTemplate(std::string const& id, DungeonTemplate& out)
{
    std::vector<DungeonTemplate> pool = GetDungeonTemplates();
    for (DungeonTemplate const& t : pool)
    {
        if (id == t.id)
        {
            out = t;
            return true;
        }
    }

    return false;
}

std::vector<DraftOption> BuildDungeonPool()
{
    std::vector<DraftOption> options;
    std::vector<DungeonTemplate> pool = GetDungeonTemplates();
    options.reserve(pool.size());
    for (DungeonTemplate const& d : pool)
    {
        DraftOption opt;
        opt.id = d.id;
        opt.name = d.name;
        opt.description = "Level " + std::to_string(d.levelMin) + "-" + std::to_string(d.levelMax);
        options.push_back(opt);
    }

    return options;
}

std::vector<DraftOption> BuildSpecPoolForDungeon(std::string const& dungeonId)
{
    // MVP generic pool reused for all selected dungeons.
    // We filter by race/class compatibility after this.
    (void)dungeonId;
    return {
        {"warrior_arms", "Warrior Arms", "Weapon pressure and durable front line"},
        {"warrior_fury", "Warrior Fury", "Faster rage spend with sustained melee damage"},
        {"paladin_holy", "Paladin Holy", "Stable single-target healing and support"},
        {"paladin_retribution", "Paladin Retribution", "Melee pressure with utility buffs"},
        {"shaman_restoration", "Shaman Restoration", "Reactive healing and totem utility"},
        {"shaman_enhancement", "Shaman Enhancement", "Melee burst and totem support"},
        {"rogue_combat", "Rogue Combat", "Stable single-target pressure and control"},
        {"mage_frost", "Mage Frost", "Kiting, slows, and safety tools"},
        {"mage_fire", "Mage Fire", "Burst windows and strong AoE pressure"},
        {"priest_holy", "Priest Holy", "Reliable sustain and emergency saves"},
        {"priest_shadow", "Priest Shadow", "DoT pressure with utility"},
        {"druid_feral", "Druid Feral", "Flexible melee with mobility"},
        {"druid_restoration", "Druid Restoration", "Efficient healing-over-time support"},
        {"hunter_marksmanship", "Hunter Marks", "Ranged control and steady damage"},
        {"warlock_affliction", "Warlock Affliction", "Drain and attrition gameplay"}
    };
}

uint8 GetClassFromSpecId(std::string const& specId)
{
    if (specId.find("warrior_") == 0) return CLASS_WARRIOR;
    if (specId.find("paladin_") == 0) return CLASS_PALADIN;
    if (specId.find("hunter_") == 0) return CLASS_HUNTER;
    if (specId.find("rogue_") == 0) return CLASS_ROGUE;
    if (specId.find("priest_") == 0) return CLASS_PRIEST;
    if (specId.find("shaman_") == 0) return CLASS_SHAMAN;
    if (specId.find("mage_") == 0) return CLASS_MAGE;
    if (specId.find("warlock_") == 0) return CLASS_WARLOCK;
    if (specId.find("druid_") == 0) return CLASS_DRUID;
    return 0;
}

uint32 GetClassMask(uint8 classId)
{
    return classId > 0 ? (1u << (classId - 1)) : 0u;
}

uint32 GetMaxArmorSubclassForClass(uint8 classId, uint32 level)
{
    switch (classId)
    {
        case CLASS_MAGE:
        case CLASS_PRIEST:
        case CLASS_WARLOCK:
            return ITEM_SUBCLASS_ARMOR_CLOTH;
        case CLASS_DRUID:
        case CLASS_ROGUE:
            return ITEM_SUBCLASS_ARMOR_LEATHER;
        case CLASS_HUNTER:
        case CLASS_SHAMAN:
            return level >= 40 ? ITEM_SUBCLASS_ARMOR_MAIL : ITEM_SUBCLASS_ARMOR_LEATHER;
        case CLASS_WARRIOR:
        case CLASS_PALADIN:
            return level >= 40 ? ITEM_SUBCLASS_ARMOR_PLATE : ITEM_SUBCLASS_ARMOR_MAIL;
        default:
            return ITEM_SUBCLASS_ARMOR_CLOTH;
    }
}

char const* InventoryTypeToLabel(uint32 invType)
{
    switch (invType)
    {
        case DRAFT_SLOT_MAINHAND: return "main hand";
        case DRAFT_SLOT_OFFHAND: return "off hand";
        case DRAFT_SLOT_RANGED: return "ranged";
        case INVTYPE_HEAD: return "head";
        case INVTYPE_NECK: return "neck";
        case INVTYPE_SHOULDERS: return "shoulders";
        case INVTYPE_CLOAK: return "back";
        case INVTYPE_CHEST: return "chest";
        case INVTYPE_WRISTS: return "wrist";
        case INVTYPE_HANDS: return "hands";
        case INVTYPE_WAIST: return "belt";
        case INVTYPE_LEGS: return "legs";
        case INVTYPE_FEET: return "feet";
        case INVTYPE_FINGER: return "ring";
        case INVTYPE_TRINKET: return "trinket";
        default: return "unknown";
    }
}

std::vector<uint32> BuildGearSlotsForDraft()
{
    return {
        INVTYPE_HEAD,
        INVTYPE_NECK,
        INVTYPE_SHOULDERS,
        INVTYPE_CLOAK,
        INVTYPE_CHEST,
        INVTYPE_WRISTS,
        INVTYPE_HANDS,
        INVTYPE_WAIST,
        INVTYPE_LEGS,
        INVTYPE_FEET,
        DRAFT_SLOT_MAINHAND,
        DRAFT_SLOT_OFFHAND,
        DRAFT_SLOT_RANGED,
        INVTYPE_FINGER,
        INVTYPE_FINGER,
        INVTYPE_TRINKET,
        INVTYPE_TRINKET
    };
}

bool IsItemValidForDraft(ItemPrototype const* proto, uint8 draftedClass, uint32 level, uint32 requestedInventoryType)
{
    if (!proto || !proto->Name1 || !proto->Name1[0])
        return false;

    if (proto->RequiredLevel > level)
        return false;

    // Keep MVP pool close to dungeon level range.
    if (proto->ItemLevel > level + 8 || proto->ItemLevel + 10 < level)
        return false;

    // Ignore very low/very high quality edge cases in MVP.
    if (proto->Quality < 1 || proto->Quality > 3)
        return false;

    uint32 classMask = GetClassMask(draftedClass);
    if (classMask == 0 || (proto->AllowableClass & classMask) == 0)
        return false;

    const bool isArmorSlot =
        requestedInventoryType == INVTYPE_HEAD ||
        requestedInventoryType == INVTYPE_NECK ||
        requestedInventoryType == INVTYPE_SHOULDERS ||
        requestedInventoryType == INVTYPE_CLOAK ||
        requestedInventoryType == INVTYPE_CHEST ||
        requestedInventoryType == INVTYPE_WRISTS ||
        requestedInventoryType == INVTYPE_HANDS ||
        requestedInventoryType == INVTYPE_WAIST ||
        requestedInventoryType == INVTYPE_LEGS ||
        requestedInventoryType == INVTYPE_FEET ||
        requestedInventoryType == INVTYPE_FINGER ||
        requestedInventoryType == INVTYPE_TRINKET;

    const bool isWeaponSlot =
        requestedInventoryType == DRAFT_SLOT_MAINHAND ||
        requestedInventoryType == DRAFT_SLOT_OFFHAND ||
        requestedInventoryType == DRAFT_SLOT_RANGED;

    if (isArmorSlot)
    {
        if (proto->Class != ITEM_CLASS_ARMOR)
            return false;

        if (requestedInventoryType == INVTYPE_CHEST)
        {
            if (proto->InventoryType != INVTYPE_CHEST && proto->InventoryType != INVTYPE_ROBE)
                return false;
        }
        else if (proto->InventoryType != requestedInventoryType)
        {
            return false;
        }
    }
    else if (isWeaponSlot)
    {
        if (requestedInventoryType == DRAFT_SLOT_MAINHAND)
        {
            if (proto->Class != ITEM_CLASS_WEAPON)
                return false;

            if (proto->InventoryType != INVTYPE_WEAPON &&
                proto->InventoryType != INVTYPE_WEAPONMAINHAND &&
                proto->InventoryType != INVTYPE_2HWEAPON)
                return false;
        }
        else if (requestedInventoryType == DRAFT_SLOT_OFFHAND)
        {
            const bool isOffhandWeapon =
                proto->Class == ITEM_CLASS_WEAPON &&
                proto->InventoryType == INVTYPE_WEAPONOFFHAND;
            const bool isShield =
                proto->Class == ITEM_CLASS_ARMOR &&
                proto->InventoryType == INVTYPE_SHIELD;
            const bool isHoldable =
                proto->Class == ITEM_CLASS_ARMOR &&
                proto->InventoryType == INVTYPE_HOLDABLE;

            if (!isOffhandWeapon && !isShield && !isHoldable)
                return false;
        }
        else if (requestedInventoryType == DRAFT_SLOT_RANGED)
        {
            const bool isRangedWeapon =
                proto->Class == ITEM_CLASS_WEAPON &&
                (proto->InventoryType == INVTYPE_RANGED || proto->InventoryType == INVTYPE_RANGEDRIGHT || proto->InventoryType == INVTYPE_THROWN);
            if (!isRangedWeapon)
                return false;
        }
    }
    else
    {
        return false;
    }

    if (isArmorSlot && requestedInventoryType != INVTYPE_NECK && requestedInventoryType != INVTYPE_CLOAK &&
        requestedInventoryType != INVTYPE_FINGER && requestedInventoryType != INVTYPE_TRINKET)
    {
        if (proto->SubClass == ITEM_SUBCLASS_ARMOR_MISC)
            return false;

        if (proto->SubClass > GetMaxArmorSubclassForClass(draftedClass, level))
            return false;
    }

    return true;
}

std::string BuildDraftItemString(uint32 itemId, int32 randomPropertyId)
{
    // Random property sits at field 6 for vanilla item links after item id:
    // item:id:perm:g1:g2:g3:field5:randomProperty:field7:field8
    return "item:" + std::to_string(itemId) + ":0:0:0:0:0:" + std::to_string(randomPropertyId) + ":0:0";
}

std::vector<DraftOption> BuildGearOptionsForSlot(uint8 draftedClass, uint32 level, uint32 invType)
{
    std::vector<DraftOption> candidates;
    candidates.reserve(128);

    for (uint32 id = 1; id < sItemStorage.GetMaxEntry(); ++id)
    {
        ItemPrototype const* proto = sItemStorage.LookupEntry<ItemPrototype>(id);
        if (!IsItemValidForDraft(proto, draftedClass, level, invType))
            continue;

        int32 rolledRandomPropertyId = 0;
        std::string displayName = proto->Name1;
        if (proto->RandomProperty)
        {
            rolledRandomPropertyId = Item::GenerateItemRandomPropertyId(proto->ItemId);
            if (!rolledRandomPropertyId)
                continue;

            uint32 propertyLookupId = rolledRandomPropertyId > 0 ? uint32(rolledRandomPropertyId) : uint32(-rolledRandomPropertyId);
            if (ItemRandomPropertiesEntry const* property = sItemRandomPropertiesStore.LookupEntry(propertyLookupId))
            {
                if (property->nameSuffix[0] && property->nameSuffix[0][0])
                    displayName += std::string(" ") + property->nameSuffix[0];
            }
        }

        DraftOption option;
        option.id = std::to_string(proto->ItemId);
        option.name = displayName;
        option.description = "slot " + std::string(InventoryTypeToLabel(invType)) +
            ", req " + std::to_string(proto->RequiredLevel) +
            ", ilvl " + std::to_string(proto->ItemLevel);
        option.randomPropertyId = rolledRandomPropertyId;
        option.itemString = BuildDraftItemString(proto->ItemId, rolledRandomPropertyId);
        candidates.push_back(option);
    }

    return PickOptions(candidates, 3);
}

void SendDraftOptions(ChatHandler* handler, char const* kind, std::vector<DraftOption> const& options)
{
    for (uint32 i = 0; i < options.size(); ++i)
    {
        DraftOption const& opt = options[i];
        if (strcmp(kind, "GEAR") == 0)
            handler->PSendSysMessage("DRAFT:%s|%u|%s|%s|%s|%s", kind, i + 1, opt.id.c_str(), opt.name.c_str(), opt.description.c_str(), opt.itemString.c_str());
        else
            handler->PSendSysMessage("DRAFT:%s|%u|%s|%s|%s", kind, i + 1, opt.id.c_str(), opt.name.c_str(), opt.description.c_str());
    }
}

void SendDungeonPage(ChatHandler* handler, DraftSession& session)
{
    if (!handler)
        return;

    if (session.dungeonPool.empty())
    {
        session.dungeonOptions.clear();
        handler->SendSysMessage("DRAFT:ERROR|pool_empty|Dungeon pool is not available");
        return;
    }

    uint32 totalPages = (uint32(session.dungeonPool.size()) + DRAFT_DUNGEON_PAGE_SIZE - 1) / DRAFT_DUNGEON_PAGE_SIZE;
    if (session.dungeonPage >= totalPages)
        session.dungeonPage = totalPages ? (totalPages - 1) : 0;

    uint32 start = session.dungeonPage * DRAFT_DUNGEON_PAGE_SIZE;
    uint32 end = std::min<uint32>(start + DRAFT_DUNGEON_PAGE_SIZE, uint32(session.dungeonPool.size()));

    session.dungeonOptions.clear();
    for (uint32 i = start; i < end; ++i)
        session.dungeonOptions.push_back(session.dungeonPool[i]);

    handler->PSendSysMessage("DRAFT:DUNGEON_PAGE|%u|%u", session.dungeonPage + 1, totalPages);
    SendDraftOptions(handler, "DUNGEON", session.dungeonOptions);
}

bool GiveDraftItemInBestSlots(Player* player, uint32 itemId, int32 randomPropertyId)
{
    if (!player || !itemId)
        return false;

    if (Item* equipItem = Item::CreateItem(itemId, 1, player, randomPropertyId))
    {
        uint16 equipDest;
        uint8 equipResult = player->CanEquipItem(NULL_SLOT, equipDest, equipItem, false);
        if (equipResult == EQUIP_ERR_OK)
        {
            player->EquipItem(equipDest, equipItem, true);
            player->AutoUnequipOffhandIfNeed();
            return true;
        }

        delete equipItem;
    }

    ItemPosCountVec storeDest;
    uint8 storeResult = player->CanStoreNewItem(INVENTORY_SLOT_BAG_0, NULL_SLOT, storeDest, itemId, 1);
    if (storeResult == EQUIP_ERR_OK)
    {
        return player->StoreNewItem(storeDest, itemId, true, randomPropertyId) != nullptr;
    }

    return false;
}

bool OpenCurrentGearRound(ChatHandler* handler, DraftSession& session)
{
    if (session.gearSlotIndex >= session.gearSlots.size())
    {
        session.phase = DraftPhase::Done;
        handler->SendSysMessage("DRAFT:READY|All picks complete. Finalizing draft...");
        return true;
    }

    uint32 slotInvType = session.gearSlots[session.gearSlotIndex];
    session.gearOptions = BuildGearOptionsForSlot(session.draftedClass, session.level, slotInvType);
    if (session.gearOptions.size() < 3)
    {
        handler->PSendSysMessage("DRAFT:ERROR|slot_pool_empty|No valid items found for slot %s", InventoryTypeToLabel(slotInvType));
        return false;
    }

    handler->PSendSysMessage("DRAFT:GEAR_SLOT|%s|%u|%u",
        InventoryTypeToLabel(slotInvType), session.gearSlotIndex + 1, session.gearSlots.size());
    SendDraftOptions(handler, "GEAR", session.gearOptions);
    return true;
}

bool ResolveDraftRaceForClass(Player const* owner, uint8 classId, uint8& outRace)
{
    if (!owner || !classId)
        return false;

    // Keep owner race when possible.
    uint8 ownerRace = owner->getRace();
    if (sObjectMgr.GetPlayerInfo(ownerRace, classId))
    {
        outRace = ownerRace;
        return true;
    }

    Team ownerTeam = Player::TeamForRace(ownerRace);
    // Prefer same-faction races first.
    for (uint8 race = RACE_HUMAN; race < MAX_RACES; ++race)
    {
        if (Player::TeamForRace(race) != ownerTeam)
            continue;

        if (sObjectMgr.GetPlayerInfo(race, classId))
        {
            outRace = race;
            return true;
        }
    }

    // Fallback: any race that can be this class.
    for (uint8 race = RACE_HUMAN; race < MAX_RACES; ++race)
    {
        if (sObjectMgr.GetPlayerInfo(race, classId))
        {
            outRace = race;
            return true;
        }
    }

    return false;
}

std::string MakeDraftCharacterName()
{
    static char const alphabet[] = "abcdefghijklmnopqrstuvwxyz";
    std::string name = "Draft";
    for (uint32 i = 0; i < 4; ++i)
        name.push_back(alphabet[urand(0, 25)]);

    return name;
}

uint32 ApplyFallbackSkillsFromWorldTable(Player* player)
{
    if (!player)
        return 0;

    const uint32 raceMask = (1u << (player->getRace() - 1));
    const uint32 classMask = (1u << (player->getClass() - 1));
    auto queryResult = WorldDatabase.PQuery(
        "SELECT skill, step FROM playercreateinfo_skills "
        "WHERE (raceMask = 0 OR (raceMask & '%u')) "
        "AND (classMask = 0 OR (classMask & '%u'))",
        raceMask, classMask);

    if (!queryResult)
        return 0;

    uint32 added = 0;
    do
    {
        Field* fields = queryResult->Fetch();
        const uint16 skillId = fields[0].GetUInt16();
        uint16 step = fields[1].GetUInt16();
        if (!skillId || player->HasSkill(skillId))
            continue;

        if (!sSkillLineStore.LookupEntry(skillId))
            continue;

        uint16 value = 1;
        uint16 max = player->GetSkillMaxForLevel();
        if (step > MAX_SKILL_STEP)
            step = 0;

        if (!step)
            step = 1;

        if (SkillRaceClassInfoEntry const* info = player->GetSkillInfo(skillId))
        {
            if (info->skillTierId)
            {
                if (SkillTiersEntry const* tiers = sSkillTiersStore.LookupEntry(info->skillTierId))
                {
                    const uint16 idx = uint16(step - 1);
                    if (idx < MAX_SKILL_STEP && tiers->maxSkillValue[idx])
                    {
                        max = uint16(tiers->maxSkillValue[idx]);
                    }
                }
            }

            // Fallback path is intentionally generous: ensure visible trained skills.
            value = max;
        }

        player->SetSkill(skillId, value, max, step);
        ++added;
    }
    while (queryResult->NextRow());

    return added;
}

uint32 PersistSkillsToCharacterDbFromWorldTable(Player* player)
{
    if (!player)
        return 0;

    const uint32 guid = player->GetGUIDLow();
    const uint32 raceMask = (1u << (player->getRace() - 1));
    const uint32 classMask = (1u << (player->getClass() - 1));

    std::set<uint16> skillIds;

    // Hard baseline so the character always has visible trained skills.
    skillIds.insert(SKILL_DEFENSE);
    skillIds.insert(SKILL_UNARMED);
    skillIds.insert(player->GetTeam() == ALLIANCE ? SKILL_LANG_COMMON : SKILL_LANG_ORCISH);
    skillIds.insert(SKILL_CLOTH);

    switch (player->getClass())
    {
        case CLASS_WARRIOR:
            skillIds.insert(SKILL_MAIL);
            skillIds.insert(SKILL_AXES);
            skillIds.insert(SKILL_2H_AXES);
            skillIds.insert(SKILL_SWORDS);
            skillIds.insert(SKILL_2H_SWORDS);
            skillIds.insert(SKILL_MACES);
            skillIds.insert(SKILL_2H_MACES);
            skillIds.insert(SKILL_POLEARMS);
            skillIds.insert(SKILL_SHIELD);
            skillIds.insert(SKILL_BOWS);
            skillIds.insert(SKILL_GUNS);
            skillIds.insert(SKILL_THROWN);
            break;
        case CLASS_PALADIN:
            skillIds.insert(SKILL_MAIL);
            skillIds.insert(SKILL_LEATHER);
            skillIds.insert(SKILL_PLATE_MAIL);
            skillIds.insert(SKILL_SWORDS);
            skillIds.insert(SKILL_2H_SWORDS);
            skillIds.insert(SKILL_MACES);
            skillIds.insert(SKILL_2H_MACES);
            skillIds.insert(SKILL_SHIELD);
            break;
        case CLASS_HUNTER:
            skillIds.insert(SKILL_LEATHER);
            skillIds.insert(SKILL_MAIL);
            skillIds.insert(SKILL_BOWS);
            skillIds.insert(SKILL_GUNS);
            skillIds.insert(SKILL_CROSSBOWS);
            skillIds.insert(SKILL_AXES);
            skillIds.insert(SKILL_2H_AXES);
            skillIds.insert(SKILL_SWORDS);
            skillIds.insert(SKILL_2H_SWORDS);
            skillIds.insert(SKILL_STAVES);
            skillIds.insert(SKILL_POLEARMS);
            skillIds.insert(SKILL_DAGGERS);
            break;
        case CLASS_ROGUE:
            skillIds.insert(SKILL_LEATHER);
            skillIds.insert(SKILL_DAGGERS);
            skillIds.insert(SKILL_SWORDS);
            skillIds.insert(SKILL_MACES);
            skillIds.insert(SKILL_BOWS);
            skillIds.insert(SKILL_GUNS);
            skillIds.insert(SKILL_CROSSBOWS);
            skillIds.insert(SKILL_THROWN);
            break;
        case CLASS_PRIEST:
            skillIds.insert(SKILL_MACES);
            skillIds.insert(SKILL_STAVES);
            skillIds.insert(SKILL_DAGGERS);
            skillIds.insert(SKILL_WANDS);
            break;
        case CLASS_SHAMAN:
            skillIds.insert(SKILL_LEATHER);
            skillIds.insert(SKILL_MAIL);
            skillIds.insert(SKILL_AXES);
            skillIds.insert(SKILL_2H_AXES);
            skillIds.insert(SKILL_MACES);
            skillIds.insert(SKILL_2H_MACES);
            skillIds.insert(SKILL_STAVES);
            skillIds.insert(SKILL_DAGGERS);
            skillIds.insert(SKILL_SHIELD);
            break;
        case CLASS_MAGE:
            skillIds.insert(SKILL_DAGGERS);
            skillIds.insert(SKILL_STAVES);
            skillIds.insert(SKILL_SWORDS);
            skillIds.insert(SKILL_WANDS);
            break;
        case CLASS_WARLOCK:
            skillIds.insert(SKILL_DAGGERS);
            skillIds.insert(SKILL_STAVES);
            skillIds.insert(SKILL_SWORDS);
            skillIds.insert(SKILL_WANDS);
            break;
        case CLASS_DRUID:
            skillIds.insert(SKILL_LEATHER);
            skillIds.insert(SKILL_MACES);
            skillIds.insert(SKILL_2H_MACES);
            skillIds.insert(SKILL_STAVES);
            skillIds.insert(SKILL_DAGGERS);
            break;
        default:
            break;
    }

    auto queryResult = WorldDatabase.PQuery(
        "SELECT skill, step FROM playercreateinfo_skills "
        "WHERE (raceMask = 0 OR (raceMask & '%u')) "
        "AND (classMask = 0 OR (classMask & '%u'))",
        raceMask, classMask);

    CharacterDatabase.PExecute("DELETE FROM character_skills WHERE guid = '%u'", guid);

    if (queryResult)
    {
        do
        {
            Field* fields = queryResult->Fetch();
            const uint16 skillId = fields[0].GetUInt16();
            if (skillId)
                skillIds.insert(skillId);
        }
        while (queryResult->NextRow());
    }

    uint32 inserted = 0;
    for (uint16 skillId : skillIds)
    {
        if (!sSkillLineStore.LookupEntry(skillId))
            continue;

        uint16 max = player->GetSkillMaxForLevel();
        if (SkillRaceClassInfoEntry const* info = player->GetSkillInfo(skillId))
        {
            if (info->skillTierId)
            {
                if (SkillTiersEntry const* tiers = sSkillTiersStore.LookupEntry(info->skillTierId))
                {
                    if (tiers->maxSkillValue[0])
                        max = uint16(tiers->maxSkillValue[0]);
                }
            }
        }

        if (skillId == SKILL_LANG_COMMON || skillId == SKILL_LANG_ORCISH || skillId == SKILL_LANG_DARNASSIAN ||
            skillId == SKILL_LANG_DWARVEN || skillId == SKILL_LANG_TAURAHE || skillId == SKILL_LANG_THALASSIAN ||
            skillId == SKILL_LANG_GNOMISH || skillId == SKILL_LANG_TROLL || skillId == SKILL_LANG_GUTTERSPEAK)
        {
            max = 300;
        }

        const uint16 value = max;
        CharacterDatabase.PExecute(
            "REPLACE INTO character_skills (guid, skill, value, max) VALUES ('%u', '%u', '%u', '%u')",
            guid, uint32(skillId), uint32(value), uint32(max));
        ++inserted;
    }

    return inserted;
}

bool FinalizeDraftCharacter(ChatHandler* handler, DraftSession& session, uint32 accountId)
{
    if (!handler)
        return false;

    WorldSession* ws = handler->GetSession();
    if (!ws)
        return false;

    Player* owner = ws->GetPlayer();
    if (!owner)
    {
        handler->SendSysMessage("DRAFT:ERROR|no_player|Unable to resolve active player");
        return false;
    }

    uint8 classId = session.draftedClass;
    uint8 race = 0;
    if (!classId || !ResolveDraftRaceForClass(owner, classId, race))
    {
        handler->SendSysMessage("DRAFT:ERROR|invalid_spec_race|Drafted class has no valid race mapping");
        return false;
    }

    uint8 charcount = 0;
    auto realmCountResult = CharacterDatabase.PQuery("SELECT COUNT(guid) FROM characters WHERE account = '%u'", accountId);
    if (realmCountResult)
    {
        Field* fields = realmCountResult->Fetch();
        charcount = fields[0].GetUInt8();
    }

    if (charcount >= sWorld.getConfig(CONFIG_UINT32_CHARACTERS_PER_REALM))
    {
        handler->SendSysMessage("DRAFT:ERROR|realm_limit|Character-per-realm limit reached");
        return false;
    }

    auto accountCountResult = LoginDatabase.PQuery("SELECT SUM(numchars) FROM realmcharacters WHERE acctid = '%u'", accountId);
    if (accountCountResult)
    {
        Field* fields = accountCountResult->Fetch();
        uint32 acctcharcount = fields[0].GetUInt32();
        if (acctcharcount >= sWorld.getConfig(CONFIG_UINT32_CHARACTERS_PER_ACCOUNT))
        {
            handler->SendSysMessage("DRAFT:ERROR|account_limit|Character-per-account limit reached");
            return false;
        }
    }

    std::string name;
    bool nameFound = false;
    for (uint32 attempt = 0; attempt < 40; ++attempt)
    {
        std::string candidate = MakeDraftCharacterName();
        if (!normalizePlayerName(candidate))
            continue;

        if (ObjectMgr::CheckPlayerName(candidate, true) != CHAR_NAME_SUCCESS)
            continue;

        if (sObjectMgr.GetPlayerGuidByName(candidate))
            continue;

        name = candidate;
        nameFound = true;
        break;
    }

    if (!nameFound)
    {
        handler->SendSysMessage("DRAFT:ERROR|name_generation|Unable to generate unique character name");
        return false;
    }

    uint8 gender = owner->getGender();
    uint8 skin = owner->GetByteValue(PLAYER_BYTES, 0);
    uint8 face = owner->GetByteValue(PLAYER_BYTES, 1);
    uint8 hairStyle = owner->GetByteValue(PLAYER_BYTES, 2);
    uint8 hairColor = owner->GetByteValue(PLAYER_BYTES, 3);
    uint8 facialHair = owner->GetByteValue(PLAYER_BYTES_2, 0);

    if (!Player::ValidateAppearance(race, classId, gender, hairStyle, hairColor, face, facialHair, skin, true))
    {
        skin = 0;
        face = 0;
        hairStyle = 0;
        hairColor = 0;
        facialHair = 0;
    }

    Player* pNewChar = new Player(ws);
    if (!pNewChar->Create(sObjectMgr.GeneratePlayerLowGuid(), name, race, classId, gender, skin, face, hairStyle, hairColor, facialHair, 0))
    {
        delete pNewChar;
        handler->SendSysMessage("DRAFT:ERROR|create_failed|Failed to create draft character");
        return false;
    }

    pNewChar->SetAtLoginFlag(AT_LOGIN_FIRST);
    pNewChar->GiveLevel(session.level);
    pNewChar->LearnDefaultSkills();
    pNewChar->learnDefaultSpells();
    pNewChar->UpdateSkillsForLevel(true);
#ifdef ENABLE_PLAYERBOTS
    pNewChar->learnClassLevelSpells(false);
#endif
    if (!pNewChar->HasSkill(SKILL_DEFENSE))
    {
        uint32 fallbackAdded = ApplyFallbackSkillsFromWorldTable(pNewChar);
        if (fallbackAdded)
            pNewChar->UpdateSkillsForLevel(true);
    }

    // Starter sustain for draft runs.
    pNewChar->SetUInt32Value(PLAYER_FIELD_COINAGE, 20000); // 2 gold
    pNewChar->StoreNewItemInBestSlots(4496, 1); // small brown pouch
    pNewChar->StoreNewItemInBestSlots(4496, 1);
    pNewChar->StoreNewItemInBestSlots(4496, 1);
    pNewChar->StoreNewItemInBestSlots(4496, 1);

    uint32 equippedCount = 0;
    for (uint32 i = 0; i < session.selectedGearItemIds.size(); ++i)
    {
        uint32 itemId = session.selectedGearItemIds[i];
        int32 randomPropertyId = i < session.selectedGearRandomPropertyIds.size() ? session.selectedGearRandomPropertyIds[i] : 0;
        if (GiveDraftItemInBestSlots(pNewChar, itemId, randomPropertyId))
            ++equippedCount;
    }

    if (AreaTrigger const* entrance = sObjectMgr.GetMapEntranceTrigger(session.mapId))
    {
        pNewChar->SetMap(sMapMgr.CreateMap(entrance->target_mapId, pNewChar));
        pNewChar->Relocate(entrance->target_X, entrance->target_Y, entrance->target_Z, entrance->target_Orientation);
    }

    pNewChar->SaveToDB();
    uint32 persistedSkills = PersistSkillsToCharacterDbFromWorldTable(pNewChar);
    ++charcount;
    LoginDatabase.PExecute("DELETE FROM realmcharacters WHERE acctid= '%u' AND realmid = '%u'", accountId, realmID);
    LoginDatabase.PExecute("INSERT INTO realmcharacters (numchars, acctid, realmid) VALUES (%u, %u, %u)", charcount, accountId, realmID);

    handler->PSendSysMessage("DRAFT:FINAL|%s|%u|%s|%u", session.dungeonId.c_str(), session.level, session.selectedSpec.c_str(), uint32(session.selectedGearItemIds.size()));
    handler->PSendSysMessage("DRAFT:SKILLS_PERSISTED|%u", persistedSkills);
#ifdef ENABLE_PLAYERBOTS
    handler->SendSysMessage("DRAFT:SPELLS|Class trainer spells auto-learned for your drafted level");
#endif
    handler->PSendSysMessage("DRAFT:GEAR_APPLIED|%u|%u", equippedCount, uint32(session.selectedGearItemIds.size()));
    handler->PSendSysMessage("DRAFT:CHAR_CREATED|%s|%u|%u|%u", name.c_str(), pNewChar->GetGUIDLow(), race, classId);
    handler->SendSysMessage("DRAFT:NEXT|Character created. Logout to character select and enter with your draft character.");

    delete pNewChar;
    return true;
}
} // namespace

bool ChatHandler::HandleHelpCommand(char* args)
{
    if (!*args)
    {
        ShowHelpForCommand(getCommandTable(), "help");
        ShowHelpForCommand(getCommandTable(), "");
    }
    else
    {
        if (!ShowHelpForCommand(getCommandTable(), args))
            SendSysMessage(LANG_NO_CMD);
    }

    return true;
}

bool ChatHandler::HandleCommandsCommand(char* /*args*/)
{
    ShowHelpForCommand(getCommandTable(), "");
    return true;
}

bool ChatHandler::HandleAccountCommand(char* args)
{
    // let show subcommands at unexpected data in args
    if (*args)
        return false;

    AccountTypes gmlevel = GetAccessLevel();
    PSendSysMessage(LANG_ACCOUNT_LEVEL, uint32(gmlevel));
    return true;
}

bool ChatHandler::HandleStartCommand(char* /*args*/)
{
    Player* chr = m_session->GetPlayer();

    if (chr->IsTaxiFlying())
    {
        SendSysMessage(LANG_YOU_IN_FLIGHT);
        SetSentErrorMessage(true);
        return false;
    }

    if (chr->IsInCombat())
    {
        SendSysMessage(LANG_YOU_IN_COMBAT);
        SetSentErrorMessage(true);
        return false;
    }

    // cast spell Stuck
    chr->CastSpell(chr, 7355, TRIGGERED_NONE);
    return true;
}

bool ChatHandler::HandleServerInfoCommand(char* /*args*/)
{
    uint32 activeClientsNum = sWorld.GetActiveSessionCount();
    uint32 queuedClientsNum = sWorld.GetQueuedSessionCount();
    uint32 maxActiveClientsNum = sWorld.GetMaxActiveSessionCount();
    uint32 maxQueuedClientsNum = sWorld.GetMaxQueuedSessionCount();
    std::string str = secsToTimeString(sWorld.GetUptime());

    char const* full;
    if (m_session)
        full = _FULLVERSION(REVISION_DATE, "|cffffffff|Hurl:" REVISION_ID "|h" REVISION_ID "|h|r");
    else
        full = _FULLVERSION(REVISION_DATE, REVISION_ID);
    SendSysMessage(full);

    PSendSysMessage(LANG_USING_WORLD_DB, sWorld.GetDBVersion());
    PSendSysMessage(LANG_USING_EVENT_AI, sWorld.GetCreatureEventAIVersion());
    PSendSysMessage(LANG_CONNECTED_USERS, activeClientsNum, maxActiveClientsNum, queuedClientsNum, maxQueuedClientsNum);
    PSendSysMessage(LANG_UPTIME, str.c_str());

    return true;
}

bool ChatHandler::HandleDismountCommand(char* /*args*/)
{
    Player* player = m_session->GetPlayer();

    // If player is not mounted, so go out :)
    if (!player->IsMounted())
    {
        SendSysMessage(LANG_CHAR_NON_MOUNTED);
        SetSentErrorMessage(true);
        return false;
    }

    if (player->IsTaxiFlying())
    {
        SendSysMessage(LANG_YOU_IN_FLIGHT);
        SetSentErrorMessage(true);
        return false;
    }

    player->Unmount();
    player->RemoveSpellsCausingAura(SPELL_AURA_MOUNTED);
    return true;
}

bool ChatHandler::HandleSaveCommand(char* /*args*/)
{
    Player* player = m_session->GetPlayer();

    // save GM account without delay and output message (testing, etc)
    if (GetAccessLevel() > SEC_PLAYER)
    {
        player->SaveToDB();
        SendSysMessage(LANG_PLAYER_SAVED);
        return true;
    }

    // save or plan save after 20 sec (logout delay) if current next save time more this value and _not_ output any messages to prevent cheat planning
    uint32 save_interval = sWorld.getConfig(CONFIG_UINT32_INTERVAL_SAVE);
    if (save_interval == 0 || (save_interval > 20 * IN_MILLISECONDS && player->GetSaveTimer() <= save_interval - 20 * IN_MILLISECONDS))
        player->SaveToDB();

    return true;
}

bool ChatHandler::HandleGMListIngameCommand(char* /*args*/)
{
    std::list< std::pair<std::string, bool> > names;

    {
        HashMapHolder<Player>::ReadGuard g(HashMapHolder<Player>::GetLock());
        HashMapHolder<Player>::MapType& m = sObjectAccessor.GetPlayers();
        for (HashMapHolder<Player>::MapType::const_iterator itr = m.begin(); itr != m.end(); ++itr)
        {
            Player* player = itr->second;
            AccountTypes security = player->GetSession()->GetSecurity();
            if ((player->IsGameMaster() || (security > SEC_PLAYER && security <= (AccountTypes)sWorld.getConfig(CONFIG_UINT32_GM_LEVEL_IN_GM_LIST))) &&
                (!m_session || player->IsVisibleGloballyFor(m_session->GetPlayer())))
                names.push_back(std::make_pair<std::string, bool>(GetNameLink(player), player->isAcceptWhispers()));
        }
    }

    if (!names.empty())
    {
        SendSysMessage(LANG_GMS_ON_SRV);

        char const* accepts = GetMangosString(LANG_GM_ACCEPTS_WHISPER);
        char const* not_accept = GetMangosString(LANG_GM_NO_WHISPER);
        for (std::list<std::pair< std::string, bool> >::const_iterator iter = names.begin(); iter != names.end(); ++iter)
            PSendSysMessage("%s - %s", iter->first.c_str(), iter->second ? accepts : not_accept);
    }
    else
        SendSysMessage(LANG_GMS_NOT_LOGGED);

    return true;
}

bool ChatHandler::HandleAccountPasswordCommand(char* args)
{
    // allow use from RA, but not from console (not have associated account id)
    if (!GetAccountId())
    {
        SendSysMessage(LANG_RA_ONLY_COMMAND);
        SetSentErrorMessage(true);
        return false;
    }

    // allow or quoted string with possible spaces or literal without spaces
    char* old_pass = ExtractQuotedOrLiteralArg(&args);
    char* new_pass = ExtractQuotedOrLiteralArg(&args);
    char* new_pass_c = ExtractQuotedOrLiteralArg(&args);

    if (!old_pass || !new_pass || !new_pass_c)
        return false;

    std::string password_old = old_pass;
    std::string password_new = new_pass;
    std::string password_new_c = new_pass_c;

    if (password_new != password_new_c)
    {
        SendSysMessage(LANG_NEW_PASSWORDS_NOT_MATCH);
        SetSentErrorMessage(true);
        return false;
    }

    if (!sAccountMgr.CheckPassword(GetAccountId(), password_old))
    {
        SendSysMessage(LANG_COMMAND_WRONGOLDPASSWORD);
        SetSentErrorMessage(true);
        return false;
    }

    AccountOpResult result = sAccountMgr.ChangePassword(GetAccountId(), password_new);

    switch (result)
    {
        case AOR_OK:
            SendSysMessage(LANG_COMMAND_PASSWORD);
            break;
        case AOR_PASS_TOO_LONG:
            SendSysMessage(LANG_PASSWORD_TOO_LONG);
            SetSentErrorMessage(true);
            return false;
        case AOR_NAME_NOT_EXIST:                            // not possible case, don't want get account name for output
        default:
            SendSysMessage(LANG_COMMAND_NOTCHANGEPASSWORD);
            SetSentErrorMessage(true);
            return false;
    }

    // OK, but avoid normal report for hide passwords, but log use command for anyone
    LogCommand(".account password *** *** ***");
    SetSentErrorMessage(true);
    return false;
}

bool ChatHandler::HandleAccountLockCommand(char* args)
{
    // allow use from RA, but not from console (not have associated account id)
    if (!GetAccountId())
    {
        SendSysMessage(LANG_RA_ONLY_COMMAND);
        SetSentErrorMessage(true);
        return false;
    }

    bool value;
    if (!ExtractOnOff(&args, value))
    {
        SendSysMessage(LANG_USE_BOL);
        SetSentErrorMessage(true);
        return false;
    }

    if (value)
    {
        LoginDatabase.PExecute("UPDATE account SET locked = '1' WHERE id = '%u'", GetAccountId());
        PSendSysMessage(LANG_COMMAND_ACCLOCKLOCKED);
    }
    else
    {
        LoginDatabase.PExecute("UPDATE account SET locked = '0' WHERE id = '%u'", GetAccountId());
        PSendSysMessage(LANG_COMMAND_ACCLOCKUNLOCKED);
    }

    return true;
}

/// Display the 'Message of the day' for the realm
bool ChatHandler::HandleServerMotdCommand(char* /*args*/)
{
    PSendSysMessage(LANG_MOTD_CURRENT, sWorld.GetMotd());
    return true;
}

bool ChatHandler::HandleWhisperRestrictionCommand(char* args)
{
    if (!*args)
    {
        PSendSysMessage("Whisper restriction is %s.", m_session->GetPlayer()->isEnabledWhisperRestriction() ? "ON" : "OFF");
        return true;
    }

    bool value;
    if (!ExtractOnOff(&args, value))
    {
        SendSysMessage(LANG_USE_BOL);
        SetSentErrorMessage(true);
        return false;
    }

    m_session->GetPlayer()->SetWhisperRestriction(value);
    PSendSysMessage("Whisper restriction is now %s.", value ? "ON. Only friends, group members, or guildmates may whisper you." : "OFF");

    return true;
}

bool ChatHandler::HandleDraftCommand(char* args)
{
    if (!m_session || !m_session->GetPlayer())
        return false;

    if (!*args)
    {
        SendSysMessage("Draft commands: .draft start, .draft page <prev|next|#>, .draft pick <1-3>, .draft status, .draft finalize, .draft cancel");
        return true;
    }

    char* subCmdArg = ExtractArg(&args);
    if (!subCmdArg)
        return false;

    std::string subCmd = ToLowerCopy(subCmdArg);
    uint32 accountId = GetAccountId();

    if (subCmd == "start")
    {
        DraftSession session;
        std::vector<DraftOption> dungeonPool = BuildDungeonPool();
        if (dungeonPool.empty())
        {
            SendSysMessage("DRAFT:ERROR|pool_empty|Dungeon pool is not available");
            return true;
        }

        session.phase = DraftPhase::Dungeon;
        session.dungeonPool = dungeonPool;
        session.dungeonPage = 0;
        s_draftSessions[accountId] = session;

        SendSysMessage("DRAFT:BEGIN|none|0|0");
        SendSysMessage("DRAFT:PHASE|dungeon");
        SendDungeonPage(this, s_draftSessions[accountId]);
        return true;
    }

    std::map<uint32, DraftSession>::iterator itr = s_draftSessions.find(accountId);
    if (itr == s_draftSessions.end())
    {
        SendSysMessage("DRAFT:ERROR|no_session|Start a draft with .draft start");
        return true;
    }

    DraftSession& session = itr->second;

    if (subCmd == "page")
    {
        if (session.phase != DraftPhase::Dungeon)
        {
            SendSysMessage("DRAFT:ERROR|wrong_phase|Dungeon paging is only available during dungeon selection");
            return true;
        }

        char* pageArg = ExtractArg(&args);
        if (!pageArg)
        {
            SendSysMessage("DRAFT:ERROR|bad_page|Usage: .draft page <prev|next|#>");
            return true;
        }

        std::string pageValue = ToLowerCopy(pageArg);
        uint32 totalPages = (uint32(session.dungeonPool.size()) + DRAFT_DUNGEON_PAGE_SIZE - 1) / DRAFT_DUNGEON_PAGE_SIZE;
        if (totalPages == 0)
        {
            SendSysMessage("DRAFT:ERROR|pool_empty|Dungeon pool is not available");
            return true;
        }

        if (pageValue == "next")
            session.dungeonPage = (session.dungeonPage + 1) % totalPages;
        else if (pageValue == "prev")
            session.dungeonPage = session.dungeonPage == 0 ? (totalPages - 1) : (session.dungeonPage - 1);
        else
        {
            int32 requestedPage = atoi(pageValue.c_str());
            if (requestedPage < 1 || requestedPage > int32(totalPages))
            {
                PSendSysMessage("DRAFT:ERROR|bad_page|Page must be between 1 and %u", totalPages);
                return true;
            }
            session.dungeonPage = uint32(requestedPage - 1);
        }

        SendDungeonPage(this, session);
        return true;
    }

    if (subCmd == "pick")
    {
        char* idxArg = ExtractArg(&args);
        if (!idxArg)
            return false;

        int32 selected = atoi(idxArg);
        if (selected < 1 || selected > 3)
        {
            SendSysMessage("DRAFT:ERROR|bad_pick|Pick index must be 1, 2 or 3");
            return true;
        }

        uint32 pickIndex = uint32(selected - 1);

        if (session.phase == DraftPhase::Dungeon)
        {
            if (pickIndex >= session.dungeonOptions.size())
            {
                SendSysMessage("DRAFT:ERROR|bad_pick|Dungeon pick out of range");
                return true;
            }

            DraftOption const& selectedDungeon = session.dungeonOptions[pickIndex];
            DungeonTemplate dungeonTemplate;
            if (!ResolveDungeonTemplate(selectedDungeon.id, dungeonTemplate))
            {
                SendSysMessage("DRAFT:ERROR|bad_dungeon|Selected dungeon template is invalid");
                return true;
            }

            session.dungeonId = dungeonTemplate.id;
            session.mapId = dungeonTemplate.mapId;
            session.level = (dungeonTemplate.levelMin + dungeonTemplate.levelMax) / 2;

            std::vector<DraftOption> specPool = BuildSpecPoolForDungeon(session.dungeonId);
            if (specPool.size() < 3)
            {
                SendSysMessage("DRAFT:ERROR|pool_empty|Not enough specs in current pool");
                return true;
            }

            session.phase = DraftPhase::Spec;
            session.specOptions = PickOptions(specPool, 3);

            PSendSysMessage("DRAFT:SELECTED|dungeon|%s|%s", selectedDungeon.id.c_str(), selectedDungeon.name.c_str());
            PSendSysMessage("DRAFT:BEGIN|%s|%u|%u", session.dungeonId.c_str(), session.mapId, session.level);
            SendSysMessage("DRAFT:PHASE|spec");
            SendDraftOptions(this, "SPEC", session.specOptions);
            return true;
        }

        if (session.phase == DraftPhase::Spec)
        {
            if (pickIndex >= session.specOptions.size())
            {
                SendSysMessage("DRAFT:ERROR|bad_pick|Spec pick out of range");
                return true;
            }

            DraftOption const& selectedSpec = session.specOptions[pickIndex];
            session.selectedSpec = selectedSpec.id;
            session.draftedClass = GetClassFromSpecId(session.selectedSpec);
            session.gearSlots = BuildGearSlotsForDraft();
            session.gearSlotIndex = 0;
            session.selectedGearItemIds.clear();
            session.selectedGearRandomPropertyIds.clear();
            session.phase = DraftPhase::Gear;

            PSendSysMessage("DRAFT:SELECTED|spec|%s|%s", selectedSpec.id.c_str(), selectedSpec.name.c_str());
            SendSysMessage("DRAFT:PHASE|gear");
            if (!OpenCurrentGearRound(this, session))
            {
                s_draftSessions.erase(accountId);
                SendSysMessage("DRAFT:CANCELLED");
                return true;
            }

            if (session.phase == DraftPhase::Done)
            {
                if (FinalizeDraftCharacter(this, session, accountId))
                    s_draftSessions.erase(accountId);
            }
            return true;
        }

        if (session.phase == DraftPhase::Gear)
        {
            if (pickIndex >= session.gearOptions.size())
            {
                SendSysMessage("DRAFT:ERROR|bad_pick|Gear pick out of range");
                return true;
            }

            DraftOption const& selectedGear = session.gearOptions[pickIndex];
            uint32 itemId = uint32(atoi(selectedGear.id.c_str()));
            session.selectedGearItemIds.push_back(itemId);
            session.selectedGearRandomPropertyIds.push_back(selectedGear.randomPropertyId);
            uint32 currentSlot = session.gearSlotIndex < session.gearSlots.size() ? session.gearSlots[session.gearSlotIndex] : 0;
            ++session.gearSlotIndex;

            PSendSysMessage("DRAFT:SELECTED|gear|%s|%u|%s", InventoryTypeToLabel(currentSlot), itemId, selectedGear.name.c_str());
            if (!OpenCurrentGearRound(this, session))
            {
                s_draftSessions.erase(accountId);
                SendSysMessage("DRAFT:CANCELLED");
            }

            if (session.phase == DraftPhase::Done)
            {
                if (FinalizeDraftCharacter(this, session, accountId))
                    s_draftSessions.erase(accountId);
            }
            return true;
        }

        SendSysMessage("DRAFT:ERROR|wrong_phase|No active pick expected right now");
        return true;
    }

    if (subCmd == "status")
    {
        char const* phase = "none";
        switch (session.phase)
        {
            case DraftPhase::Dungeon: phase = "dungeon"; break;
            case DraftPhase::Spec: phase = "spec"; break;
            case DraftPhase::Gear: phase = "gear"; break;
            case DraftPhase::Done: phase = "done"; break;
            default: break;
        }

        PSendSysMessage("DRAFT:STATUS|%s|%u|%s|%s|%u|%u", session.dungeonId.c_str(), session.level, phase,
            session.selectedSpec.empty() ? "-" : session.selectedSpec.c_str(),
            uint32(session.selectedGearItemIds.size()), uint32(session.gearSlots.size()));
        return true;
    }

    if (subCmd == "finalize")
    {
        if (session.phase != DraftPhase::Done || session.selectedSpec.empty() || session.selectedGearItemIds.size() != session.gearSlots.size())
        {
            SendSysMessage("DRAFT:ERROR|not_ready|Finish all picks before finalizing");
            return true;
        }
        if (FinalizeDraftCharacter(this, session, accountId))
            s_draftSessions.erase(accountId);
        return true;
    }

    if (subCmd == "cancel")
    {
        s_draftSessions.erase(accountId);
        SendSysMessage("DRAFT:CANCELLED");
        return true;
    }

    return false;
}
