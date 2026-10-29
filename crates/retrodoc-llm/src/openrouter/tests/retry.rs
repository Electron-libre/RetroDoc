use std::time::Duration;

use super::super::{is_retryable_status, retry_delay, server_retry_delay};

#[test]
fn server_retry_delay_reads_the_header_then_the_google_body() {
    let body = r#"{"error":{"details":[{"@type":"type.googleapis.com/google.rpc.RetryInfo","retryDelay": "43s"}]}}"#;
    assert_eq!(
        server_retry_delay(Some("5"), body),
        Some(Duration::from_secs(6))
    );
    assert_eq!(
        server_retry_delay(None, body),
        Some(Duration::from_secs(44))
    );
    assert_eq!(
        server_retry_delay(None, r#""retryDelay":"1.5s""#),
        Some(Duration::from_millis(2500))
    );
    assert_eq!(server_retry_delay(None, "Too Many Requests"), None);
    assert_eq!(server_retry_delay(None, r#""retryDelay":"soon""#), None);
    assert_eq!(server_retry_delay(Some("tomorrow"), "{}"), None);
}

#[test]
fn retry_delay_backs_off_exponentially_unless_the_server_asks_for_more() {
    assert_eq!(retry_delay(1, None), Duration::from_millis(500));
    assert_eq!(retry_delay(3, None), Duration::from_secs(2));
    assert_eq!(
        retry_delay(1, Some(Duration::from_secs(44))),
        Duration::from_secs(44)
    );
    assert_eq!(
        retry_delay(3, Some(Duration::from_millis(100))),
        Duration::from_secs(2)
    );
}

#[test]
fn retryable_statuses_are_429_and_5xx() {
    assert!(is_retryable_status(reqwest::StatusCode::TOO_MANY_REQUESTS));
    assert!(is_retryable_status(
        reqwest::StatusCode::INTERNAL_SERVER_ERROR
    ));
    assert!(is_retryable_status(
        reqwest::StatusCode::SERVICE_UNAVAILABLE
    ));
    assert!(!is_retryable_status(reqwest::StatusCode::BAD_REQUEST));
    assert!(!is_retryable_status(reqwest::StatusCode::UNAUTHORIZED));
    assert!(!is_retryable_status(reqwest::StatusCode::OK));
}
