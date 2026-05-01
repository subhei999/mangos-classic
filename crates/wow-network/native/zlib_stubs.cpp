#include <zlib.h>

extern "C"
{
int uncompress(Bytef*, uLongf*, const Bytef*, uLong)
{
    return Z_DATA_ERROR;
}

int compress2(Bytef*, uLongf*, const Bytef*, uLong, int)
{
    return Z_DATA_ERROR;
}

uLong crc32(uLong crc, const Bytef*, uInt)
{
    return crc;
}
}
