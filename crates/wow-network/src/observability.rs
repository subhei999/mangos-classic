use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tracing::{info, warn};

const MAP_TICK_BUCKETS_SECONDS: [f64; 10] = [
    0.001, 0.005, 0.010, 0.025, 0.050, 0.100, 0.250, 0.500, 1.000, 2.500,
];

static REGISTRY: OnceLock<MetricsRegistry> = OnceLock::new();

fn registry() -> &'static MetricsRegistry {
    REGISTRY.get_or_init(MetricsRegistry::default)
}

#[derive(Default)]
struct MetricsRegistry {
    world_sessions_connected: AtomicU64,
    world_sessions_registered_total: AtomicU64,
    world_sessions_unregistered_total: AtomicU64,
    map_ticks_total: AtomicU64,
    map_tick_over_budget_total: AtomicU64,
    map_tick_errors_total: AtomicU64,
    map_tick_duration: Histogram,
    map_tick_lag: Histogram,
    map_phase_duration: MapPhaseDurations,
    map_runtime_snapshots: Mutex<HashMap<(u32, u32), MapRuntimeSnapshot>>,
    world_packets_in: Mutex<HashMap<u32, u64>>,
    world_packets_out: Mutex<HashMap<u32, u64>>,
    world_unknown_opcodes: Mutex<HashMap<u32, u64>>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MapRuntimeSnapshot {
    pub map_id: u32,
    pub instance_id: u32,
    pub active_players: u64,
    pub active_creatures: u64,
    pub active_gameobjects: u64,
    pub loaded_grids: u64,
    pub loaded_creature_grids: u64,
    pub loaded_gameobject_grids: u64,
    pub loaded_player_corpse_grids: u64,
    pub active_creature_combats: u64,
    pub corpses: u64,
}

#[derive(Debug, Clone, Copy)]
pub enum MapTickPhase {
    IdleMotion,
    IdleMotionDispatch,
    PlayerRegen,
    PlayerRegenDispatch,
    AuraExpiration,
    AuraExpirationDispatch,
}

impl MapTickPhase {
    fn label(self) -> &'static str {
        match self {
            Self::IdleMotion => "idle_motion",
            Self::IdleMotionDispatch => "idle_motion_dispatch",
            Self::PlayerRegen => "player_regen",
            Self::PlayerRegenDispatch => "player_regen_dispatch",
            Self::AuraExpiration => "aura_expiration",
            Self::AuraExpirationDispatch => "aura_expiration_dispatch",
        }
    }
}

const MAP_TICK_PHASES: [MapTickPhase; 6] = [
    MapTickPhase::IdleMotion,
    MapTickPhase::IdleMotionDispatch,
    MapTickPhase::PlayerRegen,
    MapTickPhase::PlayerRegenDispatch,
    MapTickPhase::AuraExpiration,
    MapTickPhase::AuraExpirationDispatch,
];

#[derive(Default)]
struct MapPhaseDurations {
    idle_motion: Histogram,
    idle_motion_dispatch: Histogram,
    player_regen: Histogram,
    player_regen_dispatch: Histogram,
    aura_expiration: Histogram,
    aura_expiration_dispatch: Histogram,
}

impl MapPhaseDurations {
    fn get(&self, phase: MapTickPhase) -> &Histogram {
        match phase {
            MapTickPhase::IdleMotion => &self.idle_motion,
            MapTickPhase::IdleMotionDispatch => &self.idle_motion_dispatch,
            MapTickPhase::PlayerRegen => &self.player_regen,
            MapTickPhase::PlayerRegenDispatch => &self.player_regen_dispatch,
            MapTickPhase::AuraExpiration => &self.aura_expiration,
            MapTickPhase::AuraExpirationDispatch => &self.aura_expiration_dispatch,
        }
    }
}

#[derive(Default)]
struct Histogram {
    buckets: [AtomicU64; MAP_TICK_BUCKETS_SECONDS.len()],
    infinite_bucket: AtomicU64,
    count: AtomicU64,
    sum_micros: AtomicU64,
    latest_micros: AtomicU64,
    max_micros: AtomicU64,
}

