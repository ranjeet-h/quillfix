use quillfix::debounce::{Debouncer, filter_text};
use std::thread;
use std::time::Duration;

#[test]
fn rapid_feeds_only_last_fires_after_delay() {
    let mut debouncer = Debouncer::new(300);

    for value in ["hello one", "hello two", "hello three", "hello four", "hello five"] {
        assert!(debouncer.feed(value).is_none());
    }

    thread::sleep(Duration::from_millis(320));
    let emitted = debouncer.feed("hello five");
    assert_eq!(emitted.as_deref(), Some("hello five"));
}

#[test]
fn same_text_is_deduplicated() {
    let mut debouncer = Debouncer::new(50);
    assert!(debouncer.feed("hello world").is_none());
    thread::sleep(Duration::from_millis(60));
    assert_eq!(debouncer.feed("hello world").as_deref(), Some("hello world"));

    assert!(debouncer.feed("hello world").is_none());
    thread::sleep(Duration::from_millis(60));
    assert!(debouncer.feed("hello world").is_none());
}

#[test]
fn different_text_after_first_stable_fires_again() {
    let mut debouncer = Debouncer::new(30);
    assert!(debouncer.feed("first stable text").is_none());
    thread::sleep(Duration::from_millis(40));
    assert_eq!(debouncer.feed("first stable text").as_deref(), Some("first stable text"));

    assert!(debouncer.feed("second stable text").is_none());
    thread::sleep(Duration::from_millis(40));
    assert_eq!(debouncer.feed("second stable text").as_deref(), Some("second stable text"));
}

#[test]
fn filter_text_short_text_rejected() {
    assert_eq!(filter_text("hi"), None);
    assert_eq!(filter_text("hello"), Some("hello"));
}

#[test]
fn filter_text_max_length_cap() {
    let valid = "a".repeat(1500);
    let invalid = "a".repeat(1501);
    assert!(filter_text(&valid).is_some());
    assert!(filter_text(&invalid).is_none());
}
