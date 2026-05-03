# Terrain Height And VMap Parity Plan

Focused plan for fixing vertical glitching on ramps, WMO floors, bridges,
caves, and other places where CMaNGOS does more than line-of-sight checks.

## Goal

Make Rust creature/world movement use CMaNGOS-shaped terrain height decisions:

- raw `.map` terrain height from extracted map tiles;
- static vmap/WMO height from extracted vmaps;
- dynamic object height later, when gameobject collision models are owned by
  `MapRuntime`;
- no invented z offsets or hardcoded gameplay corrections.

This is not an mmap replacement. Mmaps answer "where can I path?" while map/vmap
height answers "what z should this point stand on?"

## CMaNGOS Reference

Primary source paths:

- `src/game/Maps/Map.cpp`
  - `Map::GetHeight`
  - `Map::GetHeightInRange`
  - `Map::GetReachableRandomPointOnGround`
- `src/game/Maps/GridMap.cpp`
  - `TerrainInfo::GetHeightStatic`
  - `GridMap::getHeight`
- `src/game/Maps/GridDefines.h`
  - `INVALID_HEIGHT`
  - `INVALID_HEIGHT_VALUE`
  - `DEFAULT_HEIGHT_SEARCH`
  - `DEFAULT_WATER_SEARCH`
- `src/game/vmap/IVMapManager.h`
  - `VMAP_INVALID_HEIGHT`
  - `VMAP_INVALID_HEIGHT_VALUE`
  - `IVMapManager::getHeight`
- `src/game/Entities/Object.cpp`
  - `WorldObject::UpdateGroundPositionZ`
- `src/game/MotionGenerators/MotionMaster.cpp`
  - ground z correction before movement.

CMaNGOS behavior shape:

1. Load the relevant terrain grid and vmap tile.
2. Query raw `.map` height at `(x, y)`.
3. Query vmap height from `z + 2.0` within the configured search range.
4. If the first vmap search fails, retry the CMaNGOS fallback searches:
   broader downward search, upward search when terrain is far above, then search
   near terrain height.
5. Choose between terrain and vmap height using the same validity and "closest
   plausible surface" rules.
6. `Map::GetHeight` also considers dynamic object height. Rust should defer
   this until dynamic gameobject collision models exist rather than faking it.

## Current Implementation Status

Rust currently has:

- native Detour mmap pathing in `crates/wow-network/native/mmap_path.cpp`;
- native vmap LOS in `crates/wow-network/native/vmap_los.cpp`;
- native CMaNGOS-shaped static height sampling in
  `crates/wow-network/native/map_height.cpp`;
- shared vmap manager locking for LOS and height calls in
  `crates/wow-network/native/vmap_bridge.*`;
- `WorldGeometry`, owned by `MapRuntimeManager` and passed into each
  `MapRuntime`;
- creature random, waypoint, chase, and return-home motion construction that
  samples/corrects destination ground z before building the mmap path;
- no permissive generated movement when a configured data directory lacks
  usable mmaps.

Remaining gap:

- dynamic object height is not implemented yet;
- real-client ramp/WMO regression coordinates should be added as focused tests
  once observed;
- Detour path z is still preferred for path points after construction.

## Phase 1: Native Static Height Bridge

Branch idea: `codex/terrain-height-native-bridge`

Ownership:

- `crates/wow-network/native/*`
- `crates/wow-network/build.rs`
- focused Rust FFI wrapper module under `crates/wow-network/src/world`
- tests in `crates/wow-network/src/world/tests.rs`

Implement a native bridge that exposes CMaNGOS static terrain height:

- `wow_map_height_static(...)` matching `TerrainInfo::GetHeightStatic`.
- `wow_map_height_in_range(...)` matching `Map::GetHeightInRange` except dynamic
  tree height is omitted and clearly documented.
- Optionally `wow_vmap_height(...)` as a lower-level test/debug helper, but keep
  production Rust calling the CMaNGOS-shaped combined sampler.

The bridge should:

- load `.map` and `.vmtile` data from the same `C:\World of Warcraft Classic`
  data directory that `WorldDataFiles::inspect` already uses;
- use the same tile coordinate transform as current mmap/vmap code;
- return explicit status codes for unavailable map data, unavailable vmap data,
  invalid coordinates, and no height found;
- preserve CMaNGOS constants from source, not local guesses:
  `INVALID_HEIGHT = -100000.0`, `INVALID_HEIGHT_VALUE = -200000.0`,
  `DEFAULT_HEIGHT_SEARCH = 10.0`, `DEFAULT_WATER_SEARCH = 50.0`.

Proof:

- tests skip when local data is unavailable, matching existing native mmap/vmap
  tests;
- Northshire ground point returns a finite height near the known DB/player z;
- Teldrassil/Night Elf starter point returns a finite height near the known DB
  z;
- a WMO/ramp sample found from real-client observation returns vmap height when
  it differs from raw terrain.

