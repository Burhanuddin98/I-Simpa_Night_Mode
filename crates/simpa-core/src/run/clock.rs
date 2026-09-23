//! Wall-clock times for run folders and `run.json`: this machine's local time, the time the user
//! sees in Explorer and on the taskbar, with its offset from UTC recorded so that no time is
//! ambiguous.
//!
//! On Windows the local time comes from `SystemTimeToTzSpecificLocalTime` with the current time
//! zone, which applies the daylight-saving rule of the date converted, not today's. Elsewhere, or
//! if that call fails, the time is UTC with offset +00:00. This module holds the only `unsafe` of
//! the run code.

use std::time::{SystemTime, UNIX_EPOCH};

/// A civil date and time, to the millisecond.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Civil {
    pub year: i64,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    pub milli: u32,
}

impl Civil {
    /// Seconds since 1970-01-01 00:00:00 of this date and time read as UTC (Howard Hinnant's
    /// `days_from_civil`).
    fn epoch_seconds(&self) -> i64 {
        let (m, d) = (i64::from(self.month), i64::from(self.day));
        let y = self.year - i64::from(m <= 2);
        let era = y.div_euclid(400);
        let yoe = y.rem_euclid(400);
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        let days = era * 146_097 + doe - 719_468;
        days * 86_400
            + i64::from(self.hour) * 3_600
            + i64::from(self.minute) * 60
            + i64::from(self.second)
    }
}

/// `t` in UTC (Howard Hinnant's `civil_from_days`).
pub fn utc(t: SystemTime) -> Civil {
    let d = t.duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = d.as_secs() as i64;
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    Civil {
        year: yoe + era * 400 + i64::from(month <= 2),
        month,
        day,
        hour: (rem / 3_600) as u32,
        minute: (rem % 3_600 / 60) as u32,
        second: (rem % 60) as u32,
        milli: d.subsec_millis(),
    }
}

/// `t` in this machine's time zone, and the zone's offset from UTC at `t` in minutes.
pub fn local(t: SystemTime) -> (Civil, i32) {
    let u = utc(t);
    match to_local(&u) {
        Some(l) => {
            let offset = (l.epoch_seconds() - u.epoch_seconds()) / 60;
            (l, offset as i32)
        }
        None => (u, 0),
    }
}

#[cfg(windows)]
fn to_local(u: &Civil) -> Option<Civil> {
    use windows_sys::Win32::Foundation::SYSTEMTIME;
    use windows_sys::Win32::System::Time::SystemTimeToTzSpecificLocalTime;
    // SYSTEMTIME holds years 1601 to 30827.
    let year = u16::try_from(u.year).ok().filter(|y| *y >= 1601)?;
    let from = SYSTEMTIME {
        wYear: year,
        wMonth: u.month as u16,
        wDayOfWeek: 0,
        wDay: u.day as u16,
        wHour: u.hour as u16,
        wMinute: u.minute as u16,
        wSecond: u.second as u16,
        wMilliseconds: u.milli as u16,
    };
    let mut to = SYSTEMTIME::default();
    // SAFETY: both pointers are to live SYSTEMTIMEs for the duration of the call; a null zone
    // pointer means the current time zone.
    let ok = unsafe { SystemTimeToTzSpecificLocalTime(std::ptr::null(), &from, &mut to) };
    (ok != 0).then(|| Civil {
        year: i64::from(to.wYear),
        month: u32::from(to.wMonth),
        day: u32::from(to.wDay),
        hour: u32::from(to.wHour),
        minute: u32::from(to.wMinute),
        second: u32::from(to.wSecond),
        milli: u32::from(to.wMilliseconds),
    })
}

#[cfg(not(windows))]
fn to_local(_: &Civil) -> Option<Civil> {
    None
}

/// `yyyyMMdd-HHmmss-fff` of `c`.
fn stamp(c: &Civil) -> String {
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}-{:03}",
        c.year, c.month, c.day, c.hour, c.minute, c.second, c.milli
    )
}

/// `yyyyMMdd-HHmmss-fff` in local time: the start of a run folder's name.
pub fn folder_stamp(t: SystemTime) -> String {
    stamp(&local(t).0)
}

/// RFC 3339 with milliseconds, in local time with its offset (`+02:00`): `run.json`'s `started`.
pub fn rfc3339(t: SystemTime) -> String {
    let (c, offset) = local(t);
    let sign = if offset < 0 { '-' } else { '+' };
    let off = offset.unsigned_abs();
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}{sign}{:02}:{:02}",
        c.year,
        c.month,
        c.day,
        c.hour,
        c.minute,
        c.second,
        c.milli,
        off / 60,
        off % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn at(s: u64, ms: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_millis(s * 1000 + ms)
    }

    #[test]
    fn utc_dates_are_civil_dates() {
        assert_eq!(stamp(&utc(at(0, 0))), "19700101-000000-000");
        // 2026-09-23 18:52:31.123 UTC, and the leap days of 2024 and 2000.
        assert_eq!(stamp(&utc(at(1_790_189_551, 123))), "20260923-185231-123");
        assert_eq!(stamp(&utc(at(1_709_164_800, 7))), "20240229-000000-007");
        assert_eq!(stamp(&utc(at(951_782_400, 0))), "20000229-000000-000");
        assert_eq!(stamp(&utc(at(4_102_444_799, 999))), "20991231-235959-999");
        // days_from_civil inverts civil_from_days.
        for s in [0, 951_782_400, 1_709_164_800, 1_790_189_551, 4_102_444_799] {
            assert_eq!(utc(at(s, 0)).epoch_seconds(), s as i64);
        }
    }

    /// Local times and offsets agree with .NET's view of this machine's time zone, for a winter
    /// and a summer instant (so a daylight-saving rule is applied per date) and for now. Says NO
    /// to UTC stamps on any machine whose zone is not UTC, and to one offset used for both dates
    /// in a zone with daylight saving.
    #[cfg(windows)]
    #[test]
    fn local_times_are_this_machines() {
        // 2026-01-15 and 2026-07-15, 12:00:00.250 UTC.
        let instants = [1_768_478_400u64, 1_784_116_800];
        let script = format!(
            "foreach ($s in {}) {{ $d = [DateTimeOffset]::FromUnixTimeSeconds($s).ToLocalTime(); \
             $d.ToString('yyyyMMdd-HHmmss') + ' ' + $d.Offset.TotalMinutes }}; \
             (Get-Date).ToString('yyyyMMdd-HHmm')",
            instants.map(|s| s.to_string()).join(",")
        );
        let before = folder_stamp(SystemTime::now());
        let out = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .output()
            .expect("powershell runs");
        let after = folder_stamp(SystemTime::now());
        let text = String::from_utf8_lossy(&out.stdout);
        let lines: Vec<&str> = text.lines().map(str::trim).collect();
        assert_eq!(lines.len(), 3, "{text}");
        for (s, want) in instants.iter().zip(&lines) {
            let (c, offset) = local(at(*s, 250));
            assert_eq!(format!("{} {offset}", &stamp(&c)[..15]), *want, "UTC {s}");
            assert_eq!(c.milli, 250);
            let r = rfc3339(at(*s, 250));
            let sign = if offset < 0 { '-' } else { '+' };
            let off = offset.unsigned_abs();
            assert!(
                r.ends_with(&format!(".250{sign}{:02}:{:02}", off / 60, off % 60)),
                "{r}"
            );
        }
        // Now, to the minute: PowerShell's clock was read between `before` and `after`.
        assert!(
            lines[2] == &before[..13] || lines[2] == &after[..13],
            "local now {}, ours {before} .. {after}",
            lines[2]
        );
    }
}
