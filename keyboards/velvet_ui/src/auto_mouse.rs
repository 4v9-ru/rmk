/// Auto-mouse layer + Scroll/Sniper mode for velvet_ui.
///
/// - Layer 4 (Mouse):  activates automatically when trackball moves
/// - Layer 5 (Scroll): trackball → scroll wheel
/// - Layer 6 (Sniper): trackball speed reduced
///
/// Runtime settings are exposed in Vial's Keyboard Settings tab.
use core::cell::RefCell;
use core::sync::atomic::{AtomicU32, AtomicU8, Ordering};

use embassy_time::{Duration, Instant, Timer};
use rmk::channel::{CONTROLLER_CHANNEL, KEYBOARD_REPORT_CHANNEL};
use rmk::descriptor::KeyboardReport;
use rmk::embassy_futures::select::{select, Either};
use rmk::event::{ControllerEvent, Event};
use rmk::hid::Report;
use rmk::input_device::{InputProcessor, ProcessResult};
use rmk::keymap::KeyMap;
use usbd_hid::descriptor::MouseReport;

// Layer numbers
const LAYER_SCROLL: u8 = 5;
const LAYER_SNIPER: u8 = 6;

const MODE_SNIPER: u8 = 1;
const MODE_SCROLL: u8 = 2;
const MODE_TEXT: u8 = 3;

const HID_ARROW_RIGHT: u8 = 0x4F;
const HID_ARROW_LEFT: u8 = 0x50;
const HID_ARROW_DOWN: u8 = 0x51;
const HID_ARROW_UP: u8 = 0x52;

const AUTO_LAYER_NONE: u8 = 0xFF;
const AUTO_LAYER_IDLE_MS: u32 = 350;

/// Shared state
static LAST_MOTION_T: AtomicU32 = AtomicU32::new(0);
static ACTIVE_LAYER: AtomicU8 = AtomicU8::new(0);
static ACTIVE_AUTO_LAYER: AtomicU8 = AtomicU8::new(AUTO_LAYER_NONE);
/// Current mouse button state (bitmask: bit0=MB1, bit1=MB2, bit2=MB3...)
/// Updated by auto_mouse_tick_task via ControllerEvent::Key
static MOUSE_BUTTONS: AtomicU8 = AtomicU8::new(0);

/// Scroll accumulators
static SCROLL_ACCUM_X: AtomicI32 = AtomicI32::new(0);
static SCROLL_ACCUM_Y: AtomicI32 = AtomicI32::new(0);

/// Normal mode accumulators — throttled to BLE connection interval (~8ms = 125Hz)
static NORMAL_ACCUM_X: AtomicI32 = AtomicI32::new(0);
static NORMAL_ACCUM_Y: AtomicI32 = AtomicI32::new(0);
/// Last normal-mode report timestamp in ms (for throttling)
static LAST_NORMAL_REPORT_MS: AtomicU32 = AtomicU32::new(0);
/// Normal mode report interval: 16ms ≈ 62Hz, gives headroom above BLE 7.5ms interval
const NORMAL_REPORT_INTERVAL_MS: u32 = 16;

// AtomicI32 via AtomicU32 bit-cast
struct AtomicI32(core::sync::atomic::AtomicU32);
impl AtomicI32 {
    const fn new(v: i32) -> Self {
        Self(core::sync::atomic::AtomicU32::new(v as u32))
    }
    fn store(&self, v: i32, ord: Ordering) {
        self.0.store(v as u32, ord);
    }
    fn fetch_add(&self, v: i32, ord: Ordering) -> i32 {
        self.0.fetch_add(v as u32, ord) as i32
    }
}

fn now_ms() -> u32 {
    (Instant::now().as_ticks() / (embassy_time::TICK_HZ / 1000)) as u32
}

