#include "DetourAlloc.h"
#include "DetourCommon.h"
#include "DetourMath.h"
#include "DetourNavMesh.h"
#include "DetourNavMeshQuery.h"
#include "DetourStatus.h"

#include <chrono>
#include <cmath>
#include <cstdint>
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
constexpr unsigned int MAX_TILE_DATA_SIZE = 64u * 1024u * 1024u;
constexpr int MAX_PATH_POLYS = 74;
constexpr int MAX_SMOOTH_POINTS = 74;
constexpr int VERTEX_SIZE = 3;
constexpr float SMOOTH_PATH_STEP_SIZE = 4.0f;
constexpr float SMOOTH_PATH_SLOP = 0.3f;
constexpr float WOW_GRID_CENTER_ID = 32.0f;
constexpr float WOW_GRID_SIZE = 533.3333f;
constexpr float PI = 3.14159265358979323846f;
constexpr unsigned short NAV_GROUND = 1;
constexpr float NEAR_POLY_SEARCH_BOUND = 5.0f;
constexpr float FAR_POLY_SEARCH_BOUND = 10.0f;
constexpr int PATHFIND_NORMAL = 0x0001;
constexpr int PATHFIND_INCOMPLETE = 0x0004;
constexpr int PATHFIND_NOPATH = 0x0008;
using SteadyClock = std::chrono::steady_clock;

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

bool tileForPosition(float x, float y, unsigned int& tileX, unsigned int& tileY)
{
    if (!std::isfinite(x) || !std::isfinite(y))
        return false;

    const int computedX = static_cast<int>(WOW_GRID_CENTER_ID - x / WOW_GRID_SIZE);
    const int computedY = static_cast<int>(WOW_GRID_CENTER_ID - y / WOW_GRID_SIZE);
    if (computedX < 0 || computedX > 63 || computedY < 0 || computedY > 63)
        return false;

    tileX = static_cast<unsigned int>(computedX);
    tileY = static_cast<unsigned int>(computedY);
    return true;
}

