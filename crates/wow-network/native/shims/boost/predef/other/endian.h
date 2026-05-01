#pragma once

#if defined(_WIN32) || defined(__LITTLE_ENDIAN__) || defined(__x86_64__) || defined(__i386__)
#define BOOST_ENDIAN_LITTLE_BYTE 1
#define BOOST_ENDIAN_LITTLE_WORD 0
#define BOOST_ENDIAN_BIG_BYTE 0
#define BOOST_ENDIAN_BIG_WORD 0
#else
#define BOOST_ENDIAN_LITTLE_BYTE 0
#define BOOST_ENDIAN_LITTLE_WORD 0
#define BOOST_ENDIAN_BIG_BYTE 1
#define BOOST_ENDIAN_BIG_WORD 0
#endif