async fn tap_keyboard_key(keycode: u8) {
    let mut report = KeyboardReport::default();
    report.keycodes[0] = keycode;
    KEYBOARD_REPORT_CHANNEL
        .send(Report::KeyboardReport(report))
        .await;
    KEYBOARD_REPORT_CHANNEL
        .send(Report::KeyboardReport(KeyboardReport::default()))
        .await;
}

/// Handle User keycodes for runtime trackball mode switching.
/// User0-User9 are reserved for BLE profile switching (handled natively by RMK).
/// User10-User12 are for trackball mode switching.
pub fn handle_user_keycode(keycode_idx: u8, pressed: bool) {
    if !pressed && (10..=12).contains(&keycode_idx) && !rmk::vial_settings::sticky_mode() {
        rmk::vial_settings::set_mode(0);
        return;
    }
    if !pressed {
        return;
    }
    match keycode_idx {
        10 => rmk::vial_settings::set_mode(MODE_SNIPER),
        11 => rmk::vial_settings::set_mode(MODE_SCROLL),
        12 => rmk::vial_settings::set_mode(MODE_TEXT),
        _ => {}
    }
}

/// InputProcessor inserted before trackball0_processor.
pub struct AutoMouseProcessor<
    'a,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const NUM_ENCODER: usize = 0,
> {
    keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
}

impl<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>
    AutoMouseProcessor<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>
{
    pub fn new(keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>) -> Self {
        Self { keymap }
    }

    fn sync_auto_layer_for_motion(&self, mode: u8) {
        if rmk::vial_settings::auto_layer_enabled_for_mode(mode) {
            let layer = rmk::vial_settings::auto_layer();
            LAST_MOTION_T.store(now_ms(), Ordering::Relaxed);
            let previous = ACTIVE_AUTO_LAYER.swap(layer, Ordering::Relaxed);
            if previous != layer {
                let mut keymap = self.keymap.borrow_mut();
                if previous != AUTO_LAYER_NONE {
                    keymap.deactivate_layer(previous);
                }
                if layer != 0 {
                    keymap.activate_layer(layer);
                }
            }
        } else {
            self.deactivate_auto_layer();
        }
    }

    fn deactivate_auto_layer(&self) {
        let previous = ACTIVE_AUTO_LAYER.swap(AUTO_LAYER_NONE, Ordering::Relaxed);
        if previous != AUTO_LAYER_NONE {
            self.keymap.borrow_mut().deactivate_layer(previous);
        }
    }
}

