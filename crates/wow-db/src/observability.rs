use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const ROLLING_ONE_MINUTE: Duration = Duration::from_secs(60);
const ROLLING_FIVE_MINUTES: Duration = Duration::from_secs(300);

static REGISTRY: OnceLock<DbMetricsRegistry> = OnceLock::new();

fn registry() -> &'static DbMetricsRegistry {
    REGISTRY.get_or_init(DbMetricsRegistry::default)
}

#[derive(Default)]
struct DbMetricsRegistry {
    query_families: Mutex<HashMap<&'static str, DbQueryStats>>,
}

#[derive(Debug, Clone, Default)]
struct DbQueryStats {
    count: u64,
    sum_micros: u64,
    latest_micros: u64,
    max_micros: u64,
    recent: VecDeque<TimedSample>,
}

#[derive(Debug, Clone, Copy)]
struct TimedSample {
    at: Instant,
    micros: u64,
}

#[derive(Debug, Clone, Copy, Default)]
struct RollingStats {
    count: u64,
    sum_micros: u64,
    max_micros: u64,
}

impl DbQueryStats {
    fn record(&mut self, duration: Duration) {
        let micros = duration.as_micros() as u64;
        self.count += 1;
        self.sum_micros += micros;
        self.latest_micros = micros;
        self.max_micros = self.max_micros.max(micros);
        let now = Instant::now();
        self.recent.push_back(TimedSample { at: now, micros });
        prune_samples(&mut self.recent, now, ROLLING_FIVE_MINUTES);
    }

    fn average_milliseconds(&self) -> f64 {
        if self.count == 0 {
            return 0.0;
        }
        self.sum_micros as f64 / self.count as f64 / 1_000.0
    }

    fn latest_milliseconds(&self) -> f64 {
        self.latest_micros as f64 / 1_000.0
    }

    fn max_milliseconds(&self) -> f64 {
        self.max_micros as f64 / 1_000.0
    }

    fn rolling_stats(&self, window: Duration) -> RollingStats {
        let now = Instant::now();
        self.recent
            .iter()
            .filter(|sample| now.saturating_duration_since(sample.at) <= window)
            .fold(RollingStats::default(), |mut stats, sample| {
                stats.count += 1;
                stats.sum_micros += sample.micros;
                stats.max_micros = stats.max_micros.max(sample.micros);
                stats
            })
    }
}

impl RollingStats {
    fn average_milliseconds(self) -> f64 {
        if self.count == 0 {
            return 0.0;
        }
        self.sum_micros as f64 / self.count as f64 / 1_000.0
    }

    fn max_milliseconds(self) -> f64 {
        self.max_micros as f64 / 1_000.0
    }
}

fn prune_samples(samples: &mut VecDeque<TimedSample>, now: Instant, window: Duration) {
    while samples
        .front()
        .is_some_and(|sample| now.saturating_duration_since(sample.at) > window)
    {
        samples.pop_front();
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
    let mut families = registry()
        .query_families
        .lock()
        .expect("DB metrics query family registry poisoned");
    let now = Instant::now();
    let mut values: Vec<(&'static str, DbQueryStats)> = families
        .iter_mut()
        .map(|(family, stats)| {
            prune_samples(&mut stats.recent, now, ROLLING_FIVE_MINUTES);
            (*family, stats.clone())
        })
        .collect();
    drop(families);
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
    write_db_query_float_gauge(
        &mut body,
        "wow_db_query_duration_average_1m_milliseconds",
        "Average DB call duration by query family over the last minute.",
        &values,
        |stats| {
            stats
                .rolling_stats(ROLLING_ONE_MINUTE)
                .average_milliseconds()
        },
    );
    write_db_query_float_gauge(
        &mut body,
        "wow_db_query_duration_max_1m_milliseconds",
        "Maximum DB call duration by query family over the last minute.",
        &values,
        |stats| stats.rolling_stats(ROLLING_ONE_MINUTE).max_milliseconds(),
    );
    write_db_query_float_gauge(
        &mut body,
        "wow_db_query_duration_average_5m_milliseconds",
        "Average DB call duration by query family over the last five minutes.",
        &values,
        |stats| {
            stats
                .rolling_stats(ROLLING_FIVE_MINUTES)
                .average_milliseconds()
        },
    );
    write_db_query_float_gauge(
        &mut body,
        "wow_db_query_duration_max_5m_milliseconds",
        "Maximum DB call duration by query family over the last five minutes.",
        &values,
        |stats| stats.rolling_stats(ROLLING_FIVE_MINUTES).max_milliseconds(),
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
    value: impl Fn(&DbQueryStats) -> f64,
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
        body.push_str(&format!("{:.3}", value(stats)));
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
        assert!(rendered.contains(
            "wow_db_query_duration_average_1m_milliseconds{family=\"observability_test\"} 5.000"
        ));
        assert!(rendered.contains(
            "wow_db_query_duration_max_1m_milliseconds{family=\"observability_test\"} 5.000"
        ));
        assert!(rendered.contains(
            "wow_db_query_duration_average_5m_milliseconds{family=\"observability_test\"} 5.000"
        ));
        assert!(rendered.contains(
            "wow_db_query_duration_max_5m_milliseconds{family=\"observability_test\"} 5.000"
        ));
    }
}
