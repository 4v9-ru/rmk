#![no_main]
#![no_std]

mod battery_nrf;

use rmk::config::{BehaviorConfig, MouseKeyConfig, RmkConfig};
use rmk::macros::rmk_peripheral;

#[rmk_peripheral(id = 0)]
mod keyboard_peripheral {
    #[register_processor(event)]
    fn battery() -> crate::battery_nrf::Op36Battery {
        crate::battery_nrf::Op36Battery::new(p.SAADC, p.P0_31)
    }
}

#[rmk_keyboard]
mod my_keyboard {
    // This overrides RMK's default behavior configuration
    #[Override(behavior_config)]
    fn custom_behavior() -> BehaviorConfig {
        BehaviorConfig {
            mouse_key: MouseKeyConfig {
                initial_delay_ms: 80,
                repeat_interval_ms: 16, // Should match your [rmk].mouse_key_interval
                move_delta: 6,          // Increase for faster base speed
                max_speed: 5,           // Max multiplier cap
                ticks_to_max: 30,       // Reach max speed faster (30 ticks)
            },
            ..Default::default()
        }
    }
}