impl Histogram {
    fn record(&self, value: Duration) {
        let micros = value.as_micros() as u64;
        let seconds = value.as_secs_f64();
        let bucket_index = MAP_TICK_BUCKETS_SECONDS
            .iter()
            .position(|bucket| seconds <= *bucket);
        if let Some(index) = bucket_index {
            self.buckets[index].fetch_add(1, Ordering::Relaxed);
        } else {
            self.infinite_bucket.fetch_add(1, Ordering::Relaxed);
        }
        self.count.fetch_add(1, Ordering::Relaxed);
        self.sum_micros.fetch_add(micros, Ordering::Relaxed);
        self.latest_micros.store(micros, Ordering::Relaxed);
        self.max_micros.fetch_max(micros, Ordering::Relaxed);
    }

    fn average_milliseconds(&self) -> f64 {
        let count = self.count.load(Ordering::Relaxed);
        if count == 0 {
            return 0.0;
        }

        let sum_micros = self.sum_micros.load(Ordering::Relaxed);
        sum_micros as f64 / count as f64 / 1_000.0
    }

    fn latest_milliseconds(&self) -> f64 {
        self.latest_micros.load(Ordering::Relaxed) as f64 / 1_000.0
    }

    fn max_milliseconds(&self) -> f64 {
        self.max_micros.load(Ordering::Relaxed) as f64 / 1_000.0
    }
}

pub fn record_world_session_registered() {
    let metrics = registry();
    metrics
        .world_sessions_connected
        .fetch_add(1, Ordering::Relaxed);
    metrics
        .world_sessions_registered_total
        .fetch_add(1, Ordering::Relaxed);
}

pub fn record_world_session_unregistered() {
    let metrics = registry();
    let _ = metrics.world_sessions_connected.fetch_update(
        Ordering::Relaxed,
        Ordering::Relaxed,
        |value| Some(value.saturating_sub(1)),
    );
    metrics
        .world_sessions_unregistered_total
        .fetch_add(1, Ordering::Relaxed);
}

pub fn record_world_packet_in(opcode: u32) {
    increment_opcode(&registry().world_packets_in, opcode);
}

pub fn record_world_packet_out(opcode: u16) {
    increment_opcode(&registry().world_packets_out, opcode as u32);
}

pub fn record_world_unknown_opcode(opcode: u32) {
    increment_opcode(&registry().world_unknown_opcodes, opcode);
}

pub fn record_map_tick(duration: Duration, lag: Duration, budget: Duration) {
    let metrics = registry();
    metrics.map_ticks_total.fetch_add(1, Ordering::Relaxed);
    metrics.map_tick_duration.record(duration);
    metrics.map_tick_lag.record(lag);
    if duration > budget || lag > budget {
        metrics
            .map_tick_over_budget_total
            .fetch_add(1, Ordering::Relaxed);
    }
}

pub fn record_map_phase_duration(phase: MapTickPhase, duration: Duration) {
    registry().map_phase_duration.get(phase).record(duration);
}

pub fn record_map_runtime_snapshots(snapshots: impl IntoIterator<Item = MapRuntimeSnapshot>) {
    let mut gauges = registry()
        .map_runtime_snapshots
        .lock()
        .expect("metrics map runtime snapshot gauge poisoned");
    gauges.clear();
    for snapshot in snapshots {
        gauges.insert((snapshot.map_id, snapshot.instance_id), snapshot);
    }
}

pub fn record_map_tick_error() {
    registry()
        .map_tick_errors_total
        .fetch_add(1, Ordering::Relaxed);
}

fn increment_opcode(counters: &Mutex<HashMap<u32, u64>>, opcode: u32) {
    let mut counters = counters.lock().expect("metrics opcode counter poisoned");
    *counters.entry(opcode).or_insert(0) += 1;
}

