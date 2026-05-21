use super::*;
use std::fs::{create_dir_all, File, OpenOptions};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone)]
pub(in crate::world) struct MovementTraceSettings {
    pub(in crate::world) enabled: bool,
    pub(in crate::world) log_path: PathBuf,
}

struct MovementTraceState {
    writer: Mutex<File>,
}

static MOVEMENT_TRACE: OnceLock<Option<MovementTraceState>> = OnceLock::new();

pub(in crate::world) fn init_movement_trace(settings: MovementTraceSettings) -> anyhow::Result<()> {
    if MOVEMENT_TRACE.get().is_some() {
        return Ok(());
    }
    if !settings.enabled {
        let _ = MOVEMENT_TRACE.set(None);
        return Ok(());
    }

    if let Some(parent) = settings.log_path.parent() {
        if !parent.as_os_str().is_empty() {
            create_dir_all(parent)?;
        }
    }
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&settings.log_path)?;
    writeln!(file, "# movement trace")?;
    let _ = MOVEMENT_TRACE.set(Some(MovementTraceState {
        writer: Mutex::new(file),
    }));
    info!(path = %settings.log_path.display(), "Movement trace enabled");
    Ok(())
}

fn trace_writer() -> Option<&'static Mutex<File>> {
    MOVEMENT_TRACE
        .get()
        .and_then(|state| state.as_ref().map(|state| &state.writer))
}

fn movement_trace_timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

pub(in crate::world) fn trace_guid_movement(
    stage: &str,
    guid: u32,
    opcode: u32,
    movement: &MovementInfo,
    extra: &str,
) {
    let Some(writer) = trace_writer() else {
        return;
    };
    let mut writer = writer.lock().expect("movement trace writer lock poisoned");
    let _ = writeln!(
        writer,
        "ts_ms={} stage={} guid={} opcode={} flags=0x{:08X} client_time={} x={:.3} y={:.3} z={:.3} o={:.6} fall_time={} jump={} {}",
        movement_trace_timestamp_millis(),
        stage,
        guid,
        movement_opcode_name(opcode),
        movement.flags,
        movement.client_time,
        movement.position.x,
        movement.position.y,
        movement.position.z,
        movement.position.orientation,
        movement.fall_time,
        (movement.flags & MOVEFLAG_JUMPING != 0) as u8,
        extra,
    );
}

pub(in crate::world) fn trace_named_movement(
    stage: &str,
    guid: u32,
    name: &str,
    opcode: u32,
    movement: &MovementInfo,
    extra: &str,
) {
    trace_guid_movement(
        stage,
        guid,
        opcode,
        movement,
        &format!("name={} {}", name, extra),
    );
}

pub(in crate::world) fn trace_movement_broadcast(
    stage: &str,
    mover_guid: u32,
    observer_guid: u32,
    opcode: u32,
    movement: &MovementInfo,
    extra: &str,
) {
    trace_guid_movement(
        stage,
        mover_guid,
        opcode,
        movement,
        &format!("observer_guid={} {}", observer_guid, extra),
    );
}