float clampUnit(float value)
{
    if (value < 0.0f)
        return 0.0f;
    if (value > 1.0f)
        return 1.0f;
    return value;
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
    if (header.mmapMagic != MMAP_MAGIC || header.size == 0 || header.size > MAX_TILE_DATA_SIZE)
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

bool inRangeYzx(const float* left, const float* right, float radius, float height)
{
    const float dx = right[0] - left[0];
    const float dy = right[1] - left[1];
    const float dz = right[2] - left[2];
    return (dx * dx + dz * dz) < radius * radius && std::fabs(dy) < height;
}

unsigned int fixupCorridor(dtPolyRef* path, unsigned int pathCount, unsigned int maxPath, const dtPolyRef* visited, unsigned int visitedCount)
{
    int furthestPath = -1;
    int furthestVisited = -1;

    for (int i = static_cast<int>(pathCount) - 1; i >= 0; --i)
    {
        bool found = false;
        for (int j = static_cast<int>(visitedCount) - 1; j >= 0; --j)
        {
            if (path[i] == visited[j])
            {
                furthestPath = i;
                furthestVisited = j;
                found = true;
            }
        }
        if (found)
            break;
    }

    if (furthestPath == -1 || furthestVisited == -1)
        return pathCount;

    const unsigned int required = visitedCount - static_cast<unsigned int>(furthestVisited);
    const unsigned int original = static_cast<unsigned int>(furthestPath + 1) < pathCount ? static_cast<unsigned int>(furthestPath + 1) : pathCount;
    unsigned int size = pathCount > original ? pathCount - original : 0;
    if (required + size > maxPath)
        size = maxPath - required;
    if (required >= maxPath)
        return pathCount;

    if (size)
        std::memmove(path + required, path + original, size * sizeof(dtPolyRef));

    for (unsigned int i = 0; i < required; ++i)
        path[i] = visited[(visitedCount - 1) - i];

    return required + size;
}

bool getSteerTarget(
    dtNavMeshQuery* query,
    const dtPolyRef* path,
    unsigned int pathCount,
    const dtQueryFilter& filter,
    const float* startPos,
    const float* endPos,
    float minTargetDistance,
    float* steerPos,
    unsigned char& steerPosFlag,
    dtPolyRef& steerPosRef)
{
    constexpr unsigned int MAX_STEER_POINTS = 3;
    float steerPath[MAX_STEER_POINTS * VERTEX_SIZE];
    unsigned char steerPathFlags[MAX_STEER_POINTS];
    dtPolyRef steerPathPolys[MAX_STEER_POINTS];
    int steerPathCount = 0;

    const dtStatus status = query->findStraightPath(
        startPos,
        endPos,
        path,
        static_cast<int>(pathCount),
        steerPath,
        steerPathFlags,
        steerPathPolys,
        &steerPathCount,
        static_cast<int>(MAX_STEER_POINTS));
    if (!steerPathCount || dtStatusFailed(status))
        return false;

    int steerIndex = 0;
    while (steerIndex < steerPathCount)
    {
        const float* point = &steerPath[steerIndex * VERTEX_SIZE];
        if ((steerPathFlags[steerIndex] & DT_STRAIGHTPATH_OFFMESH_CONNECTION) ||
            !inRangeYzx(point, startPos, minTargetDistance, 1000.0f))
        {
            break;
        }
        ++steerIndex;
    }
    if (steerIndex >= steerPathCount)
        return false;

    dtVcopy(steerPos, &steerPath[steerIndex * VERTEX_SIZE]);
    steerPos[1] = startPos[1];
    steerPosFlag = steerPathFlags[steerIndex];
    steerPosRef = steerPathPolys[steerIndex];
    return true;
}

dtStatus findSmoothPath(
    dtNavMesh* mesh,
    dtNavMeshQuery* query,
    const dtQueryFilter& filter,
    const float* startPos,
    const float* endPos,
    const dtPolyRef* polyPath,
    int polyPathCount,
    float* smoothPath,
    int* smoothPathCount,
    int maxSmoothPathCount)
{
    *smoothPathCount = 0;
    if (!mesh || !query || !polyPath || polyPathCount <= 0 || !smoothPath || maxSmoothPathCount < 2)
        return DT_FAILURE;

    dtPolyRef smoothPolys[MAX_PATH_POLYS];
    const int copiedPolys = polyPathCount < MAX_PATH_POLYS ? polyPathCount : MAX_PATH_POLYS;
    std::memcpy(smoothPolys, polyPath, copiedPolys * sizeof(dtPolyRef));
    unsigned int polyCount = static_cast<unsigned int>(copiedPolys);

    float iterPos[VERTEX_SIZE];
    if (dtStatusFailed(query->closestPointOnPolyBoundary(smoothPolys[0], startPos, iterPos)))
        return DT_FAILURE;

    float targetPos[VERTEX_SIZE];
    if (dtStatusFailed(query->closestPointOnPolyBoundary(smoothPolys[polyCount - 1], endPos, targetPos)))
        return DT_FAILURE;

    unsigned int smoothCount = 0;
    dtVcopy(&smoothPath[smoothCount * VERTEX_SIZE], iterPos);
    ++smoothCount;

    while (polyCount && smoothCount < static_cast<unsigned int>(maxSmoothPathCount))
    {
        float steerPos[VERTEX_SIZE];
        unsigned char steerPosFlag = 0;
        dtPolyRef steerPosRef = 0;
        if (!getSteerTarget(query, smoothPolys, polyCount, filter, iterPos, targetPos, SMOOTH_PATH_SLOP, steerPos, steerPosFlag, steerPosRef))
            break;

        const bool endOfPath = (steerPosFlag & DT_STRAIGHTPATH_END) != 0;
        const bool offMeshConnection = (steerPosFlag & DT_STRAIGHTPATH_OFFMESH_CONNECTION) != 0;

        float delta[VERTEX_SIZE];
        dtVsub(delta, steerPos, iterPos);
        float length = dtMathSqrtf(dtVdot(delta, delta));
        if ((endOfPath || offMeshConnection) && length < SMOOTH_PATH_STEP_SIZE)
            length = 1.0f;
        else if (length > 0.0f)
            length = SMOOTH_PATH_STEP_SIZE / length;
        else
            break;

        float moveTarget[VERTEX_SIZE];
        dtVmad(moveTarget, iterPos, delta, length);

        float result[VERTEX_SIZE];
        constexpr unsigned int MAX_VISIT_POLY = 16;
        dtPolyRef visited[MAX_VISIT_POLY];
        int visitedCount = 0;
        if (dtStatusFailed(query->moveAlongSurface(smoothPolys[0], iterPos, moveTarget, &filter, result, visited, &visitedCount, static_cast<int>(MAX_VISIT_POLY))))
            break;

        polyCount = fixupCorridor(smoothPolys, polyCount, MAX_PATH_POLYS, visited, static_cast<unsigned int>(visitedCount));

        if (dtStatusFailed(query->getPolyHeight(smoothPolys[0], result, &result[1])))
            break;
        result[1] += 0.5f;
        dtVcopy(iterPos, result);

        if (endOfPath && inRangeYzx(iterPos, steerPos, SMOOTH_PATH_SLOP, 1.0f))
        {
            dtVcopy(iterPos, targetPos);
            if (smoothCount < static_cast<unsigned int>(maxSmoothPathCount))
            {
                dtVcopy(&smoothPath[smoothCount * VERTEX_SIZE], iterPos);
                ++smoothCount;
            }
            break;
        }

        if (offMeshConnection && inRangeYzx(iterPos, steerPos, SMOOTH_PATH_SLOP, 1.0f))
        {
            dtPolyRef prevRef = 0;
            dtPolyRef polyRef = smoothPolys[0];
            unsigned int position = 0;
            while (position < polyCount && polyRef != steerPosRef)
            {
                prevRef = polyRef;
                polyRef = smoothPolys[position];
                ++position;
            }

            for (unsigned int i = position; i < polyCount; ++i)
                smoothPolys[i - position] = smoothPolys[i];
            polyCount -= position;

            float newStartPos[VERTEX_SIZE];
            float newEndPos[VERTEX_SIZE];
            if (dtStatusSucceed(mesh->getOffMeshConnectionPolyEndPoints(prevRef, polyRef, newStartPos, newEndPos)))
            {
                if (smoothCount < static_cast<unsigned int>(maxSmoothPathCount))
                {
                    dtVcopy(&smoothPath[smoothCount * VERTEX_SIZE], startPos);
                    ++smoothCount;
                }
                dtVcopy(iterPos, endPos);
                if (polyCount && dtStatusSucceed(query->getPolyHeight(smoothPolys[0], iterPos, &iterPos[1])))
                    iterPos[1] += 0.5f;
            }
        }

        if (smoothCount < static_cast<unsigned int>(maxSmoothPathCount))
        {
            dtVcopy(&smoothPath[smoothCount * VERTEX_SIZE], iterPos);
            ++smoothCount;
        }
    }

    *smoothPathCount = static_cast<int>(smoothCount);
    return DT_SUCCESS;
}

bool findNearestPolyWithCmangosBounds(
    dtNavMeshQuery* query,
    const float* point,
    const dtQueryFilter& filter,
    dtPolyRef* polyRef,
    float* closestPoint)
{
    const float nearExtents[VERTEX_SIZE] = {
        NEAR_POLY_SEARCH_BOUND,
        NEAR_POLY_SEARCH_BOUND,
        NEAR_POLY_SEARCH_BOUND,
    };
    if (dtStatusSucceed(query->findNearestPoly(point, nearExtents, &filter, polyRef, closestPoint)) && *polyRef)
        return true;

    // CMaNGOS PathFinder retries a wider box before treating the point as
    // off-mesh. This keeps chase destinations near walls from collapsing to
    // "no path" when a reachable polygon is just outside the near search.
    const float farExtents[VERTEX_SIZE] = {
        FAR_POLY_SEARCH_BOUND,
        FAR_POLY_SEARCH_BOUND,
        FAR_POLY_SEARCH_BOUND,
    };
    return dtStatusSucceed(query->findNearestPoly(point, farExtents, &filter, polyRef, closestPoint)) &&
           *polyRef;
}

std::uint64_t elapsedNanos(SteadyClock::time_point start)
{
    return static_cast<std::uint64_t>(
        std::chrono::duration_cast<std::chrono::nanoseconds>(SteadyClock::now() - start).count());
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

struct WowMmapCallTimings
{
    std::uint64_t lock_and_tile_load_nanos;
    std::uint64_t query_alloc_init_nanos;
    std::uint64_t find_nearest_poly_nanos;
    std::uint64_t find_path_nanos;
    std::uint64_t find_smooth_path_nanos;
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
    unsigned short includeFlags,
    unsigned short excludeFlags,
    WowMmapPathPoint* outPoints,
    int maxPoints,
    int* outPathStatus,
    WowMmapCallTimings* outTimings) noexcept
{
    try
    {
        if (outTimings)
            std::memset(outTimings, 0, sizeof(*outTimings));
        if (outPathStatus)
            *outPathStatus = PATHFIND_NOPATH;
        if (!dataDir || !outPoints || maxPoints < 2 || maxPoints > MAX_SMOOTH_POINTS)
            return -1;
        if (startTileX > 63 || startTileY > 63 || targetTileX > 63 || targetTileY > 63)
            return -7;
        if (!std::isfinite(startX) || !std::isfinite(startY) || !std::isfinite(startZ) ||
            !std::isfinite(targetX) || !std::isfinite(targetY) || !std::isfinite(targetZ))
            return -8;

        const auto lockAndTileLoadStart = SteadyClock::now();
        std::lock_guard<std::mutex> lock(g_mapsMutex);
        int loadMapError = -2;
        CachedMap* cached = loadMapDataLocked(dataDir, mapId, &loadMapError);
        if (!cached || !cached->mesh)
            return loadMapError;

        if (!loadNeighborTilesLocked(*cached, dataDir, mapId, startTileX, startTileY))
            return -3;
        if (!loadNeighborTilesLocked(*cached, dataDir, mapId, targetTileX, targetTileY))
            return -4;
        if (outTimings)
            outTimings->lock_and_tile_load_nanos = elapsedNanos(lockAndTileLoadStart);

        const auto queryAllocInitStart = SteadyClock::now();
        std::unique_ptr<dtNavMeshQuery, decltype(&dtFreeNavMeshQuery)> query(dtAllocNavMeshQuery(), dtFreeNavMeshQuery);
        if (!query)
            return -5;
        if (dtStatusFailed(query->init(cached->mesh, 2048)))
        {
            return -6;
        }
        if (outTimings)
            outTimings->query_alloc_init_nanos = elapsedNanos(queryAllocInitStart);

        dtQueryFilter filter;
        filter.setIncludeFlags(includeFlags);
        filter.setExcludeFlags(excludeFlags);

        const float startPoint[3] = { startY, startZ, startX };
        const float targetPoint[3] = { targetY, targetZ, targetX };
        float nearestStart[3] = { 0.0f, 0.0f, 0.0f };
        float nearestTarget[3] = { 0.0f, 0.0f, 0.0f };
        dtPolyRef startRef = 0;
        dtPolyRef targetRef = 0;

        const auto nearestPolyStart = SteadyClock::now();
        if (!findNearestPolyWithCmangosBounds(query.get(), startPoint, filter, &startRef, nearestStart))
        {
            return 0;
        }
        if (!findNearestPolyWithCmangosBounds(query.get(), targetPoint, filter, &targetRef, nearestTarget))
        {
            return 0;
        }
        if (outTimings)
            outTimings->find_nearest_poly_nanos = elapsedNanos(nearestPolyStart);

        int pathStatus = PATHFIND_NORMAL;
        if (dtVdist(startPoint, nearestStart) > 7.0f || dtVdist(targetPoint, nearestTarget) > 7.0f)
            pathStatus = PATHFIND_INCOMPLETE;

        dtPolyRef polys[MAX_PATH_POLYS];
        int polyCount = 0;
        const auto findPathStart = SteadyClock::now();
        if (dtStatusFailed(query->findPath(startRef, targetRef, nearestStart, nearestTarget, &filter, polys, &polyCount, MAX_PATH_POLYS)) || polyCount <= 0)
        {
            return 0;
        }
        if (polys[polyCount - 1] != targetRef)
            pathStatus = PATHFIND_INCOMPLETE;
        if (outTimings)
            outTimings->find_path_nanos = elapsedNanos(findPathStart);

        const int smoothLimit = maxPoints < MAX_SMOOTH_POINTS ? maxPoints : MAX_SMOOTH_POINTS;
        std::vector<float> smooth(smoothLimit * VERTEX_SIZE);
        int smoothCount = 0;
        const auto findSmoothPathStart = SteadyClock::now();
        if (dtStatusFailed(findSmoothPath(cached->mesh, query.get(), filter, nearestStart, nearestTarget, polys, polyCount, smooth.data(), &smoothCount, smoothLimit)) || smoothCount < 2)
        {
            return 0;
        }
        if (outTimings)
            outTimings->find_smooth_path_nanos = elapsedNanos(findSmoothPathStart);

        for (int i = 0; i < smoothCount; ++i)
        {
            const int offset = i * VERTEX_SIZE;
            outPoints[i].x = smooth[offset + 2];
            outPoints[i].y = smooth[offset];
            outPoints[i].z = smooth[offset + 1];
        }

        if (outPathStatus)
            *outPathStatus = pathStatus;
        return smoothCount;
    }
    catch (...)
    {
        return -100;
    }
}

int wow_mmap_find_random_path(
    const char* dataDir,
    unsigned int mapId,
    unsigned int startTileX,
    unsigned int startTileY,
    float centerX,
    float centerY,
    float centerZ,
    float startX,
    float startY,
    float startZ,
    float radius,
    float angleSeed,
    float rangeSeed,
    unsigned short includeFlags,
    unsigned short excludeFlags,
    WowMmapPathPoint* outPoints,
    int maxPoints,
    int* outPathStatus,
    WowMmapCallTimings* outTimings) noexcept
{
    try
    {
        if (outTimings)
            std::memset(outTimings, 0, sizeof(*outTimings));
        if (outPathStatus)
            *outPathStatus = PATHFIND_NOPATH;
        if (!dataDir || !outPoints || maxPoints < 2 || maxPoints > MAX_SMOOTH_POINTS)
            return -1;
        if (startTileX > 63 || startTileY > 63)
            return -7;
        if (!std::isfinite(centerX) || !std::isfinite(centerY) || !std::isfinite(centerZ) ||
            !std::isfinite(startX) || !std::isfinite(startY) || !std::isfinite(startZ) ||
            !std::isfinite(radius) || !std::isfinite(angleSeed) || !std::isfinite(rangeSeed) ||
            radius <= 0.0f)
            return -8;

        const float angle = clampUnit(angleSeed) * 2.0f * PI;
        const float range = clampUnit(rangeSeed) * radius;
        const float targetX = centerX + std::cos(angle) * range;
        const float targetY = centerY + std::sin(angle) * range;
        const float targetZ = centerZ;

        unsigned int targetTileX = 0;
        unsigned int targetTileY = 0;
        if (!tileForPosition(targetX, targetY, targetTileX, targetTileY))
            return -9;

        const auto lockAndTileLoadStart = SteadyClock::now();
        std::lock_guard<std::mutex> lock(g_mapsMutex);
        int loadMapError = -2;
        CachedMap* cached = loadMapDataLocked(dataDir, mapId, &loadMapError);
        if (!cached || !cached->mesh)
            return loadMapError;

        if (!loadNeighborTilesLocked(*cached, dataDir, mapId, startTileX, startTileY))
            return -3;
        if (!loadNeighborTilesLocked(*cached, dataDir, mapId, targetTileX, targetTileY))
            return -4;
        if (outTimings)
            outTimings->lock_and_tile_load_nanos = elapsedNanos(lockAndTileLoadStart);

        const auto queryAllocInitStart = SteadyClock::now();
        std::unique_ptr<dtNavMeshQuery, decltype(&dtFreeNavMeshQuery)> query(dtAllocNavMeshQuery(), dtFreeNavMeshQuery);
        if (!query)
            return -5;
        if (dtStatusFailed(query->init(cached->mesh, 2048)))
            return -6;
        if (outTimings)
            outTimings->query_alloc_init_nanos = elapsedNanos(queryAllocInitStart);

        dtQueryFilter filter;
        filter.setIncludeFlags(includeFlags);
        filter.setExcludeFlags(excludeFlags);

        const float startPoint[3] = { startY, startZ, startX };
        float targetPoint[3] = { targetY, targetZ, targetX };
        float nearestStart[3] = { 0.0f, 0.0f, 0.0f };
        float nearestTarget[3] = { 0.0f, 0.0f, 0.0f };
        dtPolyRef startRef = 0;
        dtPolyRef targetRef = 0;

        const auto nearestPolyStart = SteadyClock::now();
        if (!findNearestPolyWithCmangosBounds(query.get(), startPoint, filter, &startRef, nearestStart))
            return 0;
        if (!findNearestPolyWithCmangosBounds(query.get(), targetPoint, filter, &targetRef, nearestTarget))
            return 0;
        if (dtStatusFailed(query->getPolyHeight(targetRef, nearestTarget, &nearestTarget[1])))
            return 0;
        dtVcopy(targetPoint, nearestTarget);
        if (outTimings)
            outTimings->find_nearest_poly_nanos = elapsedNanos(nearestPolyStart);

        int pathStatus = PATHFIND_NORMAL;
        if (dtVdist(startPoint, nearestStart) > 7.0f || dtVdist(targetPoint, nearestTarget) > 7.0f)
            pathStatus = PATHFIND_INCOMPLETE;

        dtPolyRef polys[MAX_PATH_POLYS];
        int polyCount = 0;
        const auto findPathStart = SteadyClock::now();
        if (dtStatusFailed(query->findPath(startRef, targetRef, nearestStart, targetPoint, &filter, polys, &polyCount, MAX_PATH_POLYS)) || polyCount <= 0)
            return 0;
        if (polys[polyCount - 1] != targetRef)
            pathStatus = PATHFIND_INCOMPLETE;
        if (outTimings)
            outTimings->find_path_nanos = elapsedNanos(findPathStart);

        const int smoothLimit = maxPoints < MAX_SMOOTH_POINTS ? maxPoints : MAX_SMOOTH_POINTS;
        std::vector<float> smooth(smoothLimit * VERTEX_SIZE);
        int smoothCount = 0;
        const auto findSmoothPathStart = SteadyClock::now();
        if (dtStatusFailed(findSmoothPath(cached->mesh, query.get(), filter, nearestStart, targetPoint, polys, polyCount, smooth.data(), &smoothCount, smoothLimit)) || smoothCount < 2)
            return 0;
        if (outTimings)
            outTimings->find_smooth_path_nanos = elapsedNanos(findSmoothPathStart);

        for (int i = 0; i < smoothCount; ++i)
        {
            const int offset = i * VERTEX_SIZE;
            outPoints[i].x = smooth[offset + 2];
            outPoints[i].y = smooth[offset];
            outPoints[i].z = smooth[offset + 1];
        }

        if (outPathStatus)
            *outPathStatus = pathStatus;
        return smoothCount;
    }
    catch (...)
    {
        return -100;
    }
}
}
