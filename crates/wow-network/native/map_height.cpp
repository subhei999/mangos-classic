#include "IVMapManager.h"
#include "VMapFactory.h"
#include "vmap_bridge.h"

#include <algorithm>
#include <array>
#include <cmath>
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
constexpr unsigned int MAX_NUMBER_OF_GRIDS = 64;
constexpr float SIZE_OF_GRIDS = 533.33333f;
constexpr float MAP_RESOLUTION = 128.0f;
constexpr float INVALID_HEIGHT = -100000.0f;
constexpr float INVALID_HEIGHT_VALUE = -200000.0f;
constexpr float DEFAULT_HEIGHT_SEARCH = 10.0f;

constexpr unsigned int MAP_AREA_NO_AREA = 0x0001;
constexpr unsigned int MAP_HEIGHT_NO_HEIGHT = 0x0001;
constexpr unsigned int MAP_HEIGHT_AS_INT16 = 0x0002;
constexpr unsigned int MAP_HEIGHT_AS_INT8 = 0x0004;

constexpr unsigned int fourcc(const char (&text)[5])
{
    return static_cast<unsigned int>(text[0]) |
           (static_cast<unsigned int>(text[1]) << 8) |
           (static_cast<unsigned int>(text[2]) << 16) |
           (static_cast<unsigned int>(text[3]) << 24);
}

constexpr unsigned int MAP_MAGIC = fourcc("MAPS");
constexpr unsigned int MAP_VERSION_MAGIC = fourcc("z1.4");
constexpr unsigned int MAP_AREA_MAGIC = fourcc("AREA");
constexpr unsigned int MAP_HEIGHT_MAGIC = fourcc("MHGT");

struct GridMapFileHeader
{
    unsigned int mapMagic;
    unsigned int versionMagic;
    unsigned int areaMapOffset;
    unsigned int areaMapSize;
    unsigned int heightMapOffset;
    unsigned int heightMapSize;
    unsigned int liquidMapOffset;
    unsigned int liquidMapSize;
    unsigned int holesOffset;
    unsigned int holesSize;
};

struct GridMapAreaHeader
{
    unsigned int fourcc;
    unsigned short flags;
    unsigned short gridArea;
};

struct GridMapHeightHeader
{
    unsigned int fourcc;
    unsigned int flags;
    float gridHeight;
    float gridMaxHeight;
};

const unsigned short HOLE_TABLE_H[4] = {0x1111, 0x2222, 0x4444, 0x8888};
const unsigned short HOLE_TABLE_V[4] = {0x000F, 0x00F0, 0x0F00, 0xF000};

enum class HeightStorage
{
    Flat,
    Float,
    UInt16,
    UInt8,
};

struct CachedGridMap
{
    unsigned int flags = 0;
    float gridHeight = INVALID_HEIGHT_VALUE;
    float gridIntHeightMultiplier = 0.0f;
    HeightStorage storage = HeightStorage::Flat;
    std::array<unsigned short, 16 * 16> holes{};
    std::vector<float> floatV9;
    std::vector<float> floatV8;
    std::vector<unsigned short> uint16V9;
    std::vector<unsigned short> uint16V8;
    std::vector<unsigned char> uint8V9;
    std::vector<unsigned char> uint8V8;

    bool isHole(int row, int col) const
    {
        const int cellRow = row / 8;
        const int cellCol = col / 8;
        const int holeRow = row % 8 / 2;
        const int holeCol = (col - (cellCol * 8)) / 2;
        const unsigned short hole = holes[cellRow * 16 + cellCol];
        return (hole & HOLE_TABLE_H[holeCol] & HOLE_TABLE_V[holeRow]) != 0;
    }

    float height(float x, float y) const
    {
        switch (storage)
        {
            case HeightStorage::Float:
                return heightFromFloat(x, y);
            case HeightStorage::UInt16:
                return heightFromUInt16(x, y);
            case HeightStorage::UInt8:
                return heightFromUInt8(x, y);
            case HeightStorage::Flat:
            default:
                return gridHeight;
        }
    }