pub fn render_prometheus() -> String {
    let metrics = registry();
    let mut body = String::with_capacity(8192);

    write_counter(
        &mut body,
        "wow_world_sessions_registered_total",
        "Total authenticated world sessions registered.",
        metrics
            .world_sessions_registered_total
            .load(Ordering::Relaxed),
    );
    write_counter(
        &mut body,
        "wow_world_sessions_unregistered_total",
        "Total authenticated world sessions unregistered.",
        metrics
            .world_sessions_unregistered_total
            .load(Ordering::Relaxed),
    );
    write_gauge(
        &mut body,
        "wow_world_sessions_connected",
        "Currently connected authenticated world sessions.",
        metrics.world_sessions_connected.load(Ordering::Relaxed),
    );
    write_counter(
        &mut body,
        "wow_map_ticks_total",
        "Total map runtime update ticks.",
        metrics.map_ticks_total.load(Ordering::Relaxed),
    );
    write_counter(
        &mut body,
        "wow_map_tick_over_budget_total",
        "Map ticks whose duration or start lag exceeded the world tick budget.",
        metrics.map_tick_over_budget_total.load(Ordering::Relaxed),
    );
    write_counter(
        &mut body,
        "wow_map_tick_errors_total",
        "Map runtime update phases that returned an error.",
        metrics.map_tick_errors_total.load(Ordering::Relaxed),
    );
    write_histogram(
        &mut body,
        "wow_map_tick_duration_seconds",
        "Time spent executing map runtime update work.",
        &metrics.map_tick_duration,
    );
    write_float_gauge(
        &mut body,
        "wow_map_tick_duration_average_milliseconds",
        "Average time spent executing map runtime update work.",
        metrics.map_tick_duration.average_milliseconds(),
    );
    write_float_gauge(
        &mut body,
        "wow_map_tick_duration_latest_milliseconds",
        "Most recent time spent executing map runtime update work.",
        metrics.map_tick_duration.latest_milliseconds(),
    );
    write_float_gauge(
        &mut body,
        "wow_map_tick_duration_max_milliseconds",
        "Maximum observed time spent executing map runtime update work since server start.",
        metrics.map_tick_duration.max_milliseconds(),
    );
    write_histogram(
        &mut body,
        "wow_map_tick_lag_seconds",
        "How late the map runtime update loop started relative to its scheduled tick.",
        &metrics.map_tick_lag,
    );
    write_float_gauge(
        &mut body,
        "wow_map_tick_lag_average_milliseconds",
        "Average delay between the scheduled map tick time and the actual start time.",
        metrics.map_tick_lag.average_milliseconds(),
    );
    write_float_gauge(
        &mut body,
        "wow_map_tick_lag_latest_milliseconds",
        "Most recent delay between the scheduled map tick time and the actual start time.",
        metrics.map_tick_lag.latest_milliseconds(),
    );
    write_float_gauge(
        &mut body,
        "wow_map_tick_lag_max_milliseconds",
        "Maximum observed delay between the scheduled map tick time and the actual start time since server start.",
        metrics.map_tick_lag.max_milliseconds(),
    );
    write_map_phase_duration_summaries(&mut body, &metrics.map_phase_duration);
    write_map_runtime_gauges(&mut body, &metrics.map_runtime_snapshots);
    write_opcode_counter(
        &mut body,
        "wow_world_packets_in_total",
        "Total world packets received by opcode.",
        &metrics.world_packets_in,
    );
    write_opcode_counter(
        &mut body,
        "wow_world_packets_out_total",
        "Total world packets sent by opcode.",
        &metrics.world_packets_out,
    );
    write_opcode_counter(
        &mut body,
        "wow_world_unknown_opcodes_total",
        "Total authenticated world packets with no handler.",
        &metrics.world_unknown_opcodes,
    );
    body.push_str(&wow_db::render_db_metrics_prometheus());

    body
}

fn write_counter(body: &mut String, name: &str, help: &str, value: u64) {
    body.push_str("# HELP ");
    body.push_str(name);
    body.push(' ');
    body.push_str(help);
    body.push('\n');
    body.push_str("# TYPE ");
    body.push_str(name);
    body.push_str(" counter\n");
    body.push_str(name);
    body.push(' ');
    body.push_str(&value.to_string());
    body.push('\n');
}

