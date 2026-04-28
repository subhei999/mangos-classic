#include "DetourAlloc.h"
#include "DetourCommon.h"
#include "DetourNavMesh.h"
#include "DetourNavMeshQuery.h"
#include "DetourStatus.h"

#include <cstdio>
#include <cstring>
#include <memory>
#include <mutex>
#include <string>
#include <unordered_map>
#include <unordered_set>
#include <vector>

namespace
{
constexpr unsigned int MMAP_MAGIC = 0x4d4d4150;
constexpr int MAX_PATH_POLYS = 74;
constexpr int MAX_STRAIGHT_POINTS = 32;
constexpr unsigned short NAV_GROUND = 1;

struct MmapTileHeader
{
    unsigned int mmapMagic;
    unsigned int dtVersion;
    unsigned int mmapVersion;
    unsigned int size;
    unsigned int usesLiquids;
};

struct CachedMap
{
    dtNavMesh* mesh = nullptr;
    std::unordered_set<unsigned int> loadedTiles;

    ~CachedMap()
    {
        if (mesh)
            dtFreeNavMesh(mesh);
    }
};

std::mutex g_mapsMutex;
std::unordered_map<std::string, std::unique_ptr<CachedMap>> g_maps;

std::string pathJoin(const char* dataDir, const char* child)
{
    std::string path(dataDir ? dataDir : "");
    if (!path.empty() && path.back() != '/' && path.back() != '\\')
        path.push_back('/');
    path.append(child);
    return path;
}

std::string mapKey(const char* dataDir, unsigned int mapId)
{
    return std::string(dataDir ? dataDir : "") + "|" + std::to_string(mapId);
}

unsigned int packTile(unsigned int tileX, unsigned int tileY)
{
    return (tileX << 16) | tileY;
}

std::string mmapFileName(const char* dataDir, unsigned int mapId)
{
    char child[32];
    std::snprintf(child, sizeof(child), "mmaps/%03u.mmap", mapId);
    return pathJoin(dataDir, child);
}

std::string tileFileName(const char* dataDir, unsigned int mapId, unsigned int tileX, unsigned int tileY)
{
    char child[40];
    std::snprintf(child, sizeof(child), "mmaps/%03u%02u%02u.mmtile", mapId, tileX, tileY);
    return pathJoin(dataDir, child);
}

CachedMap* loadMapDataLocked(const char* dataDir, unsigned int mapId, int* errorCode)
{
    const std::string key = mapKey(dataDir, mapId);
    auto existing = g_maps.find(key);
    if (existing != g_maps.end())
        return existing->second.get();

    const std::string fileName = mmapFileName(dataDir, mapId);
    FILE* file = std::fopen(fileName.c_str(), "rb");
    if (!file)
    {
        if (errorCode)
            *errorCode = -20;
        return nullptr;
    }

    dtNavMeshParams params;
    const size_t read = std::fread(&params, sizeof(dtNavMeshParams), 1, file);
    std::fclose(file);
    if (read != 1)
    {
        if (errorCode)
            *errorCode = -21;
        return nullptr;
    }

    dtNavMesh* mesh = dtAllocNavMesh();
    if (!mesh)
    {
        if (errorCode)
            *errorCode = -22;
        return nullptr;
    }
    dtStatus initStatus = mesh->init(&params);
    if (dtStatusFailed(initStatus))
    {
        dtFreeNavMesh(mesh);
        if (errorCode)
            *errorCode = -23;
        return nullptr;
    }

    auto cached = std::make_unique<CachedMap>();
    cached->mesh = mesh;
    CachedMap* ptr = cached.get();
    g_maps.emplace(key, std::move(cached));
    return ptr;
}

bool loadTileLocked(CachedMap& cached, const char* dataDir, unsigned int mapId, unsigned int tileX, unsigned int tileY)
{
    const unsigned int packed = packTile(tileX, tileY);
    if (cached.loadedTiles.find(packed) != cached.loadedTiles.end())
        return true;

    const std::string fileName = tileFileName(dataDir, mapId, tileX, tileY);
    FILE* file = std::fopen(fileName.c_str(), "rb");
    if (!file)
        return false;

    MmapTileHeader header;
    if (std::fread(&header, sizeof(MmapTileHeader), 1, file) != 1)
    {
        std::fclose(file);
        return false;
    }
    if (header.mmapMagic != MMAP_MAGIC || header.size == 0)
    {
        std::fclose(file);
        return false;
    }

    unsigned char* data = static_cast<unsigned char*>(dtAlloc(header.size, DT_ALLOC_PERM));
    if (!data)
    {
        std::fclose(file);
        return false;
    }
    if (std::fread(data, header.size, 1, file) != 1)
    {
        dtFree(data);
        std::fclose(file);
        return false;
    }
    std::fclose(file);

    dtTileRef tileRef = 0;
    if (dtStatusFailed(cached.mesh->addTile(data, header.size, DT_TILE_FREE_DATA, 0, &tileRef)))
    {
        dtFree(data);
        return false;
    }
    cached.loadedTiles.insert(packed);
    return true;
}

bool loadNeighborTilesLocked(CachedMap& cached, const char* dataDir, unsigned int mapId, unsigned int tileX, unsigned int tileY)
{
    bool loadedCenter = false;
    for (int dx = -1; dx <= 1; ++dx)
    {
        for (int dy = -1; dy <= 1; ++dy)
        {
            const int nx = static_cast<int>(tileX) + dx;
            const int ny = static_cast<int>(tileY) + dy;
            if (nx < 0 || ny < 0 || nx > 63 || ny > 63)
                continue;
            const bool loaded = loadTileLocked(cached, dataDir, mapId, static_cast<unsigned int>(nx), static_cast<unsigned int>(ny));
            if (dx == 0 && dy == 0)
                loadedCenter = loaded;
        }
    }
    return loadedCenter;
}
}

