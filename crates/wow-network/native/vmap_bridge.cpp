#include "vmap_bridge.h"

std::mutex& wow_vmap_bridge_mutex()
{
    static std::mutex mutex;
    return mutex;
}