fn write_gauge(body: &mut String, name: &str, help: &str, value: u64) {
    body.push_str("# HELP ");
    body.push_str(name);
    body.push(' ');
    body.push_str(help);
    body.push('\n');
    body.push_str("# TYPE ");
    body.push_str(name);
    body.push_str(" gauge\n");
    body.push_str(name);
    body.push(' ');
    body.push_str(&value.to_string());
    body.push('\n');
}

fn write_float_gauge(body: &mut String, name: &str, help: &str, value: f64) {
    body.push_str("# HELP ");
    body.push_str(name);
    body.push(' ');
    body.push_str(help);
    body.push('\n');
    body.push_str("# TYPE ");
    body.push_str(name);
    body.push_str(" gauge\n");
    body.push_str(name);
    body.push(' ');
    body.push_str(&format!("{value:.3}"));
    body.push('\n');
}

fn write_histogram(body: &mut String, name: &str, help: &str, histogram: &Histogram) {
    body.push_str("# HELP ");
    body.push_str(name);
    body.push(' ');
    body.push_str(help);
    body.push('\n');
    body.push_str("# TYPE ");
    body.push_str(name);
    body.push_str(" histogram\n");

    let mut cumulative = 0u64;
    for (index, bucket) in MAP_TICK_BUCKETS_SECONDS.iter().enumerate() {
        cumulative += histogram.buckets[index].load(Ordering::Relaxed);
        body.push_str(name);
        body.push_str("_bucket{le=\"");
        body.push_str(&format_bucket(*bucket));
        body.push_str("\"} ");
        body.push_str(&cumulative.to_string());
        body.push('\n');
    }

    cumulative += histogram.infinite_bucket.load(Ordering::Relaxed);
    body.push_str(name);
    body.push_str("_bucket{le=\"+Inf\"} ");
    body.push_str(&cumulative.to_string());
    body.push('\n');
    body.push_str(name);
    body.push_str("_sum ");
    let sum_seconds = histogram.sum_micros.load(Ordering::Relaxed) as f64 / 1_000_000.0;
    body.push_str(&format!("{sum_seconds:.6}"));
    body.push('\n');
    body.push_str(name);
    body.push_str("_count ");
    body.push_str(&histogram.count.load(Ordering::Relaxed).to_string());
    body.push('\n');
}

fn write_map_phase_duration_summaries(body: &mut String, phases: &MapPhaseDurations) {
    write_phase_float_gauges(
        body,
        "wow_map_phase_duration_average_milliseconds",
        "Average map update phase duration.",
        phases,
        Histogram::average_milliseconds,
    );
    write_phase_float_gauges(
        body,
        "wow_map_phase_duration_latest_milliseconds",
        "Most recent map update phase duration.",
        phases,
        Histogram::latest_milliseconds,
    );
    write_phase_float_gauges(
        body,
        "wow_map_phase_duration_max_milliseconds",
        "Maximum observed map update phase duration since server start.",
        phases,
        Histogram::max_milliseconds,
    );
}