    float heightFromFloat(float x, float y) const
    {
        if (floatV8.empty() || floatV9.empty())
            return INVALID_HEIGHT_VALUE;

        x = MAP_RESOLUTION * (32.0f - x / SIZE_OF_GRIDS);
        y = MAP_RESOLUTION * (32.0f - y / SIZE_OF_GRIDS);

        int xInt = static_cast<int>(x);
        int yInt = static_cast<int>(y);
        x -= xInt;
        y -= yInt;
        xInt &= 127;
        yInt &= 127;

        if (isHole(xInt, yInt))
            return INVALID_HEIGHT_VALUE;

        float a = 0.0f;
        float b = 0.0f;
        float c = 0.0f;
        if (x + y < 1.0f)
        {
            if (x > y)
            {
                const float h1 = floatV9[xInt * 129 + yInt];
                const float h2 = floatV9[(xInt + 1) * 129 + yInt];
                const float h5 = 2.0f * floatV8[xInt * 128 + yInt];
                a = h2 - h1;
                b = h5 - h1 - h2;
                c = h1;
            }
            else
            {
                const float h1 = floatV9[xInt * 129 + yInt];
                const float h3 = floatV9[xInt * 129 + yInt + 1];
                const float h5 = 2.0f * floatV8[xInt * 128 + yInt];
                a = h5 - h1 - h3;
                b = h3 - h1;
                c = h1;
            }
        }
        else
        {
            if (x > y)
            {
                const float h2 = floatV9[(xInt + 1) * 129 + yInt];
                const float h4 = floatV9[(xInt + 1) * 129 + yInt + 1];
                const float h5 = 2.0f * floatV8[xInt * 128 + yInt];
                a = h2 + h4 - h5;
                b = h4 - h2;
                c = h5 - h4;
            }
            else
            {
                const float h3 = floatV9[xInt * 129 + yInt + 1];
                const float h4 = floatV9[(xInt + 1) * 129 + yInt + 1];
                const float h5 = 2.0f * floatV8[xInt * 128 + yInt];
                a = h4 - h3;
                b = h3 + h4 - h5;
                c = h5 - h4;
            }
        }
        return a * x + b * y + c;
    }

    float heightFromUInt8(float x, float y) const
    {
        if (uint8V8.empty() || uint8V9.empty())
            return gridHeight;

        x = MAP_RESOLUTION * (32.0f - x / SIZE_OF_GRIDS);
        y = MAP_RESOLUTION * (32.0f - y / SIZE_OF_GRIDS);

        int xInt = static_cast<int>(x);
        int yInt = static_cast<int>(y);
        x -= xInt;
        y -= yInt;
        xInt &= 127;
        yInt &= 127;

        int a = 0;
        int b = 0;
        int c = 0;
        const unsigned char* h1Ptr = &uint8V9[xInt * 128 + xInt + yInt];
        if (x + y < 1.0f)
        {
            if (x > y)
            {
                const int h1 = h1Ptr[0];
                const int h2 = h1Ptr[129];
                const int h5 = 2 * uint8V8[xInt * 128 + yInt];
                a = h2 - h1;
                b = h5 - h1 - h2;
                c = h1;
            }
            else
            {
                const int h1 = h1Ptr[0];
                const int h3 = h1Ptr[1];
                const int h5 = 2 * uint8V8[xInt * 128 + yInt];
                a = h5 - h1 - h3;
                b = h3 - h1;
                c = h1;
            }
        }
        else
        {
            if (x > y)
            {
                const int h2 = h1Ptr[129];
                const int h4 = h1Ptr[130];
                const int h5 = 2 * uint8V8[xInt * 128 + yInt];
                a = h2 + h4 - h5;
                b = h4 - h2;
                c = h5 - h4;
            }
            else
            {
                const int h3 = h1Ptr[1];
                const int h4 = h1Ptr[130];
                const int h5 = 2 * uint8V8[xInt * 128 + yInt];
                a = h4 - h3;
                b = h3 + h4 - h5;
                c = h5 - h4;
            }
        }
        return (static_cast<float>(a) * x + static_cast<float>(b) * y + static_cast<float>(c)) *
                   gridIntHeightMultiplier +
               gridHeight;
    }

