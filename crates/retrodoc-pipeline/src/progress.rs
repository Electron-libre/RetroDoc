//! Progress reporting of the long LLM passes: one log line when a unit
//! (file, directory, feature…) starts, with its rank, the percentage done,
//! the elapsed time and a rough ETA. The last line on screen therefore
//! tells which unit is being worked on, and how long the run has been on it.
//!
//! Units served from a cache are counted as done with [`Progress::skip`] and
//! don't log; the ETA is the average time of the units actually processed,
//! applied to those left.

use std::fmt::Write as _;
use std::time::{Duration, Instant};

pub(crate) struct Progress {
    label: &'static str,
    total: usize,
    done: usize,
    started: Instant,
    /// Start of the unit being processed, until the next `begin`/`skip`.
    current: Option<Instant>,
    processed: usize,
    processed_time: Duration,
}

impl Progress {
    pub fn new(label: &'static str, total: usize) -> Self {
        Self {
            label,
            total,
            done: 0,
            started: Instant::now(),
            current: None,
            processed: 0,
            processed_time: Duration::ZERO,
        }
    }

    /// Starts a unit (the previous one, if any, is complete) and logs it.
    pub fn begin(&mut self, item: &str) {
        self.finish_current();
        tracing::info!("{}", self.line(item, self.started.elapsed()));
        self.current = Some(Instant::now());
    }

    /// Logs a unit that starts while others may be in flight (concurrent
    /// passes); pair it with [`Progress::finish`] when it completes.
    pub fn start(&self, item: &str) {
        tracing::info!("{}", self.line(item, self.started.elapsed()));
    }

    /// Counts a unit started with [`Progress::start`] as complete. The ETA
    /// then rests on wall-clock time, so it accounts for the parallelism.
    pub fn finish(&mut self) {
        self.done += 1;
        self.processed += 1;
        self.processed_time = self.started.elapsed();
    }

    /// Counts a unit that needed no work (cache hit, nothing to do).
    pub fn skip(&mut self) {
        self.finish_current();
        self.done += 1;
    }

    fn finish_current(&mut self) {
        if let Some(started) = self.current.take() {
            self.done += 1;
            self.processed += 1;
            self.processed_time += started.elapsed();
        }
    }

    fn line(&self, item: &str, elapsed: Duration) -> String {
        let total = self.total.max(1);
        let percent = self.done * 100 / total;
        let mut line = format!(
            "{}: {}/{} ({percent}%) {item} — elapsed {}",
            self.label,
            (self.done + 1).min(total),
            self.total,
            format_duration(elapsed)
        );
        if self.processed > 0 {
            let remaining = self.total.saturating_sub(self.done);
            let millis =
                self.processed_time.as_millis() * remaining as u128 / self.processed as u128;
            let eta = Duration::from_millis(u64::try_from(millis).unwrap_or(u64::MAX));
            let _ = write!(line, ", ~{} left", format_duration(eta));
        }
        line
    }
}

fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    match secs {
        0..=59 => format!("{secs}s"),
        60..=3599 => format!("{}m{:02}s", secs / 60, secs % 60),
        _ => format!("{}h{:02}m", secs / 3600, secs % 3600 / 60),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_durations() {
        assert_eq!(format_duration(Duration::from_secs(7)), "7s");
        assert_eq!(format_duration(Duration::from_secs(192)), "3m12s");
        assert_eq!(format_duration(Duration::from_mins(65)), "1h05m");
    }

    #[test]
    fn line_shows_rank_percentage_and_eta_from_processed_units_only() {
        let mut progress = Progress::new("repo map", 10);
        assert_eq!(
            progress.line("a.rs", Duration::from_secs(0)),
            "repo map: 1/10 (0%) a.rs — elapsed 0s"
        );
        // Four cached units, two processed in 60s each: 4 units left.
        for _ in 0..4 {
            progress.skip();
        }
        progress.done += 2;
        progress.processed = 2;
        progress.processed_time = Duration::from_secs(120);
        assert_eq!(
            progress.line("b.rs", Duration::from_secs(125)),
            "repo map: 7/10 (60%) b.rs — elapsed 2m05s, ~4m00s left"
        );
    }
}