fn write_map_runtime_gauges(
    body: &mut String,
    snapshots: &Mutex<HashMap<(u32, u32), MapRuntimeSnapshot>>,
) {
    let mut values: Vec<MapRuntimeSnapshot> = snapshots
        .lock()
        .expect("metrics map runtime snapshot gauge poisoned")
        .values()
        .copied()
        .collect();
    values.sort_by_key(|snapshot| (snapshot.map_id, snapshot.instance_id));

    write_map_runtime_gauge_family(
        body,
        "wow_map_active_players",
        "Currently active players by map runtime.",
        &values,
        |snapshot| snapshot.active_players,
    );
    write_map_runtime_gauge_family(
        body,
        "wow_map_active_creatures",
        "Currently active DB creatures by map runtime.",
        &values,
        |snapshot| snapshot.active_creatures,
    );
    write_map_runtime_gauge_family(
        body,
        "wow_map_active_gameobjects",
        "Currently active DB gameobjects by map runtime.",
        &values,
        |snapshot| snapshot.active_gameobjects,
    );
    write_map_runtime_gauge_family(
        body,
        "wow_map_loaded_grids",
        "Loaded spatial grids by map runtime.",
        &values,
        |snapshot| snapshot.loaded_grids,
    );
    write_map_runtime_gauge_family(
        body,
        "wow_map_loaded_creature_grids",
        "Loaded creature grids by map runtime.",
        &values,
        |snapshot| snapshot.loaded_creature_grids,
    );
    write_map_runtime_gauge_family(
        body,
        "wow_map_loaded_gameobject_grids",
        "Loaded gameobject grids by map runtime.",
        &values,
        |snapshot| snapshot.loaded_gameobject_grids,
    );
    write_map_runtime_gauge_family(
        body,
        "wow_map_loaded_player_corpse_grids",
        "Loaded player corpse grids by map runtime.",
        &values,
        |snapshot| snapshot.loaded_player_corpse_grids,
    );
    write_map_runtime_gauge_family(
        body,
        "wow_map_active_creature_combats",
        "Active creature combat states by map runtime.",
        &values,
        |snapshot| snapshot.active_creature_combats,
    );
    write_map_runtime_gauge_family(
        body,
        "wow_map_corpses",
        "Tracked player corpses by map runtime.",
        &values,
        |snapshot| snapshot.corpses,
    );
}

fn write_map_runtime_gauge_family(
    body: &mut String,
    name: &str,
    help: &str,
    snapshots: &[MapRuntimeSnapshot],
    value: fn(&MapRuntimeSnapshot) -> u64,
) {
    body.push_str("# HELP ");
    body.push_str(name);
    body.push(' ');
    body.push_str(help);
    body.push('\n');
    body.push_str("# TYPE ");
    body.push_str(name);
    body.push_str(" gauge\n");

    for snapshot in snapshots {
        body.push_str(name);
        body.push_str("{map_id=\"");
        body.push_str(&snapshot.map_id.to_string());
        body.push_str("\",instance_id=\"");
        body.push_str(&snapshot.instance_id.to_string());
        body.push_str("\"} ");
        body.push_str(&value(snapshot).to_string());
        body.push('\n');
    }
}

fn write_phase_float_gauges(
    body: &mut String,
    name: &str,
    help: &str,
    phases: &MapPhaseDurations,
    value: fn(&Histogram) -> f64,
) {
    body.push_str("# HELP ");
    body.push_str(name);
    body.push(' ');
    body.push_str(help);
    body.push('\n');
    body.push_str("# TYPE ");
    body.push_str(name);
    body.push_str(" gauge\n");
    for phase in MAP_TICK_PHASES {
        body.push_str(name);
        body.push_str("{phase=\"");
        body.push_str(phase.label());
        body.push_str("\"} ");
        body.push_str(&format!("{:.3}", value(phases.get(phase))));
        body.push('\n');
    }
}

fn write_opcode_counter(
    body: &mut String,
    name: &str,
    help: &str,
    counters: &Mutex<HashMap<u32, u64>>,
) {
    body.push_str("# HELP ");
    body.push_str(name);
    body.push(' ');
    body.push_str(help);
    body.push('\n');
    body.push_str("# TYPE ");
    body.push_str(name);
    body.push_str(" counter\n");

    let mut values: Vec<(u32, u64)> = counters
        .lock()
        .expect("metrics opcode counter poisoned")
        .iter()
        .map(|(opcode, count)| (*opcode, *count))
        .collect();
    values.sort_by_key(|(opcode, _)| *opcode);

    for (opcode, count) in values {
        body.push_str(name);
        body.push_str("{opcode=\"");
        body.push_str(&format_opcode(opcode));
        body.push_str("\"} ");
        body.push_str(&count.to_string());
        body.push('\n');
    }
}

fn format_bucket(bucket: f64) -> String {
    format!("{bucket:.3}")
}

fn format_opcode(opcode: u32) -> String {
    format!("0x{opcode:04X}")
}

