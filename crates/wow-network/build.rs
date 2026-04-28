fn main() {
    let detour_sources = [
        "../../dep/recastnavigation/Detour/Source/DetourAlloc.cpp",
        "../../dep/recastnavigation/Detour/Source/DetourAssert.cpp",
        "../../dep/recastnavigation/Detour/Source/DetourCommon.cpp",
        "../../dep/recastnavigation/Detour/Source/DetourNavMesh.cpp",
        "../../dep/recastnavigation/Detour/Source/DetourNavMeshBuilder.cpp",
        "../../dep/recastnavigation/Detour/Source/DetourNavMeshQuery.cpp",
        "../../dep/recastnavigation/Detour/Source/DetourNode.cpp",
        "native/mmap_path.cpp",
    ];

    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++17")
        .define("DT_POLYREF64", None)
        .include("../../dep/recastnavigation/Detour/Include")
        .warnings(false);

    for source in detour_sources {
        build.file(source);
        println!("cargo:rerun-if-changed={source}");
    }

    build.compile("wow_mmap_path");
}