    float heightFromUInt16(float x, float y) const
    {
        if (uint16V8.empty() || uint16V9.empty())
            return gridHeight;

        x = MAP_RESOLUTION * (32.0f - x / SIZE_OF_GRIDS);
        y = MAP_RESOLUTION * (32.0f - y / SIZE_OF_GRIDS);

        int xInt = static_cast<int>(x);
        int yInt = static_cast<int>(y);
        x -= xInt;
        y -= yInt;
        xInt &= 127;
        yInt &= 127;

        int a = 0;
        int b = 0;
        int c = 0;
        const unsigned short* h1Ptr = &uint16V9[xInt * 128 + xInt + yInt];
        if (x + y < 1.0f)
        {
            if (x > y)
            {
                const int h1 = h1Ptr[0];
                const int h2 = h1Ptr[129];
                const int h5 = 2 * uint16V8[xInt * 128 + yInt];
                a = h2 - h1;
                b = h5 - h1 - h2;
                c = h1;
            }
            else
            {
                const int h1 = h1Ptr[0];
                const int h3 = h1Ptr[1];
                const int h5 = 2 * uint16V8[xInt * 128 + yInt];
                a = h5 - h1 - h3;
                b = h3 - h1;
                c = h1;
            }
        }
        else
        {
            if (x > y)
            {
                const int h2 = h1Ptr[129];
                const int h4 = h1Ptr[130];
                const int h5 = 2 * uint16V8[xInt * 128 + yInt];
                a = h2 + h4 - h5;
                b = h4 - h2;
                c = h5 - h4;
            }
            else
            {
                const int h3 = h1Ptr[1];
                const int h4 = h1Ptr[130];
                const int h5 = 2 * uint16V8[xInt * 128 + yInt];
                a = h4 - h3;
                b = h3 + h4 - h5;
                c = h5 - h4;
            }
        }
        return (static_cast<float>(a) * x + static_cast<float>(b) * y + static_cast<float>(c)) *
                   gridIntHeightMultiplier +
               gridHeight;
    }
};

std::mutex g_gridMapsMutex;
std::unordered_map<std::string, std::unique_ptr<CachedGridMap>> g_gridMaps;
std::unordered_set<std::string> g_missingGridMaps;

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

std::string mapFileName(const char* dataDir, unsigned int mapId, unsigned int tileX, unsigned int tileY)
{
    char child[40];
    std::snprintf(child, sizeof(child), "maps/%03u%02u%02u.map", mapId, tileX, tileY);
    return pathJoin(dataDir, child);
}

std::string gridKey(const char* dataDir, unsigned int mapId, unsigned int tileX, unsigned int tileY)
{
    return std::string(dataDir ? dataDir : "") + "|" + std::to_string(mapId) + "|" +
           std::to_string(tileX) + "|" + std::to_string(tileY);
}

bool tileIdIsValid(unsigned int tile)
{
    return tile < MAX_NUMBER_OF_GRIDS;
}

bool tileForPosition(float x, float y, unsigned int& tileX, unsigned int& tileY)
{
    if (!std::isfinite(x) || !std::isfinite(y))
        return false;
    const int gx = static_cast<int>(32.0f - x / SIZE_OF_GRIDS);
    const int gy = static_cast<int>(32.0f - y / SIZE_OF_GRIDS);
    if (gx < 0 || gy < 0 || gx >= static_cast<int>(MAX_NUMBER_OF_GRIDS) ||
        gy >= static_cast<int>(MAX_NUMBER_OF_GRIDS))
        return false;
    tileX = static_cast<unsigned int>(gx);
    tileY = static_cast<unsigned int>(gy);
    return true;
}

template <typename T>
bool readExact(FILE* file, T& value)
{
    return std::fread(&value, sizeof(T), 1, file) == 1;
}

template <typename T>
bool readVector(FILE* file, std::vector<T>& values, std::size_t count)
{
    values.resize(count);
    return count == 0 || std::fread(values.data(), sizeof(T), count, file) == count;
}

bool loadHeightData(FILE* file, CachedGridMap& map, unsigned int offset)
{
    if (std::fseek(file, static_cast<long>(offset), SEEK_SET) != 0)
        return false;

    GridMapHeightHeader header{};
    if (!readExact(file, header) || header.fourcc != MAP_HEIGHT_MAGIC)
        return false;

    map.gridHeight = header.gridHeight;
    if (header.flags & MAP_HEIGHT_NO_HEIGHT)
    {
        map.storage = HeightStorage::Flat;
        return true;
    }

    if (header.flags & MAP_HEIGHT_AS_INT16)
    {
        if (!readVector(file, map.uint16V9, 129 * 129) ||
            !readVector(file, map.uint16V8, 128 * 128))
            return false;
        map.gridIntHeightMultiplier = (header.gridMaxHeight - header.gridHeight) / 65535.0f;
        map.storage = HeightStorage::UInt16;
        return true;
    }

    if (header.flags & MAP_HEIGHT_AS_INT8)
    {
        if (!readVector(file, map.uint8V9, 129 * 129) ||
            !readVector(file, map.uint8V8, 128 * 128))
            return false;
        map.gridIntHeightMultiplier = (header.gridMaxHeight - header.gridHeight) / 255.0f;
        map.storage = HeightStorage::UInt8;
        return true;
    }

    if (!readVector(file, map.floatV9, 129 * 129) || !readVector(file, map.floatV8, 128 * 128))
        return false;
    map.storage = HeightStorage::Float;
    return true;
}

