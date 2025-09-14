use chrono::{DateTime, Local};
use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub struct ApiCallRecord {
    pub timestamp: DateTime<Local>,
}

impl ApiCallRecord {
    pub fn new(timestamp: DateTime<Local>) -> Self {
        Self { timestamp }
    }
}

#[derive(Debug)]
pub struct RateLimiter {
    calls: Vec<ApiCallRecord>,
    max_calls: usize,
    time_window: Duration,
}

impl RateLimiter {
    pub fn new(max_calls: usize, time_window: Duration) -> Self {
        Self {
            calls: Vec::new(),
            max_calls,
            time_window,
        }
    }

    pub fn try_add_call_record(&mut self, call_record: ApiCallRecord) -> bool {
        if self.calls.len() < self.max_calls {
            self.calls.push(call_record);
            true
        } else {
            let timestamp = call_record.timestamp;
            let cutoff_time = timestamp - chrono::Duration::from_std(self.time_window).unwrap();
            if self.max_calls > 0 && self.calls[0].timestamp < cutoff_time {
                self.calls.remove(0);
                self.calls.push(call_record);
                true
            } else {
                false
            }
        }
    }

    pub fn calls_count(&self) -> usize {
        self.calls.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_rate_limiter_allows_initial_calls() {
        let mut limiter = RateLimiter::new(3, Duration::from_secs(60));
        let base_time = Local.with_ymd_and_hms(2023, 1, 1, 12, 0, 0).unwrap();

        assert!(limiter.try_add_call_record(ApiCallRecord::new(base_time)));
        assert!(limiter.try_add_call_record(ApiCallRecord::new(base_time)));
        assert!(limiter.try_add_call_record(ApiCallRecord::new(base_time)));
        assert_eq!(limiter.calls_count(), 3);
    }

    #[test]
    fn test_rate_limiter_blocks_excess_calls() {
        let mut limiter = RateLimiter::new(2, Duration::from_secs(60));
        let base_time = Local.with_ymd_and_hms(2023, 1, 1, 12, 0, 0).unwrap();

        assert!(limiter.try_add_call_record(ApiCallRecord::new(base_time)));
        assert!(limiter.try_add_call_record(ApiCallRecord::new(base_time)));
        assert!(!limiter.try_add_call_record(ApiCallRecord::new(base_time)));
        assert_eq!(limiter.calls_count(), 2);
    }

    #[test]
    fn test_rate_limiter_cleans_old_calls() {
        let mut limiter = RateLimiter::new(2, Duration::from_secs(60));
        let base_time = Local.with_ymd_and_hms(2023, 1, 1, 12, 0, 0).unwrap();
        let later_time = base_time + chrono::Duration::seconds(70);

        // Fill up the limiter
        assert!(limiter.try_add_call_record(ApiCallRecord::new(base_time)));
        assert!(limiter.try_add_call_record(ApiCallRecord::new(base_time)));
        assert!(!limiter.try_add_call_record(ApiCallRecord::new(base_time)));

        // After time window passes, should allow new calls
        assert!(limiter.try_add_call_record(ApiCallRecord::new(later_time)));
        assert_eq!(limiter.calls_count(), 2);
    }

    #[test]
    fn test_empty_rate_limiter() {
        let limiter = RateLimiter::new(5, Duration::from_secs(60));
        assert_eq!(limiter.calls_count(), 0);
    }
}
