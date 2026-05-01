#include "IVMapManager.h"
#include "VMapFactory.h"

#include <cmath>
#include <cstdio>
#include <mutex>
#include <string>

namespace
{
std::mutex g_vmapMutex;

bool tileIdIsValid(unsigned int tile)
{
    return tile < 64;
}

std::string pathJoin(const char* dataDir, const char* child)
{
    std::string path(dataDir ? dataDir : "");
    if (!path.empty() && path.back() != '/' && path.back() != '\\')
        path.push_back('/');
    path.append(child);
    return path;
}

std::string vmapBasePath(const char* dataDir)
{
    std::string path = pathJoin(dataDir, "vmaps");
    if (!path.empty() && path.back() != '/' && path.back() != '\\')
        path.push_back('/');
    return path;
}
}

extern "C"
{
int wow_vmap_line_of_sight(
    const char* dataDir,
    unsigned int mapId,
    unsigned int startTileX,
    unsigned int startTileY,
    unsigned int targetTileX,
    unsigned int targetTileY,
    float startX,
    float startY,
    float startZ,
    float targetX,
    float targetY,
    float targetZ,
    int ignoreM2Model) noexcept
{
    try
    {
        if (!dataDir)
            return -1;
        if (!tileIdIsValid(startTileX) || !tileIdIsValid(startTileY) ||
            !tileIdIsValid(targetTileX) || !tileIdIsValid(targetTileY))
            return -2;
        if (!std::isfinite(startX) || !std::isfinite(startY) || !std::isfinite(startZ) ||
            !std::isfinite(targetX) || !std::isfinite(targetY) || !std::isfinite(targetZ))
            return -3;

        std::lock_guard<std::mutex> lock(g_vmapMutex);
        VMAP::IVMapManager* manager = VMAP::VMapFactory::createOrGetVMapManager();
        if (!manager)
            return -4;

        const std::string basePath = vmapBasePath(dataDir);
        const VMAP::VMAPLoadResult startLoad =
            manager->loadMap(basePath.c_str(), mapId, static_cast<int>(startTileX), static_cast<int>(startTileY));
        if (startLoad == VMAP::VMAP_LOAD_RESULT_ERROR)
            return -5;
        const VMAP::VMAPLoadResult targetLoad =
            manager->loadMap(basePath.c_str(), mapId, static_cast<int>(targetTileX), static_cast<int>(targetTileY));
        if (targetLoad == VMAP::VMAP_LOAD_RESULT_ERROR)
            return -6;

        const bool clear = manager->isInLineOfSight(
            mapId,
            startX,
            startY,
            startZ,
            targetX,
            targetY,
            targetZ,
            ignoreM2Model != 0);
        return clear ? 1 : 0;
    }
    catch (...)
    {
        return -100;
    }
}
}
