use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

static REGISTRY: OnceLock<DbMetricsRegistry> = OnceLock::new();

fn registry() -> &'static DbMetricsRegistry {
    REGISTRY.get_or_init(DbMetricsRegistry::default)
}

#[derive(Default)]
struct DbMetricsRegistry {
    query_families: Mutex<HashMap<&'static str, DbQueryStats>>,
}

#[derive(Debug, Clone, Copy, Default)]
struct DbQueryStats {
    count: u64,
    sum_micros: u64,
    latest_micros: u64,
    max_micros: u64,
}

impl DbQueryStats {
    fn record(&mut self, duration: Duration) {
        let micros = duration.as_micros() as u64;
        self.count += 1;
        self.sum_micros += micros;
        self.latest_micros = micros;
        self.max_micros = self.max_micros.max(micros);
    }

    fn average_milliseconds(self) -> f64 {
        if self.count == 0 {
            return 0.0;
        }
        self.sum_micros as f64 / self.count as f64 / 1_000.0
    }

    fn latest_milliseconds(self) -> f64 {
        self.latest_micros as f64 / 1_000.0
    }

    fn max_milliseconds(self) -> f64 {
        self.max_micros as f64 / 1_000.0
    }
}

#[must_use]
pub struct DbQueryTimer {
    family: &'static str,
    started_at: Instant,
}

impl DbQueryTimer {
    pub fn start(family: &'static str) -> Self {
        Self {
            family,
            started_at: Instant::now(),
        }
    }
}

impl Drop for DbQueryTimer {
    fn drop(&mut self) {
        record_db_query_duration(self.family, self.started_at.elapsed());
    }
}

pub fn record_db_query_duration(family: &'static str, duration: Duration) {
    let mut families = registry()
        .query_families
        .lock()
        .expect("DB metrics query family registry poisoned");
    families.entry(family).or_default().record(duration);
}

pub fn render_db_metrics_prometheus() -> String {
    let mut values: Vec<(&'static str, DbQueryStats)> = registry()
        .query_families
        .lock()
        .expect("DB metrics query family registry poisoned")
        .iter()
        .map(|(family, stats)| (*family, *stats))
        .collect();
    values.sort_by_key(|(family, _)| *family);

    let mut body = String::new();
    write_db_query_counter(
        &mut body,
        "wow_db_query_total",
        "Total DB calls by query family.",
        &values,
    );
    write_db_query_float_gauge(
        &mut body,
        "wow_db_query_duration_average_milliseconds",
        "Average DB call duration by query family.",
        &values,
        DbQueryStats::average_milliseconds,
    );
    write_db_query_float_gauge(
        &mut body,
        "wow_db_query_duration_latest_milliseconds",
        "Most recent DB call duration by query family.",
        &values,
        DbQueryStats::latest_milliseconds,
    );
    write_db_query_float_gauge(
        &mut body,
        "wow_db_query_duration_max_milliseconds",
        "Maximum observed DB call duration by query family since server start.",
        &values,
        DbQueryStats::max_milliseconds,
    );
    body
}

fn write_db_query_counter(
    body: &mut String,
    name: &str,
    help: &str,
    values: &[(&'static str, DbQueryStats)],
) {
    body.push_str("# HELP ");
    body.push_str(name);
    body.push(' ');
    body.push_str(help);
    body.push('\n');
    body.push_str("# TYPE ");
    body.push_str(name);
    body.push_str(" counter\n");
    for (family, stats) in values {
        body.push_str(name);
        body.push_str("{family=\"");
        body.push_str(family);
        body.push_str("\"} ");
        body.push_str(&stats.count.to_string());
        body.push('\n');
    }
}

fn write_db_query_float_gauge(
    body: &mut String,
    name: &str,
    help: &str,
    values: &[(&'static str, DbQueryStats)],
    value: fn(DbQueryStats) -> f64,
) {
    body.push_str("# HELP ");
    body.push_str(name);
    body.push(' ');
    body.push_str(help);
    body.push('\n');
    body.push_str("# TYPE ");
    body.push_str(name);
    body.push_str(" gauge\n");
    for (family, stats) in values {
        body.push_str(name);
        body.push_str("{family=\"");
        body.push_str(family);
        body.push_str("\"} ");
        body.push_str(&format!("{:.3}", value(*stats)));
        body.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prometheus_render_includes_db_query_family_stats() {
        record_db_query_duration("observability_test", Duration::from_millis(5));

        let rendered = render_db_metrics_prometheus();

        assert!(rendered.contains("wow_db_query_total{family=\"observability_test\"}"));
        assert!(rendered.contains(
            "wow_db_query_duration_average_milliseconds{family=\"observability_test\"} 5.000"
        ));
        assert!(rendered.contains(
            "wow_db_query_duration_latest_milliseconds{family=\"observability_test\"} 5.000"
        ));
        assert!(rendered.contains(
            "wow_db_query_duration_max_milliseconds{family=\"observability_test\"} 5.000"
        ));
    }
}