Status: implemented for static `.map` + static vmap height. Dynamic object
height remains Phase 4.

## Phase 2: Map-Owned Geometry Service

Branch idea: `codex/mapruntime-terrain-height`

Ownership:

- `crates/wow-network/src/world/maps/*`
- `crates/wow-network/src/world/combat/motion.rs`
- focused tests.

Add a map-owned terrain/geometry service, not ad hoc calls from session code.

Suggested shape:

- `MapRuntimeManager` or shared world context owns an `Arc<WorldGeometry>`.
- `WorldGeometry` wraps `Arc<WorldDataFiles>` and native height/LOS/path access.
- `MapRuntime` receives the geometry service for movement decisions.
- Session state remains a viewer/input/output cache; it should not own terrain
  truth.

Public operations should mirror CMaNGOS concepts:

- `height_static(map_id, x, y, z, search) -> Option<f32>`
- `height_in_range(map_id, x, y, z, max_search) -> Option<f32>`
- `ground_position(position) -> Option<WorldPosition>`

Proof:

- no session-owned geometry decisions;
- map runtime tests prove height sampling is reused for random, waypoint, chase,
  and return-home logic;
- missing data behavior remains guarded: do not invent z; either keep DB z for
  existing spawns or fail to start generated motion where CMaNGOS would fail.

Status: implemented. `MapRuntimeManager` owns `Arc<WorldGeometry>`, and
session state remains a viewer/input/output cache.

## Phase 3: Apply Height To Creature Movement

Branch idea: `codex/creature-motion-ground-z`

Ownership:

- `crates/wow-network/src/world/combat/motion.rs`
- `crates/wow-network/src/world/maps/map/creature_motion.rs`
- packet movement tests.

Apply sampled ground z at motion construction boundaries:

- random destination generation should sample ground z for candidate `(x, y)`;
- waypoint path legs should validate/correct destination z when DB point is
  close enough, using `GetHeightInRange` semantics;
- chase stop/cut points should sample or preserve Detour-provided z rather than
  reusing stale start/target z;
- return-home should sample/validate home z before issuing movement;
- stop packets should use the authoritative runtime position, after the same
  height correction path.

Do not use a global "clamp every tick" first. CMaNGOS builds movement around
valid positions and then advances along splines. Per-tick z correction can hide
bad path construction and cause visible snapping.

Proof:

- focused ramp regression test from a real-client coordinate pair;
- random movement destination z changes on sloped terrain instead of preserving
  home z;
- chase/return path points keep finite, plausible z;
- packet builder still serializes CMaNGOS-shaped `SMSG_MONSTER_MOVE`.

Status: first static-height slice implemented. Add real-client ramp/WMO
coordinates as regression tests after playtest.

## Phase 4: Dynamic Gameobject Height

Branch idea: `codex/dynamic-vmap-height`

This should wait until static map/vmap height is solid.

CMaNGOS `Map::GetHeight` takes the max of static height and dynamic object
height. Rust should add this only when gameobject collision models are loaded
and owned by `MapRuntime`; until then, document that `Map::GetHeightStatic`
parity exists but dynamic tree parity is not complete.

Proof:

- dynamic object model inserted/removed with gameobject lifecycle;
- height changes only near spawned collision gameobjects;
- no global fake collision or hand-authored z offsets.

## Phase 5: Real-Client Playtest

Use the user's real-client testing as the final grader.

Required routes:

- Northshire abbey stairs/ramp-like terrain;
- Teldrassil starter ramps and WMO-adjacent floors;
- one cave/mineshaft or bridge location once convenient.

Pass criteria:

- hostile mobs in Teldrassil aggro and chase when the local mmap tile exists;
- if a configured map lacks mmap tiles, hostile mobs do not start generated
  aggro/chase/idle movement through straight-line fallback;
- right-click / auto-attack range no longer reports false out-of-range when
  x/y distance is valid and the mmap/vmap path check is clear;
- idle wandering on Northshire and Teldrassil slopes does not preserve a stale
  home z, visibly hover, sink, or snap;
- chase movement follows slope/ramp z smoothly, without vertical popping at
  the first movement packet;
- return-home movement ends on the visible ground and does not teleport
  vertically at arrival;
- WMO-adjacent floors and cave/ramp samples do not place creatures under the
  floor or above the player;
- no regression in Northshire kobold/wolf aggro, melee, leash, and return-home
  behavior.

## Open Risks

- The native bridge may need more CMaNGOS terrain code linked into
  `wow-network` than the LOS bridge did. Keep the first slice narrowly focused
  on static height and avoid dragging dynamic vmap ownership in early.
- Full world DB startup currently loads many static spawns; height sampling must
  be cached by tile/map and measured before broad use with thousands of active
  creatures.
- Mmaps already contain walkable navmesh z. For Detour paths, prefer the mmap
  path z unless CMaNGOS reference shows a later `GetHeightInRange` correction at
  that movement boundary.