impl<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>
    InputProcessor<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>
    for AutoMouseProcessor<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>
{
    async fn process(&mut self, event: Event) -> ProcessResult {
        let Event::Joystick(axes) = event else {
            return ProcessResult::Continue(event);
        };

        let active_layer = ACTIVE_LAYER.load(Ordering::Relaxed);
        let mut mode = rmk::vial_settings::mode();
        if active_layer == LAYER_SCROLL {
            mode = MODE_SCROLL;
        } else if active_layer == LAYER_SNIPER {
            mode = MODE_SNIPER;
        }
        self.sync_auto_layer_for_motion(mode);

        // Extract dx/dy
        let mut dx: i16 = 0;
        let mut dy: i16 = 0;
        for axis in axes.iter() {
            match axis.axis {
                rmk::event::Axis::X => dx = axis.value,
                rmk::event::Axis::Y => dy = axis.value,
                _ => {}
            }
        }
        let (dx, dy) = rmk::vial_settings::transform_axes(dx, dy);
        let dx = rmk::vial_settings::scale_by_ball_dpi(dx as i32);
        let dy = rmk::vial_settings::scale_by_ball_dpi(dy as i32);

        match mode {
            MODE_SCROLL => {
                let divisor = rmk::vial_settings::scroll_sens();
                let acc_x = SCROLL_ACCUM_X.fetch_add(dx, Ordering::Relaxed) + dx;
                let acc_y = SCROLL_ACCUM_Y.fetch_add(dy, Ordering::Relaxed) + dy;

                let wheel_direction = if rmk::vial_settings::invert_scroll() {
                    1
                } else {
                    -1
                };
                let wheel = (wheel_direction * (acc_y / divisor)) as i8;
                let pan = (acc_x / divisor) as i8;

                if wheel != 0 || pan != 0 {
                    SCROLL_ACCUM_X.store(acc_x % divisor, Ordering::Relaxed);
                    SCROLL_ACCUM_Y.store(acc_y % divisor, Ordering::Relaxed);

                    let report = MouseReport {
                        buttons: 0,
                        x: 0,
                        y: 0,
                        wheel,
                        pan,
                    };
                    KEYBOARD_REPORT_CHANNEL
                        .send(Report::MouseReport(report))
                        .await;
                }
                ProcessResult::Stop
            }
            MODE_SNIPER => {
                let divisor = rmk::vial_settings::sniper_sens() as i32;
                let slow_dx = dx / divisor;
                let slow_dy = dy / divisor;

                if slow_dx != 0 || slow_dy != 0 {
                    let report = MouseReport {
                        buttons: MOUSE_BUTTONS.load(Ordering::Relaxed),
                        x: slow_dx.clamp(i8::MIN as i32, i8::MAX as i32) as i8,
                        y: slow_dy.clamp(i8::MIN as i32, i8::MAX as i32) as i8,
                        wheel: 0,
                        pan: 0,
                    };
                    KEYBOARD_REPORT_CHANNEL
                        .send(Report::MouseReport(report))
                        .await;
                }
                ProcessResult::Stop
            }
            MODE_TEXT => {
                let divisor = rmk::vial_settings::text_sens();
                let acc_x = NORMAL_ACCUM_X.fetch_add(dx, Ordering::Relaxed) + dx;
                let acc_y = NORMAL_ACCUM_Y.fetch_add(dy, Ordering::Relaxed) + dy;
                let mut shift_x = acc_x / divisor;
                let mut shift_y = acc_y / divisor;

                if shift_x != 0 || shift_y != 0 {
                    NORMAL_ACCUM_X.store(acc_x % divisor, Ordering::Relaxed);
                    NORMAL_ACCUM_Y.store(acc_y % divisor, Ordering::Relaxed);
                }

                if shift_x.abs() > shift_y.abs() {
                    shift_y = 0;
                    NORMAL_ACCUM_Y.store(0, Ordering::Relaxed);
                } else if shift_y.abs() > shift_x.abs() {
                    shift_x = 0;
                    NORMAL_ACCUM_X.store(0, Ordering::Relaxed);
                }

                if rmk::vial_settings::invert_text() {
                    shift_y = -shift_y;
                }

                while shift_x > 0 {
                    tap_keyboard_key(HID_ARROW_RIGHT).await;
                    shift_x -= 1;
                }
                while shift_x < 0 {
                    tap_keyboard_key(HID_ARROW_LEFT).await;
                    shift_x += 1;
                }
                while shift_y < 0 {
                    tap_keyboard_key(HID_ARROW_UP).await;
                    shift_y += 1;
                }
                while shift_y > 0 {
                    tap_keyboard_key(HID_ARROW_DOWN).await;
                    shift_y -= 1;
                }

                ProcessResult::Stop
            }
            _ => {
                // Normal mode: accumulate dx/dy and send throttled report (62Hz)
                // Include current button state so drag works correctly.
                let acc_x = NORMAL_ACCUM_X.fetch_add(dx, Ordering::Relaxed) + dx;
                let acc_y = NORMAL_ACCUM_Y.fetch_add(dy, Ordering::Relaxed) + dy;

                let now = now_ms();
                let last = LAST_NORMAL_REPORT_MS.load(Ordering::Relaxed);
                if now.wrapping_sub(last) >= NORMAL_REPORT_INTERVAL_MS {
                    if acc_x != 0 || acc_y != 0 {
                        NORMAL_ACCUM_X.store(0, Ordering::Relaxed);
                        NORMAL_ACCUM_Y.store(0, Ordering::Relaxed);
                        LAST_NORMAL_REPORT_MS.store(now, Ordering::Relaxed);

                        let report = MouseReport {
                            buttons: MOUSE_BUTTONS.load(Ordering::Relaxed),
                            x: acc_x.clamp(i8::MIN as i32, i8::MAX as i32) as i8,
                            y: acc_y.clamp(i8::MIN as i32, i8::MAX as i32) as i8,
                            wheel: 0,
                            pan: 0,
                        };
                        KEYBOARD_REPORT_CHANNEL
                            .send(Report::MouseReport(report))
                            .await;
                    }
                }
                ProcessResult::Stop
            }
        }
    }

    fn get_keymap(&self) -> &RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>> {
        self.keymap
    }
}

