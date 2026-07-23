use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::sha256::short_hex;

/// 妈祖是湄洲的神明，一律按东八区算「今天」
const TZ_OFFSET_SECS: i64 = 8 * 3600;
const KEEP_DAYS: usize = 30;

pub struct Worship {
    pub day: String,
    pub rank: u64,
    pub total: u64,
    pub repeat: bool,
}

#[derive(Default)]
struct DayRecord {
    pilgrims: HashMap<String, u64>,
    total: u64,
}

pub struct Counter {
    salt: String,
    log_path: PathBuf,
    log: BufWriter<File>,
    days: HashMap<String, DayRecord>,
}

impl Counter {
    pub fn open(data_dir: &Path, salt: String) -> std::io::Result<Self> {
        fs::create_dir_all(data_dir)?;
        let log_path = data_dir.join("worship.log");
        let days = load(&log_path)?;
        let log = BufWriter::new(OpenOptions::new().create(true).append(true).open(&log_path)?);
        Ok(Self {
            salt,
            log_path,
            log,
            days,
        })
    }

    fn pilgrim_id(&self, identity: &str) -> String {
        short_hex(format!("{}:{}", self.salt, identity).as_bytes())
    }

    /// 记一次祭拜。同一个人当天重复祭拜，仍然返回他第一次的号码。
    pub fn worship(&mut self, identity: &str) -> Worship {
        let day = today();
        let id = self.pilgrim_id(identity);

        let fresh_day = !self.days.contains_key(&day);
        let record = self.days.entry(day.clone()).or_default();

        if let Some(&rank) = record.pilgrims.get(&id) {
            return Worship {
                day,
                rank,
                total: record.total,
                repeat: true,
            };
        }

        record.total += 1;
        let rank = record.total;
        record.pilgrims.insert(id.clone(), rank);

        // 日志写失败不该拦住信众，最多是重启后丢号
        let _ = writeln!(self.log, "{day}\t{id}\t{rank}");
        let _ = self.log.flush();

        if fresh_day {
            self.prune();
        }

        Worship {
            day,
            rank,
            total: rank,
            repeat: false,
        }
    }

    /// 只留最近 KEEP_DAYS 天，超了就重写日志
    fn prune(&mut self) {
        if self.days.len() <= KEEP_DAYS {
            return;
        }
        let mut keys: Vec<String> = self.days.keys().cloned().collect();
        keys.sort();
        for stale in &keys[..keys.len() - KEEP_DAYS] {
            self.days.remove(stale);
        }
        let _ = self.rewrite();
    }

    fn rewrite(&mut self) -> std::io::Result<()> {
        let tmp = self.log_path.with_extension("tmp");
        {
            let mut out = BufWriter::new(File::create(&tmp)?);
            let mut keys: Vec<&String> = self.days.keys().collect();
            keys.sort();
            for day in keys {
                for (id, rank) in &self.days[day].pilgrims {
                    writeln!(out, "{day}\t{id}\t{rank}")?;
                }
            }
            out.flush()?;
        }
        fs::rename(&tmp, &self.log_path)?;
        self.log = BufWriter::new(OpenOptions::new().append(true).open(&self.log_path)?);
        Ok(())
    }
}

fn load(path: &Path) -> std::io::Result<HashMap<String, DayRecord>> {
    let mut days: HashMap<String, DayRecord> = HashMap::new();
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(days),
        Err(e) => return Err(e),
    };
    for line in raw.lines() {
        let mut parts = line.split('\t');
        let (Some(day), Some(id), Some(rank)) = (parts.next(), parts.next(), parts.next()) else {
            continue; // 半行、脏行，跳过
        };
        let Ok(rank) = rank.parse::<u64>() else {
            continue;
        };
        let record = days.entry(day.to_string()).or_default();
        record.pilgrims.insert(id.to_string(), rank);
        record.total = record.total.max(rank);
    }
    Ok(days)
}

pub fn today() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
        + TZ_OFFSET_SECS;
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    format!("{y:04}-{m:02}-{d:02}")
}

/// Howard Hinnant 的 civil_from_days，省掉一个 chrono 依赖
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_dates_are_right() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1)); // 闰年年初
        assert_eq!(civil_from_days(19_782), (2024, 2, 29)); // 闰日
        assert_eq!(civil_from_days(20_657), (2026, 7, 23));
    }

    #[test]
    fn same_visitor_keeps_first_rank() {
        let dir = std::env::temp_dir().join(format!("mazu-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mut c = Counter::open(&dir, "salt".into()).unwrap();

        let a = c.worship("1.1.1.1");
        let b = c.worship("2.2.2.2");
        let a2 = c.worship("1.1.1.1");

        assert_eq!((a.rank, a.repeat), (1, false));
        assert_eq!((b.rank, b.repeat), (2, false));
        assert_eq!((a2.rank, a2.repeat), (1, true));
        assert_eq!(a2.total, 2);

        // 重启后号码不变
        drop(c);
        let mut c = Counter::open(&dir, "salt".into()).unwrap();
        let a3 = c.worship("1.1.1.1");
        assert_eq!((a3.rank, a3.repeat, a3.total), (1, true, 2));
        let d = c.worship("3.3.3.3");
        assert_eq!(d.rank, 3);

        let _ = fs::remove_dir_all(&dir);
    }
}
