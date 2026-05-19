use anyhow::{ensure, Context};
use bytes::BytesMut;
use sha1::{Digest, Sha1};
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use sqlx::FromRow;
use std::collections::HashSet;
use std::io::{ErrorKind, Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use wow_common::guid::{HighGuid, ObjectGuid};
use wow_common::position::WorldPosition;
use wow_crypto::HeaderCrypto;
use wow_proto::{
    AuthCommand, LogonChallengeRequest, LogonChallengeResponse, LogonProofRequest,
    LogonProofResponse,
};
use wow_srp::client::{SrpClient, SrpClientChallenge};
use wow_srp::normalized_string::NormalizedString;
use wow_srp::server::SrpVerifier;
use wow_srp::PublicKey;

const BUILD_1121: u16 = 5875;
const CLIENT_SEED: u32 = 0x1234_5678;

const CMSG_CHAR_ENUM: u32 = 0x0037;
const CMSG_PLAYER_LOGIN: u32 = 0x003D;
const CMSG_LOGOUT_REQUEST: u32 = 0x004B;
const MSG_MOVE_START_FORWARD: u32 = 0x00B5;
const MSG_MOVE_STOP: u32 = 0x00B7;
const MSG_MOVE_JUMP: u32 = 0x00BB;
const MSG_MOVE_FALL_LAND: u32 = 0x00C9;
const MSG_MOVE_SET_FACING: u32 = 0x00DA;
const MSG_MOVE_HEARTBEAT: u32 = 0x00EE;
const CMSG_AUTH_SESSION: u32 = 0x01ED;

const SMSG_CHAR_ENUM: u32 = 0x003B;
const SMSG_LOGOUT_COMPLETE: u32 = 0x004D;
const SMSG_UPDATE_OBJECT: u32 = 0x00A9;
const SMSG_AUTH_CHALLENGE: u32 = 0x01EC;
const SMSG_AUTH_RESPONSE: u32 = 0x01EE;

const AUTH_OK: u8 = 0x0C;

const MOVEFLAG_FORWARD: u32 = 0x0000_0001;
const MOVEFLAG_JUMPING: u32 = 0x0000_2000;

const DEFAULT_LOGIN_DATABASE_URL: &str = "mysql://mangos:mangos@127.0.0.1:3307/realmd";
const DEFAULT_CHARACTER_DATABASE_URL: &str = "mysql://mangos:mangos@127.0.0.1:3307/characters";
const DEFAULT_WORLD_DATABASE_URL: &str = "mysql://mangos:mangos@127.0.0.1:3307/mangos";
const DEFAULT_AUTH_ADDR: &str = "127.0.0.1:13724";
const DEFAULT_WORLD_ADDR: &str = "127.0.0.1:18085";

const DEFAULT_ACCOUNT_PREFIX: &str = "THINLD";
const DEFAULT_CHARACTER_PREFIX: &str = "Load";
const DEFAULT_PASSWORD: &str = "THINPASS";

const DEFAULT_CENTER_X: f32 = -8949.0;
const DEFAULT_CENTER_Y: f32 = -132.0;
const DEFAULT_CENTER_Z: f32 = 83.5;
const DEFAULT_RADIUS: f32 = 150.0;
const DEFAULT_MAP_ID: u32 = 0;
const MAP_SIZE_YARDS: f32 = 34133.333;
const MAX_NUMBER_OF_GRIDS: u32 = 64;
const GRID_SIZE_YARDS: f32 = 533.333_3;

const DEFAULT_MOVE_RADIUS: f32 = 6.0;
const DEFAULT_CLIENT_COUNT: usize = 500;
const DEFAULT_HOLD_SECONDS: u64 = 60;
const DEFAULT_LOGIN_BOOTSTRAP_TIMEOUT_SECS: u64 = 15;
const DEFAULT_MOVE_INTERVAL_MS: u64 = 500;
const DEFAULT_MOVE_PHASE_JITTER_MS: u64 = 0;
const DEFAULT_LOGIN_STAGGER_MS: u64 = 25;
const DEFAULT_DRAIN_TIMEOUT_MS: u64 = 5;
const DEFAULT_MAX_ATTEMPTS: u32 = 3;
const DEFAULT_LOGIN_READY_TIMEOUT_SECS: u64 = 30;
const CLIENT_THREAD_STACK_SIZE_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone)]
struct Config {
    client_count: usize,
    hold_seconds: u64,
    login_bootstrap_timeout_secs: u64,
    login_ready_timeout_secs: u64,
    move_interval_ms: u64,
    move_phase_jitter_ms: u64,
    login_stagger_ms: u64,
    drain_timeout_ms: u64,
    max_attempts: u32,
    account_prefix: String,
    character_prefix: String,
    password: String,
    auth_addr: String,
    world_addr: String,
    login_database_url: String,
    character_database_url: String,
    world_database_url: String,
    map_id: u32,
    spawn_mode: SpawnMode,
    center_x: f32,
    center_y: f32,
    center_z: f32,
    radius: f32,
    move_radius: f32,
    race: u8,
    class: u8,
    gender: u8,
    seed_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpawnMode {
    LocalRadius,
    CreatureGridScatter,
}

impl SpawnMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::LocalRadius => "local_radius",
            Self::CreatureGridScatter => "creature_grid_scatter",
        }
    }
}

impl std::str::FromStr for SpawnMode {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "local_radius" => Ok(Self::LocalRadius),
            "creature_grid_scatter" => Ok(Self::CreatureGridScatter),
            _ => Err("expected local_radius or creature_grid_scatter"),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            client_count: DEFAULT_CLIENT_COUNT,
            hold_seconds: DEFAULT_HOLD_SECONDS,
            login_bootstrap_timeout_secs: DEFAULT_LOGIN_BOOTSTRAP_TIMEOUT_SECS,
            login_ready_timeout_secs: DEFAULT_LOGIN_READY_TIMEOUT_SECS,
            move_interval_ms: DEFAULT_MOVE_INTERVAL_MS,
            move_phase_jitter_ms: DEFAULT_MOVE_PHASE_JITTER_MS,
            login_stagger_ms: DEFAULT_LOGIN_STAGGER_MS,
            drain_timeout_ms: DEFAULT_DRAIN_TIMEOUT_MS,
            max_attempts: DEFAULT_MAX_ATTEMPTS,
            account_prefix: DEFAULT_ACCOUNT_PREFIX.to_string(),
            character_prefix: DEFAULT_CHARACTER_PREFIX.to_string(),
            password: DEFAULT_PASSWORD.to_string(),
            auth_addr: DEFAULT_AUTH_ADDR.to_string(),
            world_addr: DEFAULT_WORLD_ADDR.to_string(),
            login_database_url: DEFAULT_LOGIN_DATABASE_URL.to_string(),
            character_database_url: DEFAULT_CHARACTER_DATABASE_URL.to_string(),
            world_database_url: DEFAULT_WORLD_DATABASE_URL.to_string(),
            map_id: DEFAULT_MAP_ID,
            spawn_mode: SpawnMode::LocalRadius,
            center_x: DEFAULT_CENTER_X,
            center_y: DEFAULT_CENTER_Y,
            center_z: DEFAULT_CENTER_Z,
            radius: DEFAULT_RADIUS,
            move_radius: DEFAULT_MOVE_RADIUS,
            race: 1,
            class: 1,
            gender: 0,
            seed_only: false,
        }
    }
}

