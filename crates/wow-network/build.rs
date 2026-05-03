fn main() {
    let sources = [
        "../../dep/recastnavigation/Detour/Source/DetourAlloc.cpp",
        "../../dep/recastnavigation/Detour/Source/DetourAssert.cpp",
        "../../dep/recastnavigation/Detour/Source/DetourCommon.cpp",
        "../../dep/recastnavigation/Detour/Source/DetourNavMesh.cpp",
        "../../dep/recastnavigation/Detour/Source/DetourNavMeshBuilder.cpp",
        "../../dep/recastnavigation/Detour/Source/DetourNavMeshQuery.cpp",
        "../../dep/recastnavigation/Detour/Source/DetourNode.cpp",
        "../../dep/g3dlite/AABox.cpp",
        "../../dep/g3dlite/Any.cpp",
        "../../dep/g3dlite/BinaryFormat.cpp",
        "../../dep/g3dlite/BinaryInput.cpp",
        "../../dep/g3dlite/BinaryOutput.cpp",
        "../../dep/g3dlite/Box.cpp",
        "../../dep/g3dlite/Capsule.cpp",
        "../../dep/g3dlite/CollisionDetection.cpp",
        "../../dep/g3dlite/CoordinateFrame.cpp",
        "../../dep/g3dlite/Crypto.cpp",
        "../../dep/g3dlite/Cylinder.cpp",
        "../../dep/g3dlite/FileSystem.cpp",
        "../../dep/g3dlite/Line.cpp",
        "../../dep/g3dlite/LineSegment.cpp",
        "../../dep/g3dlite/Log.cpp",
        "../../dep/g3dlite/Matrix3.cpp",
        "../../dep/g3dlite/Matrix4.cpp",
        "../../dep/g3dlite/MemoryManager.cpp",
        "../../dep/g3dlite/PhysicsFrame.cpp",
        "../../dep/g3dlite/Plane.cpp",
        "../../dep/g3dlite/Quat.cpp",
        "../../dep/g3dlite/Random.cpp",
        "../../dep/g3dlite/Ray.cpp",
        "../../dep/g3dlite/ReferenceCount.cpp",
        "../../dep/g3dlite/RegistryUtil.cpp",
        "../../dep/g3dlite/Sphere.cpp",
        "../../dep/g3dlite/System.cpp",
        "../../dep/g3dlite/TextInput.cpp",
        "../../dep/g3dlite/TextOutput.cpp",
        "../../dep/g3dlite/Triangle.cpp",
        "../../dep/g3dlite/UprightFrame.cpp",
        "../../dep/g3dlite/Vector2.cpp",
        "../../dep/g3dlite/Vector3.cpp",
        "../../dep/g3dlite/Vector4.cpp",
        "../../dep/g3dlite/debugAssert.cpp",
        "../../dep/g3dlite/fileutils.cpp",
        "../../dep/g3dlite/format.cpp",
        "../../dep/g3dlite/g3dmath.cpp",
        "../../dep/g3dlite/g3dfnmatch.cpp",
        "../../dep/g3dlite/prompt.cpp",
        "../../dep/g3dlite/stringutils.cpp",
        "../../dep/g3dlite/uint128.cpp",
        "../../src/game/vmap/BIH.cpp",
        "../../src/game/vmap/MapTree.cpp",
        "../../src/game/vmap/ModelInstance.cpp",
        "../../src/game/vmap/TileAssembler.cpp",
        "../../src/game/vmap/VMapFactory.cpp",
        "../../src/game/vmap/VMapManager2.cpp",
        "../../src/game/vmap/WorldModel.cpp",
        "native/vmap_bridge.cpp",
        "native/mmap_path.cpp",
        "native/vmap_los.cpp",
        "native/map_height.cpp",
        "native/zlib_stubs.cpp",
    ];

    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++17")
        .define("DT_POLYREF64", None)
        .define("NO_CORE_FUNCS", None)
        .include("../../dep/recastnavigation/Detour/Include")
        .include("native/shims")
        .include("../../dep/g3dlite")
        .include("../../src/shared")
        .include("../../src/game")
        .include("../../src/game/vmap")
        .flag_if_supported("/Gy")
        .warnings(false);

    for source in sources {
        build.file(source);
        println!("cargo:rerun-if-changed={source}");
    }

    build.compile("wow_native_world_data");
    println!("cargo:rustc-link-lib=user32");
}
