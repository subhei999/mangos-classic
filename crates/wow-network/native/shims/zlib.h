#pragma once

#define Z_OK 0
#define Z_DATA_ERROR (-3)
#define Z_NULL 0

typedef unsigned char Bytef;
typedef unsigned int uInt;
typedef unsigned long uLong;
typedef unsigned long uLongf;

extern "C" {
int uncompress(Bytef* dest, uLongf* destLen, const Bytef* source, uLong sourceLen);
int compress2(Bytef* dest, uLongf* destLen, const Bytef* source, uLong sourceLen, int level);
uLong crc32(uLong crc, const Bytef* buf, uInt len);
}