pub async fn run_metrics_endpoint(bind_addr: SocketAddr) -> anyhow::Result<()> {
    let listener = TcpListener::bind(bind_addr).await?;
    info!(%bind_addr, "Observability metrics endpoint listening");

    loop {
        let (mut socket, peer) = listener.accept().await?;
        tokio::spawn(async move {
            let mut request = [0u8; 1024];
            let read = match socket.read(&mut request).await {
                Ok(read) => read,
                Err(error) => {
                    warn!(%peer, "Failed to read metrics request: {error}");
                    return;
                }
            };
            let request = String::from_utf8_lossy(&request[..read]);
            let first_line = request.lines().next().unwrap_or_default();
            let (status, content_type, body) = if first_line.starts_with("GET /metrics ") {
                (
                    "200 OK",
                    "text/plain; version=0.0.4; charset=utf-8",
                    render_prometheus(),
                )
            } else if first_line.starts_with("GET /healthz ") {
                ("200 OK", "text/plain; charset=utf-8", "ok\n".to_string())
            } else {
                (
                    "404 Not Found",
                    "text/plain; charset=utf-8",
                    "not found\n".to_string(),
                )
            };
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            if let Err(error) = socket.write_all(response.as_bytes()).await {
                warn!(%peer, "Failed to write metrics response: {error}");
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prometheus_render_includes_histogram_and_opcode_labels() {
        record_map_tick(
            Duration::from_millis(12),
            Duration::from_millis(3),
            Duration::from_millis(100),
        );
        record_map_phase_duration(MapTickPhase::PlayerRegen, Duration::from_millis(7));
        record_map_runtime_snapshots([MapRuntimeSnapshot {
            map_id: 0,
            instance_id: 0,
            active_players: 1,
            active_creatures: 42,
            active_gameobjects: 3,
            loaded_grids: 4,
            loaded_creature_grids: 5,
            loaded_gameobject_grids: 6,
            loaded_player_corpse_grids: 7,
            active_creature_combats: 2,
            corpses: 1,
        }]);
        record_world_packet_in(0x01E0);
        record_world_packet_out(0x00DD);
        record_world_unknown_opcode(0x01E0);

        let rendered = render_prometheus();

        assert!(rendered.contains("# TYPE wow_map_tick_duration_seconds histogram"));
        assert!(rendered.contains("wow_map_tick_duration_seconds_bucket{le=\"0.025\"}"));
        assert!(rendered.contains("wow_map_tick_duration_seconds_count "));
        assert!(rendered.contains("wow_map_tick_duration_average_milliseconds 12.000"));
        assert!(rendered.contains("wow_map_tick_duration_latest_milliseconds 12.000"));
        assert!(rendered.contains("wow_map_tick_duration_max_milliseconds 12.000"));
        assert!(rendered.contains("wow_map_tick_lag_average_milliseconds 3.000"));
        assert!(rendered.contains("wow_map_tick_lag_latest_milliseconds 3.000"));
        assert!(rendered.contains("wow_map_tick_lag_max_milliseconds 3.000"));
        assert!(rendered
            .contains("wow_map_phase_duration_max_milliseconds{phase=\"player_regen\"} 7.000"));
        assert!(rendered
            .contains("wow_map_phase_duration_latest_milliseconds{phase=\"idle_motion\"} 0.000"));
        assert!(rendered.contains("wow_map_active_players{map_id=\"0\",instance_id=\"0\"} 1"));
        assert!(rendered.contains("wow_map_active_creatures{map_id=\"0\",instance_id=\"0\"} 42"));
        assert!(
            rendered.contains("wow_map_active_creature_combats{map_id=\"0\",instance_id=\"0\"} 2")
        );
        assert!(rendered.contains("wow_world_packets_in_total{opcode=\"0x01E0\"}"));
        assert!(rendered.contains("wow_world_packets_out_total{opcode=\"0x00DD\"}"));
        assert!(rendered.contains("wow_world_unknown_opcodes_total{opcode=\"0x01E0\"}"));
    }
}