bool loadHolesData(FILE* file, CachedGridMap& map, unsigned int offset)
{
    if (std::fseek(file, static_cast<long>(offset), SEEK_SET) != 0)
        return false;
    return std::fread(map.holes.data(), sizeof(unsigned short), map.holes.size(), file) ==
           map.holes.size();
}

std::unique_ptr<CachedGridMap> loadGridMapFile(const char* dataDir, unsigned int mapId, unsigned int tileX, unsigned int tileY)
{
    const std::string fileName = mapFileName(dataDir, mapId, tileX, tileY);
    FILE* file = std::fopen(fileName.c_str(), "rb");
    if (!file)
        return nullptr;

    GridMapFileHeader header{};
    if (!readExact(file, header))
    {
        std::fclose(file);
        return nullptr;
    }
    if (header.mapMagic != MAP_MAGIC || header.versionMagic != MAP_VERSION_MAGIC)
    {
        std::fclose(file);
        return nullptr;
    }

    auto map = std::make_unique<CachedGridMap>();
    if (header.holesOffset && !loadHolesData(file, *map, header.holesOffset))
    {
        std::fclose(file);
        return nullptr;
    }
    if (header.heightMapOffset && !loadHeightData(file, *map, header.heightMapOffset))
    {
        std::fclose(file);
        return nullptr;
    }

    std::fclose(file);
    return map;
}

CachedGridMap* loadGridMapLocked(const char* dataDir, unsigned int mapId, unsigned int tileX, unsigned int tileY)
{
    const std::string key = gridKey(dataDir, mapId, tileX, tileY);
    auto existing = g_gridMaps.find(key);
    if (existing != g_gridMaps.end())
        return existing->second.get();
    if (g_missingGridMaps.find(key) != g_missingGridMaps.end())
        return nullptr;

    auto map = loadGridMapFile(dataDir, mapId, tileX, tileY);
    if (!map)
    {
        g_missingGridMaps.insert(key);
        return nullptr;
    }

    CachedGridMap* ptr = map.get();
    g_gridMaps.emplace(key, std::move(map));
    return ptr;
}

float mapHeight(const char* dataDir, unsigned int mapId, float x, float y)
{
    unsigned int tileX = 0;
    unsigned int tileY = 0;
    if (!tileForPosition(x, y, tileX, tileY))
        return INVALID_HEIGHT_VALUE;

    std::lock_guard<std::mutex> lock(g_gridMapsMutex);
    CachedGridMap* map = loadGridMapLocked(dataDir, mapId, tileX, tileY);
    if (!map)
        return INVALID_HEIGHT_VALUE;
    return map->height(x, y);
}

float vmapHeightLoaded(const char* dataDir, unsigned int mapId, unsigned int tileX, unsigned int tileY, float x, float y, float z, float search)
{
    VMAP::IVMapManager* manager = VMAP::VMapFactory::createOrGetVMapManager();
    if (!manager || !manager->isHeightCalcEnabled())
        return VMAP_INVALID_HEIGHT_VALUE;

    const std::string basePath = vmapBasePath(dataDir);
    const VMAP::VMAPLoadResult loadResult =
        manager->loadMap(basePath.c_str(), mapId, static_cast<int>(tileX), static_cast<int>(tileY));
    if (loadResult == VMAP::VMAP_LOAD_RESULT_ERROR)
        return VMAP_INVALID_HEIGHT_VALUE;
    return manager->getHeight(mapId, x, y, z, search);
}

