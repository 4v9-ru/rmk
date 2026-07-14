pub mod common;

use embassy_futures::block_on;
use embassy_time::{Duration, Timer};
use rmk::channel::{KEY_EVENT_CHANNEL, KEYBOARD_REPORT_CHANNEL};
use rmk::config::BehaviorConfig;
use rmk::event::KeyboardEvent;
use rmk::hid::Report;

use crate::common::KC_LSHIFT;
use crate::common::morse::create_simple_morse_keyboard;

#[test]
fn expired_morse_timeout_wins_over_queued_event() {
    block_on(async {
        KEY_EVENT_CHANNEL.clear();
        KEYBOARD_REPORT_CHANNEL.clear();

        let mut keyboard = create_simple_morse_keyboard(BehaviorConfig::default());
        keyboard.process_inner(KeyboardEvent::key(0, 1, true)).await;
        let key = keyboard
            .next_buffered_key()
            .expect("morse key should be buffered");

        Timer::at(key.timeout_time + Duration::from_millis(1)).await;
        KEY_EVENT_CHANNEL.send(KeyboardEvent::key(0, 0, true)).await;
        keyboard.process_buffered_key(key).await;

        let Report::KeyboardReport(report) = KEYBOARD_REPORT_CHANNEL.receive().await else {
            panic!("expected keyboard report");
        };
        assert_eq!(report.modifier, KC_LSHIFT);
    });
}