#[derive(Debug, Clone)]
struct ClientSpec {
    index: usize,
    username: String,
    password: String,
    character_name: String,
    character_guid: u32,
    spawn_position: WorldPosition,
}

#[derive(Debug, Clone, Copy, FromRow)]
struct CreatureSpawnAnchor {
    guid: u32,
    position_x: f32,
    position_y: f32,
    position_z: f32,
    orientation: f32,
}

#[derive(Debug)]
struct EnumCharacter {
    guid: u32,
    name: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
struct JumpInfo {
    z_speed: f32,
    cos_angle: f32,
    sin_angle: f32,
    xy_speed: f32,
}

#[derive(Debug, Clone)]
struct MovementPacket {
    opcode: u32,
    flags: u32,
    client_time: u32,
    position: WorldPosition,
    fall_time: u32,
    jump: JumpInfo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MovementPhase {
    Idle { remaining_steps: u32 },
    Moving { remaining_steps: u32 },
    Landing { remaining_steps: u32 },
}

#[derive(Debug, Clone)]
struct MovementActor {
    spec: ClientSpec,
    move_radius: f32,
    move_interval_ms: u64,
    orbit_step: u32,
    client_time: u32,
    position: WorldPosition,
    phase: MovementPhase,
    script_step: u32,
}

#[derive(Debug, Default)]
struct ClientRunResult {
    movements_sent: u32,
    packets_drained: u32,
}

#[derive(Debug)]
struct MovementStartGate {
    target: usize,
    state: Mutex<MovementStartGateState>,
    condvar: Condvar,
}

#[derive(Debug, Default)]
struct MovementStartGateState {
    ready: usize,
    open: bool,
    aborted: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = parse_args()?;
    ensure!(config.client_count > 0, "client count must be positive");
    ensure!(
        config.move_interval_ms > 0,
        "move interval must be greater than zero"
    );
    ensure!(config.radius.is_finite() && config.radius >= 0.0);
    ensure!(config.move_radius.is_finite() && config.move_radius >= 0.0);

    let login_pool = connect_pool(&config.login_database_url, 8).await?;
    let character_pool = connect_pool(&config.character_database_url, 8).await?;
    let world_pool = connect_pool(&config.world_database_url, 4).await?;

    let specs = seed_accounts_and_characters(&config, &login_pool, &character_pool, &world_pool)
        .await
        .context("seed thin-client accounts and characters")?;

    println!(
        "Seeded {} dedicated load-test accounts and characters with prefix {} / {}",
        specs.len(),
        config.account_prefix,
        config.character_prefix
    );

    if config.seed_only {
        return Ok(());
    }

    let started_at = Instant::now();
    let mut handles = Vec::with_capacity(specs.len());
    let movement_start_gate = Arc::new(MovementStartGate::new(specs.len()));
    for spec in specs {
        let auth_addr = config.auth_addr.clone();
        let world_addr = config.world_addr.clone();
        let move_interval = Duration::from_millis(config.move_interval_ms);
        let move_phase_jitter = Duration::from_millis(config.move_phase_jitter_ms);
        let stagger =
            Duration::from_millis(config.login_stagger_ms.saturating_mul(spec.index as u64));
        let hold_duration = Duration::from_secs(config.hold_seconds);
        let login_bootstrap_timeout = Duration::from_secs(config.login_bootstrap_timeout_secs);
        let login_ready_timeout = Duration::from_secs(config.login_ready_timeout_secs);
        let drain_timeout = Duration::from_millis(config.drain_timeout_ms);
        let move_radius = config.move_radius;
        let max_attempts = config.max_attempts.max(1);
        let movement_start_gate = Arc::clone(&movement_start_gate);
        let username = spec.username.clone();
        let thread_name = format!("thin-client-{:04}", spec.index + 1);
        let handle = thread::Builder::new()
            // The load harness does shallow blocking I/O work. A smaller stack
            // avoids unstable virtual-memory pressure when spinning up
            // thousands of client threads on Windows.
            .stack_size(CLIENT_THREAD_STACK_SIZE_BYTES)
            .name(thread_name)
            .spawn(move || {
                thread::sleep(stagger);
                run_client_with_retries(
                    &spec,
                    &auth_addr,
                    &world_addr,
                    hold_duration,
                    login_bootstrap_timeout,
                    login_ready_timeout,
                    move_interval,
                    move_phase_jitter,
                    drain_timeout,
                    move_radius,
                    max_attempts,
                    movement_start_gate,
                )
            })
            .with_context(|| format!("spawn thin-client thread {username}"))?;
        handles.push(handle);
    }

    let mut total_movements = 0u64;
    let mut total_packets_drained = 0u64;
    let mut failures = Vec::new();
    for handle in handles {
        match handle.join() {
            Ok(Ok(result)) => {
                total_movements += u64::from(result.movements_sent);
                total_packets_drained += u64::from(result.packets_drained);
            }
            Ok(Err(error)) => failures.push(error.to_string()),
            Err(_) => failures.push("client thread panicked".to_string()),
        }
    }

    println!(
        "world-load-test finished in {:.2}s: clients={}, failures={}, movements_sent={}, packets_drained={}",
        started_at.elapsed().as_secs_f32(),
        config.client_count,
        failures.len(),
        total_movements,
        total_packets_drained
    );

    if failures.is_empty() {
        return Ok(());
    }

