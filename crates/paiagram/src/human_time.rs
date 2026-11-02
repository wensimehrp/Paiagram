// SPDX-License-Identifier: MPL-2.0
//! Take timestamps and humanize the times.

use egui_i18n::tr;

pub(crate) fn to_human_time_string(time: web_time::SystemTime) -> String {
    let now = web_time::SystemTime::now();

    let (is_future, diff) = match now.duration_since(time) {
        Ok(d) => (false, d),
        Err(d) => (true, d.duration()),
    };

    let seconds = diff.as_secs();

    if !is_future && seconds < 5 {
        return tr!("time-just-now");
    }

    let (unit, count) = if seconds < 60 {
        ("seconds", seconds.max(1))
    } else if seconds < 3_600 {
        ("minutes", seconds / 60)
    } else if seconds < 86_400 {
        ("hours", seconds / 3_600)
    } else if seconds < 604_800 {
        ("days", seconds / 86_400)
    } else {
        let timestamp;
        #[cfg(target_arch = "wasm32")]
        {
            use web_time::web::SystemTimeExt;
            timestamp = jiff::Timestamp::try_from(time.to_std());
        };
        #[cfg(not(target_arch = "wasm32"))]
        {
            timestamp = jiff::Timestamp::try_from(time);
        };

        let timestamp = timestamp.expect("system time is outside Jiff's supported range");

        return timestamp.to_zoned(jiff::tz::TimeZone::UTC).strftime("%b %d, %Y").to_string();
    };

    match (is_future, unit) {
        (false, "seconds") => tr!("time-ago-seconds", { count: count }),
        (false, "minutes") => tr!("time-ago-minutes", { count: count }),
        (false, "hours") => tr!("time-ago-hours", { count: count }),
        (false, "days") => tr!("time-ago-days", { count: count }),
        (true, "seconds") => tr!("time-in-seconds", { count: count }),
        (true, "minutes") => tr!("time-in-minutes", { count: count }),
        (true, "hours") => tr!("time-in-hours", { count: count }),
        (true, "days") => tr!("time-in-days", { count: count }),
        _ => unreachable!(),
    }
}