/// Background task: auto-mouse timeout + layer tracking + User keycode handling.
pub async fn auto_mouse_tick_task<
    'a,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const NUM_ENCODER: usize,
>(
    keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
) {
    let mut sub = defmt::unwrap!(CONTROLLER_CHANNEL.subscriber());

    loop {
        match select(
            Timer::after(Duration::from_millis(50)),
            sub.next_message_pure(),
        )
        .await
        {
            Either::First(_) => {
                let active = ACTIVE_AUTO_LAYER.load(Ordering::Relaxed);
                if active != AUTO_LAYER_NONE {
                    let elapsed = now_ms().wrapping_sub(LAST_MOTION_T.load(Ordering::Relaxed));
                    if elapsed >= AUTO_LAYER_IDLE_MS {
                        let previous = ACTIVE_AUTO_LAYER.swap(AUTO_LAYER_NONE, Ordering::Relaxed);
                        if previous != AUTO_LAYER_NONE {
                            keymap.borrow_mut().deactivate_layer(previous);
                        }
                        SCROLL_ACCUM_X.store(0, Ordering::Relaxed);
                        SCROLL_ACCUM_Y.store(0, Ordering::Relaxed);
                        NORMAL_ACCUM_X.store(0, Ordering::Relaxed);
                        NORMAL_ACCUM_Y.store(0, Ordering::Relaxed);
                    }
                }
            }
            Either::Second(event) => {
                match event {
                    ControllerEvent::Layer(layer) => {
                        ACTIVE_LAYER.store(layer, Ordering::Relaxed);
                    }
                    ControllerEvent::Key(key_event, action) => {
                        use rmk::types::action::{Action, KeyAction};
                        use rmk::types::keycode::KeyCode;
                        if let KeyAction::Single(Action::Key(kc)) = action {
                            // Track mouse button state for drag support
                            let btn_bit: Option<u8> = match kc {
                                KeyCode::MouseBtn1 => Some(1 << 0),
                                KeyCode::MouseBtn2 => Some(1 << 1),
                                KeyCode::MouseBtn3 => Some(1 << 2),
                                KeyCode::MouseBtn4 => Some(1 << 3),
                                KeyCode::MouseBtn5 => Some(1 << 4),
                                _ => None,
                            };
                            if let Some(bit) = btn_bit {
                                // Toggle button bit on each event (press sets, release clears).
                                // ControllerEvent::Key fires for both press and release,
                                // so XOR correctly tracks state: 0→1 on press, 1→0 on release.
                                let cur = MOUSE_BUTTONS.load(Ordering::Relaxed);
                                MOUSE_BUTTONS.store(cur ^ bit, Ordering::Relaxed);
                            }

                            // Handle User keycodes for trackball settings
                            let id = match kc {
                                KeyCode::User10 => Some(10),
                                KeyCode::User11 => Some(11),
                                KeyCode::User12 => Some(12),
                                _ => None,
                            };
                            if let Some(id) = id {
                                handle_user_keycode(id, key_event.is_pressed());
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