    for failure in failures.iter().take(8) {
        eprintln!("client failure: {failure}");
    }
    anyhow::bail!("{} client(s) failed during world-load-test", failures.len());
}

async fn connect_pool(url: &str, max_connections: u32) -> anyhow::Result<MySqlPool> {
    MySqlPoolOptions::new()
        .max_connections(max_connections)
        .connect(url)
        .await
        .with_context(|| format!("connect to mysql pool {url}"))
}

async fn seed_accounts_and_characters(
    config: &Config,
    login_pool: &MySqlPool,
    character_pool: &MySqlPool,
    world_pool: &MySqlPool,
) -> anyhow::Result<Vec<ClientSpec>> {
    let mut specs = Vec::with_capacity(config.client_count);
    let spawn_positions = seed_spawn_positions(config, world_pool)
        .await
        .context("prepare thin-client spawn positions")?;
    for (index, spawn_position) in spawn_positions
        .into_iter()
        .enumerate()
        .take(config.client_count)
    {
        let username = format!("{}{:04}", config.account_prefix, index + 1);
        let account_id = seed_account(login_pool, &username, &config.password).await?;
        cleanup_account(login_pool, character_pool, account_id).await?;

        let character_name = format!(
            "{}{}",
            config.character_prefix,
            alphabetic_suffix(index as u32)
        );
        let created = wow_db::create_character(
            character_pool,
            world_pool,
            wow_db::NewCharacter {
                account_id,
                name: character_name.clone(),
                race: config.race,
                class: config.class,
                gender: config.gender,
                skin: 0,
                face: 0,
                hair_style: 0,
                hair_color: 0,
                facial_hair: 0,
            },
        )
        .await?;
        wow_db::update_character_position(character_pool, account_id, created.guid, spawn_position)
            .await?;
        wow_db::refresh_realm_character_count(login_pool, character_pool, account_id, 1).await?;

        specs.push(ClientSpec {
            index,
            username,
            password: config.password.clone(),
            character_name,
            character_guid: created.guid,
            spawn_position,
        });
    }
    Ok(specs)
}

async fn seed_spawn_positions(
    config: &Config,
    world_pool: &MySqlPool,
) -> anyhow::Result<Vec<WorldPosition>> {
    match config.spawn_mode {
        SpawnMode::LocalRadius => Ok((0..config.client_count)
            .map(|index| seeded_spawn_position(config, index as u32))
            .collect()),
        SpawnMode::CreatureGridScatter => creature_grid_scatter_spawn_positions(config, world_pool)
            .await
            .context("load sparse creature-grid scatter positions"),
    }
}

async fn seed_account(
    login_pool: &MySqlPool,
    username: &str,
    password: &str,
) -> anyhow::Result<u32> {
    let verifier = SrpVerifier::from_username_and_password(
        NormalizedString::new(username)?,
        NormalizedString::new(password)?,
    );

    sqlx::query(
        "INSERT INTO account (username, gmlevel, sessionkey, v, s, email, locked, expansion, locale, os) \
         VALUES (?, 0, '', ?, ?, '', 0, 0, '', 'Win') \
         ON DUPLICATE KEY UPDATE sessionkey = '', v = VALUES(v), s = VALUES(s), locked = 0, os = 'Win'",
    )
    .bind(username)
    .bind(bytes_to_hex(verifier.password_verifier()))
    .bind(bytes_to_hex(verifier.salt()))
    .execute(login_pool)
    .await
    .with_context(|| format!("seed account {username}"))?;

    let account_id = sqlx::query_scalar("SELECT id FROM account WHERE username = ?")
        .bind(username)
        .fetch_one(login_pool)
        .await?;
    Ok(account_id)
}

async fn cleanup_account(
    login_pool: &MySqlPool,
    character_pool: &MySqlPool,
    account_id: u32,
) -> anyhow::Result<()> {
    let characters = wow_db::get_character_enum_entries(character_pool, account_id).await?;
    for character in characters {
        wow_db::delete_character(character_pool, account_id, character.guid).await?;
    }
    wow_db::refresh_realm_character_count(login_pool, character_pool, account_id, 1).await?;
    Ok(())
}

fn seeded_spawn_position(config: &Config, index: u32) -> WorldPosition {
    if config.radius <= f32::EPSILON {
        return WorldPosition::new(
            config.map_id,
            config.center_x,
            config.center_y,
            config.center_z,
            0.0,
        );
    }

    let ratio = ((index as f32) + 0.5) / (config.client_count as f32);
    let distance = ratio.sqrt() * config.radius;
    let angle = index as f32 * 2.399_963_1;
    WorldPosition::new(
        config.map_id,
        config.center_x + distance * angle.cos(),
        config.center_y + distance * angle.sin(),
        config.center_z,
        angle,
    )
}

async fn creature_grid_scatter_spawn_positions(
    config: &Config,
    world_pool: &MySqlPool,
) -> anyhow::Result<Vec<WorldPosition>> {
    let anchors = sqlx::query_as::<_, CreatureSpawnAnchor>(
        "SELECT CAST(guid AS UNSIGNED) AS guid, \
                CAST(position_x AS DOUBLE) AS position_x, \
                CAST(position_y AS DOUBLE) AS position_y, \
                CAST(position_z AS DOUBLE) AS position_z, \
                CAST(orientation AS DOUBLE) AS orientation \
         FROM creature \
         WHERE map = ? \
         ORDER BY guid ASC",
    )
    .bind(config.map_id)
    .fetch_all(world_pool)
    .await
    .with_context(|| format!("load creature spawn anchors for map {}", config.map_id))?;

    build_creature_grid_scatter_positions(config.map_id, config.client_count, &anchors)
}

fn build_creature_grid_scatter_positions(
    map_id: u32,
    client_count: usize,
    anchors: &[CreatureSpawnAnchor],
) -> anyhow::Result<Vec<WorldPosition>> {
    let mut shuffled = anchors.to_vec();
    shuffled.sort_by_key(|anchor| scatter_shuffle_key(u64::from(anchor.guid)));

    let mut seen_grids = HashSet::new();
    let mut ordered = Vec::with_capacity(shuffled.len());
    let mut leftovers = Vec::new();
    for anchor in shuffled {
        let grid = grid_coord_for_world_axes(anchor.position_x, anchor.position_y);
        if seen_grids.insert(grid) {
            ordered.push(anchor);
        } else {
            leftovers.push(anchor);
        }
    }
    ordered.extend(leftovers);

    ensure!(
        ordered.len() >= client_count,
        "not enough map {} creature spawn anchors for {} clients",
        map_id,
        client_count
    );

    Ok(ordered
        .into_iter()
        .take(client_count)
        .map(|anchor| {
            WorldPosition::new(
                map_id,
                anchor.position_x,
                anchor.position_y,
                anchor.position_z,
                anchor.orientation,
            )
        })
        .collect())
}

fn grid_coord_for_world_axes(x: f32, y: f32) -> (u32, u32) {
    let half = MAP_SIZE_YARDS / 2.0;
    let grid_x = ((half - y) / GRID_SIZE_YARDS)
        .floor()
        .clamp(0.0, (MAX_NUMBER_OF_GRIDS - 1) as f32) as u32;
    let grid_y = ((half - x) / GRID_SIZE_YARDS)
        .floor()
        .clamp(0.0, (MAX_NUMBER_OF_GRIDS - 1) as f32) as u32;
    (grid_x, grid_y)
}

fn scatter_shuffle_key(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[allow(clippy::too_many_arguments)]
fn run_client_with_retries(
    spec: &ClientSpec,
    auth_addr: &str,
    world_addr: &str,
    hold_duration: Duration,
    login_bootstrap_timeout: Duration,
    login_ready_timeout: Duration,
    move_interval: Duration,
    move_phase_jitter: Duration,
    drain_timeout: Duration,
    move_radius: f32,
    max_attempts: u32,
    movement_start_gate: Arc<MovementStartGate>,
) -> anyhow::Result<ClientRunResult> {
    let mut last_error = None;
    for attempt in 1..=max_attempts.max(1) {
        match run_client(
            spec,
            auth_addr,
            world_addr,
            hold_duration,
            login_bootstrap_timeout,
            login_ready_timeout,
            move_interval,
            move_phase_jitter,
            drain_timeout,
            move_radius,
            movement_start_gate.as_ref(),
        ) {
            Ok(result) => return Ok(result),
            Err(error) => {
                last_error = Some(error);
                if attempt < max_attempts {
                    thread::sleep(Duration::from_millis(250 * attempt as u64));
                }
            }
        }
    }

    movement_start_gate.abort();
    Err(last_error
        .unwrap_or_else(|| anyhow::anyhow!("client run failed without an error"))
        .context(format!(
            "client {} exhausted {} attempt(s)",
            spec.username,
            max_attempts.max(1)
        )))
}

#[allow(clippy::too_many_arguments)]
fn run_client(
    spec: &ClientSpec,
    auth_addr: &str,
    world_addr: &str,
    hold_duration: Duration,
    login_bootstrap_timeout: Duration,
    login_ready_timeout: Duration,
    move_interval: Duration,
    move_phase_jitter: Duration,
    drain_timeout: Duration,
    move_radius: f32,
    movement_start_gate: &MovementStartGate,
) -> anyhow::Result<ClientRunResult> {
    let srp_client = complete_auth_flow(auth_addr, &spec.username, &spec.password)
        .with_context(|| format!("auth flow for {}", spec.username))?;
    let mut world = WorldClient::connect(world_addr, &spec.username, srp_client.session_key())
        .with_context(|| format!("world connect for {}", spec.username))?;

    let characters = world.char_enum()?;
    let selected = characters
        .into_iter()
        .find(|character| {
            character.guid == spec.character_guid || character.name == spec.character_name
        })
        .with_context(|| format!("character enum missing {}", spec.character_name))?;
    world.login_character(selected.guid, login_bootstrap_timeout)?;
    movement_start_gate
        .wait_until_open(login_ready_timeout)
        .with_context(|| format!("movement start gate for {}", spec.username))?;
    let movement_phase_jitter = movement_phase_jitter_for_index(spec.index, move_phase_jitter);
    if !movement_phase_jitter.is_zero() {
        thread::sleep(movement_phase_jitter);
    }

    let run_started_at = Instant::now();
    let mut result = ClientRunResult::default();
    let mut movement = MovementActor::new(spec, move_radius, move_interval);
    while run_started_at.elapsed() < hold_duration {
        if let Some(packet) = movement.next_packet() {
            world.send_movement_packet(&packet)?;
            result.movements_sent = result.movements_sent.saturating_add(1);
        }
        result.packets_drained = result
            .packets_drained
            .saturating_add(world.drain_pending_packets(drain_timeout)? as u32);
        thread::sleep(move_interval);
    }

    world.logout()?;
    Ok(result)
}

fn movement_position(spec: &ClientSpec, move_radius: f32, step: u32) -> WorldPosition {
    if move_radius <= f32::EPSILON {
        return spec.spawn_position;
    }
    let angle = (step as f32 * 0.45) + (spec.index as f32 * 0.173);
    let distance = move_radius * (0.7 + (spec.index % 5) as f32 * 0.1);
    WorldPosition::new(
        spec.spawn_position.map_id,
        spec.spawn_position.x + distance * angle.cos(),
        spec.spawn_position.y + distance * angle.sin(),
        spec.spawn_position.z,
        angle,
    )
}

impl MovementActor {
    fn new(spec: &ClientSpec, move_radius: f32, move_interval: Duration) -> Self {
        Self {
            spec: spec.clone(),
            move_radius,
            move_interval_ms: move_interval.as_millis().max(1) as u64,
            orbit_step: 0,
            client_time: 1_000,
            position: spec.spawn_position,
            phase: MovementPhase::Idle {
                remaining_steps: idle_span_for_index(spec.index, 0),
            },
            script_step: 0,
        }
    }

    fn next_packet(&mut self) -> Option<MovementPacket> {
        self.client_time = self.client_time.wrapping_add(self.move_interval_ms as u32);
        self.script_step = self.script_step.wrapping_add(1);

        match self.phase {
            MovementPhase::Idle {
                mut remaining_steps,
            } => {
                if remaining_steps == 0 {
                    let next_position = self.advance_position();
                    let heading = heading_between(self.position, next_position)
                        .unwrap_or(self.position.orientation);
                    self.position = with_orientation(next_position, heading);
                    self.phase = MovementPhase::Moving {
                        remaining_steps: moving_span_for_index(self.spec.index, self.script_step),
                    };
                    Some(self.packet(
                        MSG_MOVE_START_FORWARD,
                        MOVEFLAG_FORWARD,
                        self.position,
                        0,
                        JumpInfo::default(),
                    ))
                } else {
                    remaining_steps -= 1;
                    self.phase = MovementPhase::Idle { remaining_steps };
                    if should_adjust_facing(self.spec.index, self.script_step, remaining_steps) {
                        let next_orientation = idle_facing(
                            self.spec.spawn_position.orientation,
                            self.spec.index,
                            self.script_step,
                        );
                        self.position.orientation = next_orientation;
                        Some(self.packet(
                            MSG_MOVE_SET_FACING,
                            0,
                            self.position,
                            0,
                            JumpInfo::default(),
                        ))
                    } else {
                        None
                    }
                }
            }
            MovementPhase::Moving {
                mut remaining_steps,
            } => {
                if should_jump_on_step(self.spec.index, self.script_step, remaining_steps) {
                    let next_position = self.advance_position();
                    let heading = heading_between(self.position, next_position)
                        .unwrap_or(self.position.orientation);
                    self.position = with_orientation(next_position, heading);
                    self.phase = MovementPhase::Landing {
                        remaining_steps: remaining_steps.saturating_sub(1),
                    };
                    let jump = jump_for_orientation(heading);
                    let fall_time = self.move_interval_ms as u32;
                    Some(self.packet(
                        MSG_MOVE_JUMP,
                        MOVEFLAG_FORWARD | MOVEFLAG_JUMPING,
                        self.position,
                        fall_time,
                        jump,
                    ))
                } else {
                    let next_position = self.advance_position();
                    let heading = heading_between(self.position, next_position)
                        .unwrap_or(self.position.orientation);
                    self.position = with_orientation(next_position, heading);
                    if remaining_steps <= 1 {
                        self.phase = MovementPhase::Idle {
                            remaining_steps: idle_span_for_index(self.spec.index, self.script_step),
                        };
                        Some(self.packet(MSG_MOVE_STOP, 0, self.position, 0, JumpInfo::default()))
                    } else {
                        remaining_steps -= 1;
                        self.phase = MovementPhase::Moving { remaining_steps };
                        Some(self.packet(
                            MSG_MOVE_HEARTBEAT,
                            MOVEFLAG_FORWARD,
                            self.position,
                            0,
                            JumpInfo::default(),
                        ))
                    }
                }
            }
            MovementPhase::Landing { remaining_steps } => {
                let next_position = self.advance_position();
                let heading = heading_between(self.position, next_position)
                    .unwrap_or(self.position.orientation);
                self.position = with_orientation(next_position, heading);
                self.phase = if remaining_steps == 0 {
                    MovementPhase::Idle {
                        remaining_steps: idle_span_for_index(self.spec.index, self.script_step),
                    }
                } else {
                    MovementPhase::Moving { remaining_steps }
                };
                Some(self.packet(
                    MSG_MOVE_FALL_LAND,
                    MOVEFLAG_FORWARD,
                    self.position,
                    (self.move_interval_ms.saturating_mul(2)) as u32,
                    JumpInfo::default(),
                ))
            }
        }
    }

    fn advance_position(&mut self) -> WorldPosition {
        self.orbit_step = self.orbit_step.wrapping_add(1);
        movement_position(&self.spec, self.move_radius, self.orbit_step)
    }

    fn packet(
        &self,
        opcode: u32,
        flags: u32,
        position: WorldPosition,
        fall_time: u32,
        jump: JumpInfo,
    ) -> MovementPacket {
        MovementPacket {
            opcode,
            flags,
            client_time: self.client_time,
            position,
            fall_time,
            jump,
        }
    }
}

impl MovementStartGate {
    fn new(target: usize) -> Self {
        Self {
            target,
            state: Mutex::new(MovementStartGateState::default()),
            condvar: Condvar::new(),
        }
    }

    fn wait_until_open(&self, timeout: Duration) -> anyhow::Result<()> {
        if self.target <= 1 {
            return Ok(());
        }

        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("movement start gate mutex poisoned"))?;

        if state.aborted {
            anyhow::bail!("movement start gate aborted before client became ready");
        }

        state.ready = state.ready.saturating_add(1);
        if state.ready >= self.target {
            state.open = true;
            self.condvar.notify_all();
            return Ok(());
        }

        let (state, wait_result) = self
            .condvar
            .wait_timeout_while(state, timeout, |state| !state.open && !state.aborted)
            .map_err(|_| anyhow::anyhow!("movement start gate condvar poisoned"))?;

        if state.aborted {
            anyhow::bail!("movement start gate aborted while waiting for ready clients");
        }

        ensure!(
            state.open || !wait_result.timed_out(),
            "movement start gate timed out after {:?} with {}/{} clients ready",
            timeout,
            state.ready,
            self.target
        );

        Ok(())
    }

    fn abort(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.aborted = true;
            self.condvar.notify_all();
        }
    }
}

fn idle_span_for_index(index: usize, step: u32) -> u32 {
    2 + stable_u32(index, step, 0) % 5
}

fn moving_span_for_index(index: usize, step: u32) -> u32 {
    8 + stable_u32(index, step, 1) % 15
}

fn should_adjust_facing(index: usize, step: u32, remaining_steps: u32) -> bool {
    remaining_steps > 0 && stable_u32(index, step, 2).is_multiple_of(3)
}

fn should_jump_on_step(index: usize, step: u32, remaining_steps: u32) -> bool {
    remaining_steps > 3 && stable_u32(index, step, 3).is_multiple_of(11)
}

fn idle_facing(base_orientation: f32, index: usize, step: u32) -> f32 {
    let offset = ((stable_u32(index, step, 4) % 7) as f32 - 3.0) * 0.35;
    normalize_orientation(base_orientation + offset)
}

fn movement_phase_jitter_for_index(index: usize, max_jitter: Duration) -> Duration {
    let max_jitter_ms = max_jitter.as_millis() as u64;
    if max_jitter_ms == 0 {
        return Duration::ZERO;
    }
    Duration::from_millis((stable_u32(index, 0, 5) as u64) % (max_jitter_ms + 1))
}

fn jump_for_orientation(orientation: f32) -> JumpInfo {
    JumpInfo {
        z_speed: 7.95,
        cos_angle: orientation.cos(),
        sin_angle: orientation.sin(),
        xy_speed: 4.25,
    }
}

fn stable_u32(index: usize, step: u32, salt: u32) -> u32 {
    let mut value = (index as u32)
        .wrapping_mul(1_103_515_245)
        .wrapping_add(step.wrapping_mul(12_345))
        .wrapping_add(salt.wrapping_mul(97_531));
    value ^= value >> 16;
    value = value.wrapping_mul(2_246_822_519);
    value ^ (value >> 13)
}

fn heading_between(from: WorldPosition, to: WorldPosition) -> Option<f32> {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    if dx.abs() <= f32::EPSILON && dy.abs() <= f32::EPSILON {
        None
    } else {
        Some(normalize_orientation(dy.atan2(dx)))
    }
}

fn normalize_orientation(orientation: f32) -> f32 {
    orientation.rem_euclid(std::f32::consts::TAU)
}

fn with_orientation(position: WorldPosition, orientation: f32) -> WorldPosition {
    WorldPosition::new(
        position.map_id,
        position.x,
        position.y,
        position.z,
        normalize_orientation(orientation),
    )
}

fn complete_auth_flow(
    auth_addr: &str,
    username: &str,
    password: &str,
) -> anyhow::Result<SrpClient> {
    let mut stream = connect_blocking(auth_addr)?;
    let (challenge, client) = perform_challenge(&mut stream, username, password)?;
    ensure!(
        challenge.error == 0,
        "auth challenge failed with {} for {username}",
        challenge.error
    );

    let proof = send_proof(&mut stream, &client)?;
    ensure!(proof.cmd == AuthCommand::LogonProof);
    ensure!(
        proof.error == 0,
        "auth proof failed with {} for {username}",
        proof.error
    );
    Ok(client.verify_server_proof(proof.m2)?)
}

fn perform_challenge(
    stream: &mut TcpStream,
    username: &str,
    password: &str,
) -> anyhow::Result<(LogonChallengeResponse, SrpClientChallenge)> {
    stream.write_all(&logon_challenge_request(username))?;

    let challenge_bytes = read_exact_vec(stream, LogonChallengeResponse::SIZE)?;
    let challenge = LogonChallengeResponse::read(&mut &challenge_bytes[..])?;
    ensure!(challenge.cmd == AuthCommand::LogonChallenge);
    ensure!(challenge.g_len == 1, "unexpected generator length");
    ensure!(challenge.n_len == 32, "unexpected safe-prime length");

    let client = SrpClientChallenge::new(
        NormalizedString::new(username)?,
        NormalizedString::new(password)?,
        challenge.g,
        challenge.n,
        PublicKey::from_le_bytes(challenge.server_public)?,
        challenge.salt,
    );

    Ok((challenge, client))
}

fn send_proof(
    stream: &mut TcpStream,
    client: &SrpClientChallenge,
) -> anyhow::Result<LogonProofResponse> {
    let proof_request = LogonProofRequest {
        cmd: AuthCommand::LogonProof,
        client_public: *client.client_public_key(),
        m1: *client.client_proof(),
        crc_hash: [0; 20],
        num_keys: 0,
        security_flags: 0,
    };
    let mut proof_bytes = BytesMut::new();
    proof_request.write(&mut proof_bytes);
    stream.write_all(&proof_bytes)?;

    let response = read_exact_vec(stream, LogonProofResponse::SIZE)?;
    Ok(LogonProofResponse::read(&mut &response[..])?)
}

struct WorldClient {
    stream: TcpStream,
    crypto: HeaderCrypto,
}

impl WorldClient {
    fn connect(world_addr: &str, username: &str, session_key: &[u8; 40]) -> anyhow::Result<Self> {
        let mut stream = connect_blocking(world_addr)?;
        let (opcode, body) = read_server_packet(&mut stream, None)?;
        ensure!(opcode == SMSG_AUTH_CHALLENGE, "expected auth challenge");
        ensure!(body.len() == 4, "world auth challenge body was malformed");
        let server_seed = u32::from_le_bytes(body.as_slice().try_into()?);

        let auth_body = auth_session_body(username, session_key, server_seed);
        write_client_packet(&mut stream, CMSG_AUTH_SESSION, &auth_body, None)?;

        let mut crypto = HeaderCrypto::new(session_key);
        let (opcode, body) = read_server_packet(&mut stream, Some(&mut crypto))?;
        ensure!(opcode == SMSG_AUTH_RESPONSE, "expected SMSG_AUTH_RESPONSE");
        ensure!(
            body.first() == Some(&AUTH_OK),
            "world auth failed with body {:02X?}",
            body
        );

        Ok(Self { stream, crypto })
    }

