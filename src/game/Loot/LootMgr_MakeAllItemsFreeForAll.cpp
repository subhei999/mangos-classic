void Loot::MakeAllItemsFreeForAll()
{
    // Iterate through all loot items and make them free-for-all
    for (LootItem* item : m_lootItems)
    {
        if (item)
        {
            item->freeForAll = true;      // Mark as free-for-all
            item->allowedGuid.clear();    // Clear per-player permissions
        }
    }
}