float terrainHeightStatic(const char* dataDir, unsigned int mapId, unsigned int tileX, unsigned int tileY, float x, float y, float z, float maxSearchDist)
{
    float mapHeightValue = mapHeight(dataDir, mapId, x, y);
    float vmapHeightValue = VMAP_INVALID_HEIGHT_VALUE;

    VMAP::IVMapManager* manager = VMAP::VMapFactory::createOrGetVMapManager();
    if (manager && manager->isHeightCalcEnabled())
    {
        float z2 = z + 2.0f;
        if (mapHeightValue > INVALID_HEIGHT && z2 - mapHeightValue > maxSearchDist)
            maxSearchDist = z2 - mapHeightValue + 1.0f;

        vmapHeightValue = vmapHeightLoaded(dataDir, mapId, tileX, tileY, x, y, z2, maxSearchDist);
        if (vmapHeightValue <= INVALID_HEIGHT)
            vmapHeightValue = vmapHeightLoaded(dataDir, mapId, tileX, tileY, x, y, z2, 10000.0f);
        if (vmapHeightValue <= INVALID_HEIGHT && mapHeightValue > z2 &&
            std::fabs(z2 - mapHeightValue) > 30.0f)
            vmapHeightValue = vmapHeightLoaded(dataDir, mapId, tileX, tileY, x, y, z2, -maxSearchDist);
        if (vmapHeightValue <= INVALID_HEIGHT && mapHeightValue > INVALID_HEIGHT &&
            z2 < mapHeightValue)
            vmapHeightValue =
                vmapHeightLoaded(dataDir, mapId, tileX, tileY, x, y, mapHeightValue + 2.0f, DEFAULT_HEIGHT_SEARCH);
    }

    if (vmapHeightValue > INVALID_HEIGHT)
    {
        if (mapHeightValue > INVALID_HEIGHT)
        {
            if (z < mapHeightValue || vmapHeightValue > mapHeightValue)
                return vmapHeightValue;
            return mapHeightValue;
        }
        return vmapHeightValue;
    }

    return mapHeightValue;
}

bool terrainHeightInRange(const char* dataDir, unsigned int mapId, unsigned int tileX, unsigned int tileY, float x, float y, float& z, float maxSearchDist)
{
    float mapHeightValue = mapHeight(dataDir, mapId, x, y);
    float vmapHeightValue =
        vmapHeightLoaded(dataDir, mapId, tileX, tileY, x, y, z + 2.0f, maxSearchDist + 2.0f);

    const float diffMaps = std::fabs(std::fabs(z) - std::fabs(mapHeightValue));
    const float diffVmaps = std::fabs(std::fabs(z) - std::fabs(vmapHeightValue));
    float height = INVALID_HEIGHT_VALUE;
    if (diffVmaps < maxSearchDist)
    {
        if (diffMaps < maxSearchDist)
        {
            if (vmapHeightValue > mapHeightValue || std::fabs(mapHeightValue - z) > std::fabs(vmapHeightValue - z))
                height = vmapHeightValue;
            else
                height = mapHeightValue;
        }
        else
        {
            height = vmapHeightValue;
        }
    }
    else
    {
        if (diffMaps < maxSearchDist)
            height = mapHeightValue;
        else
            return false;
    }

    z = height;
    return height > INVALID_HEIGHT;
}
}

extern "C"
{
int wow_map_height_static(
    const char* dataDir,
    unsigned int mapId,
    unsigned int tileX,
    unsigned int tileY,
    float x,
    float y,
    float z,
    float maxSearchDist,
    float* outHeight) noexcept
{
    try
    {
        if (!dataDir || !outHeight)
            return -1;
        if (!tileIdIsValid(tileX) || !tileIdIsValid(tileY))
            return -2;
        if (!std::isfinite(x) || !std::isfinite(y) || !std::isfinite(z) ||
            !std::isfinite(maxSearchDist))
            return -3;

        std::lock_guard<std::mutex> lock(wow_vmap_bridge_mutex());
        const float height = terrainHeightStatic(dataDir, mapId, tileX, tileY, x, y, z, maxSearchDist);
        if (height <= INVALID_HEIGHT)
            return 0;
        *outHeight = height;
        return 1;
    }
    catch (...)
    {
        return -100;
    }
}

int wow_map_height_in_range(
    const char* dataDir,
    unsigned int mapId,
    unsigned int tileX,
    unsigned int tileY,
    float x,
    float y,
    float z,
    float maxSearchDist,
    float* outHeight) noexcept
{
    try
    {
        if (!dataDir || !outHeight)
            return -1;
        if (!tileIdIsValid(tileX) || !tileIdIsValid(tileY))
            return -2;
        if (!std::isfinite(x) || !std::isfinite(y) || !std::isfinite(z) ||
            !std::isfinite(maxSearchDist))
            return -3;

        std::lock_guard<std::mutex> lock(wow_vmap_bridge_mutex());
        float height = z;
        if (!terrainHeightInRange(dataDir, mapId, tileX, tileY, x, y, height, maxSearchDist))
            return 0;
        *outHeight = height;
        return 1;
    }
    catch (...)
    {
        return -100;
    }
}
}