    fn char_enum(&mut self) -> anyhow::Result<Vec<EnumCharacter>> {
        write_client_packet(
            &mut self.stream,
            CMSG_CHAR_ENUM,
            &[],
            Some(&mut self.crypto),
        )?;
        let (opcode, body) = read_server_packet(&mut self.stream, Some(&mut self.crypto))?;
        ensure!(opcode == SMSG_CHAR_ENUM, "expected SMSG_CHAR_ENUM");
        parse_char_enum(&body)
    }

    fn login_character(&mut self, guid: u32, bootstrap_timeout: Duration) -> anyhow::Result<()> {
        let guid = ObjectGuid::new(HighGuid::Player, 0, guid);
        write_client_packet(
            &mut self.stream,
            CMSG_PLAYER_LOGIN,
            &guid.raw().to_le_bytes(),
            Some(&mut self.crypto),
        )?;
        self.drain_login_bootstrap(bootstrap_timeout)?;
        Ok(())
    }

    fn drain_login_bootstrap(&mut self, bootstrap_timeout: Duration) -> anyhow::Result<()> {
        const LOGIN_BOOTSTRAP_IDLE_TIMEOUT: Duration = Duration::from_millis(500);
        const LOGIN_BOOTSTRAP_POST_UPDATE_DRAIN: Duration = Duration::from_secs(1);

        let started = Instant::now();
        self.stream
            .set_read_timeout(Some(LOGIN_BOOTSTRAP_IDLE_TIMEOUT))?;
        let mut saw_update = false;
        loop {
            if saw_update && started.elapsed() >= LOGIN_BOOTSTRAP_POST_UPDATE_DRAIN {
                self.stream.set_read_timeout(Some(Duration::from_secs(5)))?;
                return Ok(());
            }
            if started.elapsed() >= bootstrap_timeout {
                self.stream.set_read_timeout(Some(Duration::from_secs(5)))?;
                ensure!(
                    saw_update,
                    "login bootstrap exceeded {:?} before receiving SMSG_UPDATE_OBJECT",
                    bootstrap_timeout
                );
                return Ok(());
            }
            match read_server_packet(&mut self.stream, Some(&mut self.crypto)) {
                Ok((SMSG_UPDATE_OBJECT, _)) => saw_update = true,
                Ok(_) => {}
                Err(error) => {
                    if let Some(io_error) = error.downcast_ref::<std::io::Error>() {
                        ensure!(
                            matches!(io_error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut),
                            "login bootstrap drain failed with unexpected IO error: {io_error}"
                        );
                        if saw_update {
                            self.stream.set_read_timeout(Some(Duration::from_secs(5)))?;
                            return Ok(());
                        }
                        continue;
                    }
                    self.stream.set_read_timeout(Some(Duration::from_secs(5)))?;
                    return Err(error);
                }
            }
        }
    }

