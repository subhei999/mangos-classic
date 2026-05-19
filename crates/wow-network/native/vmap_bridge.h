#pragma once

#include "IVMapManager.h"

#include <mutex>
#include <string>

std::mutex& wow_vmap_bridge_mutex();

bool wow_vmap_ensure_tile_loaded(
    VMAP::IVMapManager* manager,
    const std::string& basePath,
    unsigned int mapId,
    unsigned int tileX,
    unsigned int tileY);
