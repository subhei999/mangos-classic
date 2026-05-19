#include "vmap_bridge.h"

std::mutex& wow_vmap_bridge_mutex()
{
    static std::mutex mutex;
    return mutex;
}

bool wow_vmap_ensure_tile_loaded(
    VMAP::IVMapManager* manager,
    const std::string& basePath,
    unsigned int mapId,
    unsigned int tileX,
    unsigned int tileY)
{
    if (!manager)
        return false;
    if (manager->IsTileLoaded(mapId, tileX, tileY))
        return true;
    return manager->loadMap(
               basePath.c_str(),
               mapId,
               static_cast<int>(tileX),
               static_cast<int>(tileY)) != VMAP::VMAP_LOAD_RESULT_ERROR;
}