    fn send_movement_packet(&mut self, packet: &MovementPacket) -> anyhow::Result<()> {
        write_client_packet(
            &mut self.stream,
            packet.opcode,
            &movement_body(packet),
            Some(&mut self.crypto),
        )
    }

    fn drain_pending_packets(&mut self, timeout: Duration) -> anyhow::Result<usize> {
        self.stream.set_read_timeout(Some(timeout))?;
        let mut drained = 0usize;
        loop {
            match read_server_packet(&mut self.stream, Some(&mut self.crypto)) {
                Ok(_) => drained += 1,
                Err(error) => {
                    self.stream.set_read_timeout(Some(Duration::from_secs(5)))?;
                    if let Some(io_error) = error.downcast_ref::<std::io::Error>() {
                        if matches!(io_error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) {
                            return Ok(drained);
                        }
                    }
                    return Err(error);
                }
            }
        }
    }

    fn logout(&mut self) -> anyhow::Result<()> {
        write_client_packet(
            &mut self.stream,
            CMSG_LOGOUT_REQUEST,
            &[],
            Some(&mut self.crypto),
        )?;
        self.stream
            .set_read_timeout(Some(Duration::from_millis(500)))?;
        for _ in 0..32 {
            match read_server_packet(&mut self.stream, Some(&mut self.crypto)) {
                Ok((opcode, _)) if opcode == SMSG_LOGOUT_COMPLETE => {
                    self.stream.set_read_timeout(Some(Duration::from_secs(5)))?;
                    return Ok(());
                }
                Ok(_) => {}
                Err(error) => {
                    self.stream.set_read_timeout(Some(Duration::from_secs(5)))?;
                    if let Some(io_error) = error.downcast_ref::<std::io::Error>() {
                        if matches!(io_error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) {
                            return Ok(());
                        }
                    }
                    return Err(error);
                }
            }
        }
        self.stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        Ok(())
    }
}

fn parse_char_enum(body: &[u8]) -> anyhow::Result<Vec<EnumCharacter>> {
    ensure!(!body.is_empty(), "empty SMSG_CHAR_ENUM body");
    let count = body[0] as usize;
    let mut cursor = 1usize;
    let mut characters = Vec::with_capacity(count);

    for _ in 0..count {
        ensure_available(body, cursor + 8)?;
        let raw_guid = u64::from_le_bytes(body[cursor..cursor + 8].try_into()?);
        cursor += 8;

        let name_end = body[cursor..]
            .iter()
            .position(|b| *b == 0)
            .ok_or_else(|| anyhow::anyhow!("character enum name is not NUL-terminated"))?
            + cursor;
        let name = String::from_utf8(body[cursor..name_end].to_vec())?;
        cursor = name_end + 1;

        ensure_available(
            body,
            cursor + 3 + 5 + 1 + 4 + 4 + 12 + 4 + 4 + 1 + 12 + 20 * 5,
        )?;
        cursor += 3 + 5 + 1 + 4 + 4 + 12 + 4 + 4 + 1 + 12 + 20 * 5;

        characters.push(EnumCharacter {
            guid: ObjectGuid::from_raw(raw_guid).counter(),
            name,
        });
    }

    ensure!(
        cursor == body.len(),
        "SMSG_CHAR_ENUM had trailing bytes: parsed {cursor}, len {}",
        body.len()
    );
    Ok(characters)
}

fn auth_session_body(username: &str, session_key: &[u8; 40], server_seed: u32) -> Vec<u8> {
    let mut hasher = Sha1::new();
    hasher.update(username.as_bytes());
    hasher.update(0u32.to_le_bytes());
    hasher.update(CLIENT_SEED.to_le_bytes());
    hasher.update(server_seed.to_le_bytes());
    hasher.update(session_key);
    let digest: [u8; 20] = hasher.finalize().into();

    let mut body = Vec::new();
    body.extend_from_slice(&(BUILD_1121 as u32).to_le_bytes());
    body.extend_from_slice(&0u32.to_le_bytes());
    body.extend_from_slice(username.as_bytes());
    body.push(0);
    body.extend_from_slice(&CLIENT_SEED.to_le_bytes());
    body.extend_from_slice(&digest);
    body
}

fn write_client_packet(
    stream: &mut TcpStream,
    opcode: u32,
    body: &[u8],
    crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<()> {
    let size = (body.len() + 4) as u16;
    let mut header = [0u8; 6];
    header[0..2].copy_from_slice(&size.to_be_bytes());
    header[2..6].copy_from_slice(&opcode.to_le_bytes());
    if let Some(crypto) = crypto {
        crypto.encrypt(&mut header);
    }
    stream.write_all(&header)?;
    stream.write_all(body)?;
    Ok(())
}

fn read_server_packet(
    stream: &mut TcpStream,
    crypto: Option<&mut HeaderCrypto>,
) -> anyhow::Result<(u32, Vec<u8>)> {
    let mut header = [0u8; 4];
    stream.read_exact(&mut header)?;
    if let Some(crypto) = crypto {
        crypto.decrypt(&mut header);
    }

    let size = u16::from_be_bytes([header[0], header[1]]) as usize;
    let opcode = u16::from_le_bytes([header[2], header[3]]) as u32;
    ensure!(
        (2..=0x2800).contains(&size),
        "malformed server packet size {size}"
    );
    let body_len = size - 2;
    let body = read_exact_vec(stream, body_len)?;
    Ok((opcode, body))
}

fn movement_body(packet: &MovementPacket) -> Vec<u8> {
    let mut body = Vec::with_capacity(44);
    body.extend_from_slice(&packet.flags.to_le_bytes());
    body.extend_from_slice(&packet.client_time.to_le_bytes());
    body.extend_from_slice(&packet.position.x.to_le_bytes());
    body.extend_from_slice(&packet.position.y.to_le_bytes());
    body.extend_from_slice(&packet.position.z.to_le_bytes());
    body.extend_from_slice(&packet.position.orientation.to_le_bytes());
    body.extend_from_slice(&packet.fall_time.to_le_bytes());
    if packet.flags & MOVEFLAG_JUMPING != 0 {
        body.extend_from_slice(&packet.jump.z_speed.to_le_bytes());
        body.extend_from_slice(&packet.jump.cos_angle.to_le_bytes());
        body.extend_from_slice(&packet.jump.sin_angle.to_le_bytes());
        body.extend_from_slice(&packet.jump.xy_speed.to_le_bytes());
    }
    body
}

fn connect_blocking(addr: &str) -> anyhow::Result<TcpStream> {
    let stream = TcpStream::connect(addr).with_context(|| format!("connect to {addr}"))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    Ok(stream)
}

fn logon_challenge_request(username: &str) -> Vec<u8> {
    let request = LogonChallengeRequest {
        cmd: AuthCommand::LogonChallenge,
        error: 0,
        size: 30 + username.len() as u16,
        game_name: *b"WoW\0",
        version_major: 1,
        version_minor: 12,
        version_patch: 1,
        build: BUILD_1121,
        platform: *b"x86\0",
        os: *b"Win\0",
        country: *b"enUS",
        timezone_bias: 0,
        ip: [127, 0, 0, 1],
        account_name: username.to_string(),
    };

    let mut bytes = BytesMut::new();
    request.write(&mut bytes);
    bytes.to_vec()
}

fn read_exact_vec(stream: &mut TcpStream, len: usize) -> anyhow::Result<Vec<u8>> {
    let mut bytes = vec![0u8; len];
    stream.read_exact(&mut bytes)?;
    Ok(bytes)
}

fn ensure_available(body: &[u8], end: usize) -> anyhow::Result<()> {
    ensure!(
        end <= body.len(),
        "packet truncated: need {end} bytes, got {}",
        body.len()
    );
    Ok(())
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn alphabetic_suffix(mut index: u32) -> String {
    let mut suffix = Vec::new();
    loop {
        suffix.push((b'a' + (index % 26) as u8) as char);
        index /= 26;
        if index == 0 {
            break;
        }
        index -= 1;
    }
    suffix.into_iter().rev().collect()
}

fn parse_args() -> anyhow::Result<Config> {
    let mut config = Config::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--client-count" => {
                config.client_count = parse_value(&arg, args.next())?;
            }
            "--hold-seconds" => {
                config.hold_seconds = parse_value(&arg, args.next())?;
            }
            "--login-bootstrap-timeout-secs" => {
                config.login_bootstrap_timeout_secs = parse_value(&arg, args.next())?;
            }
            "--login-ready-timeout-secs" => {
                config.login_ready_timeout_secs = parse_value(&arg, args.next())?;
            }
            "--move-interval-ms" => {
                config.move_interval_ms = parse_value(&arg, args.next())?;
            }
            "--move-phase-jitter-ms" => {
                config.move_phase_jitter_ms = parse_value(&arg, args.next())?;
            }
            "--login-stagger-ms" => {
                config.login_stagger_ms = parse_value(&arg, args.next())?;
            }
            "--drain-timeout-ms" => {
                config.drain_timeout_ms = parse_value(&arg, args.next())?;
            }
            "--max-attempts" => {
                config.max_attempts = parse_value(&arg, args.next())?;
            }
            "--account-prefix" => {
                config.account_prefix = expect_string(&arg, args.next())?;
            }
            "--character-prefix" => {
                config.character_prefix = expect_string(&arg, args.next())?;
            }
            "--password" => {
                config.password = expect_string(&arg, args.next())?;
            }
            "--auth-addr" => {
                config.auth_addr = expect_string(&arg, args.next())?;
            }
            "--world-addr" => {
                config.world_addr = expect_string(&arg, args.next())?;
            }
            "--login-db-url" => {
                config.login_database_url = expect_string(&arg, args.next())?;
            }
            "--character-db-url" => {
                config.character_database_url = expect_string(&arg, args.next())?;
            }
            "--world-db-url" => {
                config.world_database_url = expect_string(&arg, args.next())?;
            }
            "--map-id" => {
                config.map_id = parse_value(&arg, args.next())?;
            }
            "--spawn-mode" => {
                config.spawn_mode = parse_value(&arg, args.next())?;
            }
            "--center-x" => {
                config.center_x = parse_value(&arg, args.next())?;
            }
            "--center-y" => {
                config.center_y = parse_value(&arg, args.next())?;
            }
            "--center-z" => {
                config.center_z = parse_value(&arg, args.next())?;
            }
            "--radius" => {
                config.radius = parse_value(&arg, args.next())?;
            }
            "--move-radius" => {
                config.move_radius = parse_value(&arg, args.next())?;
            }
            "--race" => {
                config.race = parse_value(&arg, args.next())?;
            }
            "--class" => {
                config.class = parse_value(&arg, args.next())?;
            }
            "--gender" => {
                config.gender = parse_value(&arg, args.next())?;
            }
            "--seed-only" => {
                config.seed_only = true;
            }
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            other => anyhow::bail!("unknown argument {other}"),
        }
    }
    Ok(config)
}