extern "C"
{
struct WowMmapPathPoint
{
    float x;
    float y;
    float z;
};

int wow_mmap_find_path(
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
    WowMmapPathPoint* outPoints,
    int maxPoints)
{
    if (!dataDir || !outPoints || maxPoints < 2)
        return -1;

    std::lock_guard<std::mutex> lock(g_mapsMutex);
    int loadMapError = -2;
    CachedMap* cached = loadMapDataLocked(dataDir, mapId, &loadMapError);
    if (!cached || !cached->mesh)
        return loadMapError;

    if (!loadNeighborTilesLocked(*cached, dataDir, mapId, startTileX, startTileY))
        return -3;
    if (!loadNeighborTilesLocked(*cached, dataDir, mapId, targetTileX, targetTileY))
        return -4;

    dtNavMeshQuery* query = dtAllocNavMeshQuery();
    if (!query)
        return -5;
    if (dtStatusFailed(query->init(cached->mesh, 2048)))
    {
        dtFreeNavMeshQuery(query);
        return -6;
    }

    dtQueryFilter filter;
    filter.setIncludeFlags(NAV_GROUND);
    filter.setExcludeFlags(0);

    const float startPoint[3] = { startY, startZ, startX };
    const float targetPoint[3] = { targetY, targetZ, targetX };
    const float extents[3] = { 5.0f, 5.0f, 5.0f };
    float nearestStart[3] = { 0.0f, 0.0f, 0.0f };
    float nearestTarget[3] = { 0.0f, 0.0f, 0.0f };
    dtPolyRef startRef = 0;
    dtPolyRef targetRef = 0;

    if (dtStatusFailed(query->findNearestPoly(startPoint, extents, &filter, &startRef, nearestStart)) || !startRef)
    {
        dtFreeNavMeshQuery(query);
        return 0;
    }
    if (dtStatusFailed(query->findNearestPoly(targetPoint, extents, &filter, &targetRef, nearestTarget)) || !targetRef)
    {
        dtFreeNavMeshQuery(query);
        return 0;
    }

    dtPolyRef polys[MAX_PATH_POLYS];
    int polyCount = 0;
    if (dtStatusFailed(query->findPath(startRef, targetRef, nearestStart, nearestTarget, &filter, polys, &polyCount, MAX_PATH_POLYS)) || polyCount <= 0)
    {
        dtFreeNavMeshQuery(query);
        return 0;
    }

    const int straightLimit = maxPoints < MAX_STRAIGHT_POINTS ? maxPoints : MAX_STRAIGHT_POINTS;
    std::vector<float> straight(straightLimit * 3);
    int straightCount = 0;
    if (dtStatusFailed(query->findStraightPath(nearestStart, nearestTarget, polys, polyCount, straight.data(), nullptr, nullptr, &straightCount, straightLimit)) || straightCount < 2)
    {
        dtFreeNavMeshQuery(query);
        return 0;
    }

    for (int i = 0; i < straightCount; ++i)
    {
        const int offset = i * 3;
        outPoints[i].x = straight[offset + 2];
        outPoints[i].y = straight[offset];
        outPoints[i].z = straight[offset + 1];
    }

    dtFreeNavMeshQuery(query);
    return straightCount;
}
}
