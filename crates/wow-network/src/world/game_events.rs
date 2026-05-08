#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct GameEventState {
    active_events: HashSet<u16>,
}

impl GameEventState {
    fn from_schedules_at(schedules: &[wow_db::GameEventScheduleQuery], now_unix: i64) -> Self {
        let by_entry = schedules
            .iter()
            .map(|schedule| (schedule.entry, schedule))
            .collect::<HashMap<_, _>>();
        let mut raw_active = HashSet::new();
        for schedule in schedules {
            if game_event_is_active_at(schedule, now_unix) {
                raw_active.insert(schedule.entry);
            }
        }
        let active_events = raw_active
            .iter()
            .copied()
            .filter(|entry| {
                let Some(schedule) = by_entry.get(entry) else {
                    return false;
                };
                schedule.linked_to == 0 || raw_active.contains(&schedule.linked_to)
            })
            .collect();
        Self { active_events }
    }

    fn is_active(&self, event_id: u16) -> bool {
        self.active_events.contains(&event_id)
    }

    fn spawn_is_active(&self, game_event: Option<i16>) -> bool {
        match game_event {
            Some(event) if event > 0 => self.is_active(event as u16),
            Some(event) if event < 0 => !self.is_active(event.unsigned_abs()),
            _ => true,
        }
    }

    fn active_count(&self) -> usize {
        self.active_events.len()
    }
}

fn game_event_is_active_at(schedule: &wow_db::GameEventScheduleQuery, now_unix: i64) -> bool {
    if schedule.entry == 0 || schedule.occurrence == 0 {
        return false;
    }
    if schedule.length == 0 && schedule.schedule_type != 0 {
        return false;
    }
    if schedule.occurrence < schedule.length {
        return false;
    }

    match schedule.schedule_type {
        0 => false,
        11 => yearly_game_event_is_active_at(schedule, now_unix),
        _ => {
            let Some(start) = schedule.start_time_unix else {
                return false;
            };
            let Some(end) = schedule.end_time_unix else {
                return false;
            };
            game_event_interval_is_active_at(schedule, start, end, now_unix)
        }
    }
}

fn yearly_game_event_is_active_at(schedule: &wow_db::GameEventScheduleQuery, now_unix: i64) -> bool {
    let Some(start) = schedule.start_time_unix else {
        return false;
    };
    let Some(end) = schedule.end_time_unix else {
        return false;
    };
    let (now_year, _, _, _, _, _) = unix_to_ymdhms(now_unix);
    let (_, start_month, start_day, start_hour, start_minute, start_second) =
        unix_to_ymdhms(start);
    let (_, end_month, end_day, end_hour, end_minute, end_second) = unix_to_ymdhms(end);

    for year in [now_year - 1, now_year, now_year + 1] {
        let Some(start_for_year) = ymdhms_to_unix(
            year,
            start_month,
            start_day,
            start_hour,
            start_minute,
            start_second,
        ) else {
            continue;
        };
        let mut end_year = year;
        let Some(mut end_for_year) =
            ymdhms_to_unix(end_year, end_month, end_day, end_hour, end_minute, end_second)
        else {
            continue;
        };
        if end_for_year <= start_for_year {
            end_year += 1;
            let Some(next_year_end) = ymdhms_to_unix(
                end_year,
                end_month,
                end_day,
                end_hour,
                end_minute,
                end_second,
            ) else {
                continue;
            };
            end_for_year = next_year_end;
        }
        if game_event_interval_is_active_at(schedule, start_for_year, end_for_year, now_unix) {
            return true;
        }
    }
    false
}

fn game_event_interval_is_active_at(
    schedule: &wow_db::GameEventScheduleQuery,
    start: i64,
    end: i64,
    now_unix: i64,
) -> bool {
    if !(start < now_unix && now_unix < end) {
        return false;
    }
    let occurrence = i64::from(schedule.occurrence) * 60;
    let length = i64::from(schedule.length) * 60;
    if occurrence <= 0 || length <= 0 || occurrence < length {
        return false;
    }
    (now_unix - start).rem_euclid(occurrence) < length
}

fn unix_to_ymdhms(unix: i64) -> (i32, u32, u32, u32, u32, u32) {
    let days = unix.div_euclid(86_400);
    let seconds_of_day = unix.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = (seconds_of_day / 3_600) as u32;
    let minute = ((seconds_of_day % 3_600) / 60) as u32;
    let second = (seconds_of_day % 60) as u32;
    (year, month, day, hour, minute, second)
}

fn ymdhms_to_unix(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
) -> Option<i64> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    Some(
        days_from_civil(year, month, day) * 86_400
            + i64::from(hour) * 3_600
            + i64::from(minute) * 60
            + i64::from(second),
    )
}

fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let year = i64::from(year) - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month = i64::from(month);
    let day = i64::from(day);
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let days = days + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year =
        day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year as i32, month as u32, day as u32)
}