fn parse_value<T: std::str::FromStr>(flag: &str, value: Option<String>) -> anyhow::Result<T>
where
    <T as std::str::FromStr>::Err: std::fmt::Display,
{
    let value = expect_string(flag, value)?;
    value
        .parse::<T>()
        .map_err(|error| anyhow::anyhow!("{flag} parse error: {error}"))
}

fn expect_string(flag: &str, value: Option<String>) -> anyhow::Result<String> {
    value.ok_or_else(|| anyhow::anyhow!("{flag} requires a value"))
}

fn print_usage() {
    println!("world-load-test");
    println!("  --client-count <n>          Default: {DEFAULT_CLIENT_COUNT}");
    println!("  --hold-seconds <secs>       Default: {DEFAULT_HOLD_SECONDS}");
    println!(
        "  --login-bootstrap-timeout-secs <secs> Default: {DEFAULT_LOGIN_BOOTSTRAP_TIMEOUT_SECS}"
    );
    println!("  --login-ready-timeout-secs <secs> Default: {DEFAULT_LOGIN_READY_TIMEOUT_SECS}");
    println!("  --move-interval-ms <ms>     Default: {DEFAULT_MOVE_INTERVAL_MS}");
    println!("  --move-phase-jitter-ms <ms> Default: {DEFAULT_MOVE_PHASE_JITTER_MS}");
    println!("  --login-stagger-ms <ms>     Default: {DEFAULT_LOGIN_STAGGER_MS}");
    println!("  --drain-timeout-ms <ms>     Default: {DEFAULT_DRAIN_TIMEOUT_MS}");
    println!("  --max-attempts <n>          Default: {DEFAULT_MAX_ATTEMPTS}");
    println!("  --account-prefix <prefix>   Default: {DEFAULT_ACCOUNT_PREFIX}");
    println!("  --character-prefix <prefix> Default: {DEFAULT_CHARACTER_PREFIX}");
    println!("  --password <password>       Default: {DEFAULT_PASSWORD}");
    println!("  --auth-addr <host:port>     Default: {DEFAULT_AUTH_ADDR}");
    println!("  --world-addr <host:port>    Default: {DEFAULT_WORLD_ADDR}");
    println!("  --login-db-url <url>");
    println!("  --character-db-url <url>");
    println!("  --world-db-url <url>");
    println!("  --map-id <id>               Default: {DEFAULT_MAP_ID}");
    println!(
        "  --spawn-mode <mode>         Default: {} (local_radius | creature_grid_scatter)",
        SpawnMode::LocalRadius.as_str()
    );
    println!("  --center-x <x>              Default: {DEFAULT_CENTER_X}");
    println!("  --center-y <y>              Default: {DEFAULT_CENTER_Y}");
    println!("  --center-z <z>              Default: {DEFAULT_CENTER_Z}");
    println!("  --radius <yards>            Default: {DEFAULT_RADIUS}");
    println!("  --move-radius <yards>       Default: {DEFAULT_MOVE_RADIUS}");
    println!("  --race <id>                 Default: 1");
    println!("  --class <id>                Default: 1");
    println!("  --gender <id>               Default: 0");
    println!("  --seed-only                 Seed accounts and characters but do not log in");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_spec() -> ClientSpec {
        ClientSpec {
            index: 7,
            username: "THINLD0007".to_string(),
            password: "THINPASS".to_string(),
            character_name: "Loadg".to_string(),
            character_guid: 7,
            spawn_position: WorldPosition::new(0, -8949.0, -132.0, 83.5, 0.0),
        }
    }

    #[test]
    fn movement_actor_emits_realistic_opcode_mix() {
        let spec = test_spec();
        let mut actor = MovementActor::new(&spec, 6.0, Duration::from_millis(500));
        let mut saw_start = false;
        let mut saw_stop = false;
        let mut saw_facing = false;
        let mut saw_jump = false;
        let mut saw_land = false;

        for _ in 0..128 {
            if let Some(packet) = actor.next_packet() {
                saw_start |= packet.opcode == MSG_MOVE_START_FORWARD;
                saw_stop |= packet.opcode == MSG_MOVE_STOP;
                saw_facing |= packet.opcode == MSG_MOVE_SET_FACING;
                saw_jump |= packet.opcode == MSG_MOVE_JUMP;
                saw_land |= packet.opcode == MSG_MOVE_FALL_LAND;
            }
        }

        assert!(saw_start, "movement stream should start moving");
        assert!(saw_stop, "movement stream should stop between segments");
        assert!(
            saw_facing,
            "movement stream should include facing-only updates"
        );
        assert!(saw_jump, "movement stream should include jumps");
        assert!(saw_land, "movement stream should include landing packets");
    }

    #[test]
    fn movement_actor_keeps_clients_inside_move_radius_envelope() {
        let spec = test_spec();
        let mut actor = MovementActor::new(&spec, 6.0, Duration::from_millis(500));

        for _ in 0..256 {
            let _ = actor.next_packet();
            let dx = actor.position.x - spec.spawn_position.x;
            let dy = actor.position.y - spec.spawn_position.y;
            let distance = (dx * dx + dy * dy).sqrt();
            assert!(
                distance <= 6.2,
                "movement actor drifted too far from spawn: {distance}"
            );
        }
    }

    #[test]
    fn movement_phase_jitter_is_deterministic_and_bounded() {
        let max = Duration::from_millis(50);
        let first = movement_phase_jitter_for_index(7, max);
        let second = movement_phase_jitter_for_index(7, max);
        let other = movement_phase_jitter_for_index(8, max);

        assert_eq!(first, second);
        assert!(first <= max);
        assert!(other <= max);
        assert_ne!(first, other);
        assert_eq!(
            movement_phase_jitter_for_index(7, Duration::ZERO),
            Duration::ZERO
        );
    }

    #[test]
    fn jump_packets_include_jumping_flag_and_payload() {
        let spec = test_spec();
        let mut actor = MovementActor::new(&spec, 6.0, Duration::from_millis(500));

        let jump = (0..256)
            .filter_map(|_| actor.next_packet())
            .find(|packet| packet.opcode == MSG_MOVE_JUMP)
            .expect("actor should emit a jump packet");

        assert_ne!(jump.flags & MOVEFLAG_JUMPING, 0);
        assert!(jump.jump.z_speed > 0.0);
        assert!(jump.jump.xy_speed > 0.0);
        assert!(jump.fall_time > 0);
    }

    #[test]
    fn movement_start_gate_opens_once_all_clients_are_ready() {
        let gate = Arc::new(MovementStartGate::new(2));
        let gate_clone = Arc::clone(&gate);

        let worker = thread::spawn(move || gate_clone.wait_until_open(Duration::from_secs(1)));
        thread::sleep(Duration::from_millis(10));

        gate.wait_until_open(Duration::from_secs(1))
            .expect("main waiter should open gate");
        worker
            .join()
            .expect("worker should join")
            .expect("worker waiter should open gate");
    }

    #[test]
    fn movement_start_gate_reports_abort() {
        let gate = Arc::new(MovementStartGate::new(2));
        let gate_clone = Arc::clone(&gate);

        let worker = thread::spawn(move || gate_clone.wait_until_open(Duration::from_secs(1)));
        thread::sleep(Duration::from_millis(10));
        gate.abort();

        let error = worker
            .join()
            .expect("worker should join")
            .expect_err("worker should see abort");
        assert!(
            error.to_string().contains("aborted"),
            "unexpected error: {error:#}"
        );
    }

    #[test]
    fn creature_grid_scatter_prioritizes_unique_grids_before_reuse() {
        let anchors = vec![
            CreatureSpawnAnchor {
                guid: 1,
                position_x: -8949.0,
                position_y: -132.0,
                position_z: 83.5,
                orientation: 0.0,
            },
            CreatureSpawnAnchor {
                guid: 2,
                position_x: -8950.0,
                position_y: -130.0,
                position_z: 83.5,
                orientation: 0.0,
            },
            CreatureSpawnAnchor {
                guid: 3,
                position_x: -1000.0,
                position_y: 1000.0,
                position_z: 25.0,
                orientation: 1.0,
            },
            CreatureSpawnAnchor {
                guid: 4,
                position_x: 5000.0,
                position_y: -5000.0,
                position_z: 30.0,
                orientation: 2.0,
            },
        ];

        let positions =
            build_creature_grid_scatter_positions(0, 4, &anchors).expect("scatter positions");

        let first_three_grids = positions
            .iter()
            .take(3)
            .map(|position| grid_coord_for_world_axes(position.x, position.y))
            .collect::<HashSet<_>>();
        assert_eq!(
            first_three_grids.len(),
            3,
            "first pass should consume unique grids before reusing one"
        );
    }

    #[test]
    fn creature_grid_scatter_requires_enough_anchors() {
        let anchors = vec![CreatureSpawnAnchor {
            guid: 1,
            position_x: -8949.0,
            position_y: -132.0,
            position_z: 83.5,
            orientation: 0.0,
        }];

        let error = build_creature_grid_scatter_positions(0, 2, &anchors)
            .expect_err("insufficient anchors should fail");
        assert!(
            error
                .to_string()
                .contains("not enough map 0 creature spawn anchors"),
            "unexpected error: {error:#}"
        );
    }
}
