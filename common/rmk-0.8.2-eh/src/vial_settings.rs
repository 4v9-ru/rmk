use core::sync::atomic::{AtomicU8, Ordering};

pub const QSID_BALL_DPI: u16 = 121;
pub const QSID_SNIPER_SENS: u16 = 127;
pub const QSID_SCROLL_SENS: u16 = 128;
pub const QSID_TEXT_SENS: u16 = 129;
pub const QSID_BALL_AXIS: u16 = 131;
pub const QSID_MODE: u16 = 135;
pub const QSID_INVERT_SCROLL: u16 = 138;
pub const QSID_ACCELERATION: u16 = 139;
pub const QSID_STICKY_MODE: u16 = 141;
pub const QSID_AUTO_LAYER_NORMAL: u16 = 142;
pub const QSID_AUTO_LAYER: u16 = 143;
pub const QSID_AUTO_LAYER_SNIPER: u16 = 144;
pub const QSID_AUTO_LAYER_SCROLL: u16 = 145;
pub const QSID_AUTO_LAYER_TEXT: u16 = 146;
pub const QSID_INVERT_TEXT: u16 = 148;

pub const SETTING_KEYS: &[u16] = &[
    QSID_BALL_DPI,
    QSID_SNIPER_SENS,
    QSID_SCROLL_SENS,
    QSID_TEXT_SENS,
    QSID_BALL_AXIS,
    QSID_MODE,
    QSID_INVERT_SCROLL,
    QSID_ACCELERATION,
    QSID_STICKY_MODE,
    QSID_AUTO_LAYER_NORMAL,
    QSID_AUTO_LAYER,
    QSID_AUTO_LAYER_SNIPER,
    QSID_AUTO_LAYER_SCROLL,
    QSID_AUTO_LAYER_TEXT,
    QSID_INVERT_TEXT,
];

const DEFAULT_BALL_DPI: u8 = 4; // 1000 CPI in the Phenom table
const DEFAULT_SNIPER_SENS: u8 = 4;
const DEFAULT_SCROLL_SENS: u8 = 5;
const DEFAULT_TEXT_SENS: u8 = 5;
const DEFAULT_BALL_AXIS: u8 = 0;
const DEFAULT_MODE: u8 = 0;
const DEFAULT_AUTO_LAYER: u8 = 4;
const DEFAULT_FLAGS: u8 = FLAG_AUTO_LAYER_NORMAL;

const FLAG_INVERT_SCROLL: u8 = 1 << 0;
const FLAG_ACCELERATION: u8 = 1 << 1;
const FLAG_STICKY_MODE: u8 = 1 << 2;
const FLAG_AUTO_LAYER_NORMAL: u8 = 1 << 3;
const FLAG_AUTO_LAYER_SNIPER: u8 = 1 << 4;
const FLAG_AUTO_LAYER_SCROLL: u8 = 1 << 5;
const FLAG_AUTO_LAYER_TEXT: u8 = 1 << 6;
const FLAG_INVERT_TEXT: u8 = 1 << 7;

const DPI_TABLE: [i32; 16] = [
    200, 400, 600, 800, 1000, 1200, 1400, 1600, 1800, 2000, 2200, 2400, 2600, 2800, 3000, 3200,
];

#[derive(
    Clone,
    Copy,
    Debug,
    serde::Serialize,
    serde::Deserialize,
    postcard::experimental::max_size::MaxSize,
)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct VialSettingsData {
    pub ball_dpi: u8,
    pub sniper_sens: u8,
    pub scroll_sens: u8,
    pub text_sens: u8,
    pub ball_axis: u8,
    pub mode: u8,
    pub flags: u8,
    pub auto_layer: u8,
}

static BALL_DPI: AtomicU8 = AtomicU8::new(DEFAULT_BALL_DPI);
static SNIPER_SENS: AtomicU8 = AtomicU8::new(DEFAULT_SNIPER_SENS);
static SCROLL_SENS: AtomicU8 = AtomicU8::new(DEFAULT_SCROLL_SENS);
static TEXT_SENS: AtomicU8 = AtomicU8::new(DEFAULT_TEXT_SENS);
static BALL_AXIS: AtomicU8 = AtomicU8::new(DEFAULT_BALL_AXIS);
static MODE: AtomicU8 = AtomicU8::new(DEFAULT_MODE);
static FLAGS: AtomicU8 = AtomicU8::new(DEFAULT_FLAGS);
static AUTO_LAYER: AtomicU8 = AtomicU8::new(DEFAULT_AUTO_LAYER);

impl VialSettingsData {
    pub const fn defaults() -> Self {
        Self {
            ball_dpi: DEFAULT_BALL_DPI,
            sniper_sens: DEFAULT_SNIPER_SENS,
            scroll_sens: DEFAULT_SCROLL_SENS,
            text_sens: DEFAULT_TEXT_SENS,
            ball_axis: DEFAULT_BALL_AXIS,
            mode: DEFAULT_MODE,
            flags: DEFAULT_FLAGS,
            auto_layer: DEFAULT_AUTO_LAYER,
        }
    }

    fn sanitized(mut self) -> Self {
        self.ball_dpi = self.ball_dpi.min(15);
        self.sniper_sens = self.sniper_sens.clamp(1, 255);
        self.scroll_sens = self.scroll_sens.clamp(1, 255);
        self.text_sens = self.text_sens.clamp(1, 255);
        self.ball_axis = self.ball_axis.min(3);
        self.mode = self.mode.min(3);
        self.auto_layer = self.auto_layer.min(15);
        self
    }
}

pub fn current() -> VialSettingsData {
    VialSettingsData {
        ball_dpi: BALL_DPI.load(Ordering::Relaxed),
        sniper_sens: SNIPER_SENS.load(Ordering::Relaxed),
        scroll_sens: SCROLL_SENS.load(Ordering::Relaxed),
        text_sens: TEXT_SENS.load(Ordering::Relaxed),
        ball_axis: BALL_AXIS.load(Ordering::Relaxed),
        mode: MODE.load(Ordering::Relaxed),
        flags: FLAGS.load(Ordering::Relaxed),
        auto_layer: AUTO_LAYER.load(Ordering::Relaxed),
    }
    .sanitized()
}

pub fn apply(settings: VialSettingsData) -> VialSettingsData {
    let settings = settings.sanitized();
    BALL_DPI.store(settings.ball_dpi, Ordering::Relaxed);
    SNIPER_SENS.store(settings.sniper_sens, Ordering::Relaxed);
    SCROLL_SENS.store(settings.scroll_sens, Ordering::Relaxed);
    TEXT_SENS.store(settings.text_sens, Ordering::Relaxed);
    BALL_AXIS.store(settings.ball_axis, Ordering::Relaxed);
    MODE.store(settings.mode, Ordering::Relaxed);
    FLAGS.store(settings.flags, Ordering::Relaxed);
    AUTO_LAYER.store(settings.auto_layer, Ordering::Relaxed);
    settings
}

pub fn reset() -> VialSettingsData {
    apply(VialSettingsData::defaults())
}

pub fn transform_axes(dx: i16, dy: i16) -> (i16, i16) {
    match BALL_AXIS.load(Ordering::Relaxed).min(3) {
        1 => (dy, -dx),
        2 => (-dx, -dy),
        3 => (-dy, dx),
        _ => (dx, dy),
    }
}

pub fn scale_by_ball_dpi(value: i32) -> i32 {
    let dpi = DPI_TABLE[BALL_DPI.load(Ordering::Relaxed).min(15) as usize];
    value * dpi / 1000
}

pub fn mode() -> u8 {
    MODE.load(Ordering::Relaxed).min(3)
}

pub fn set_mode(mode: u8) {
    MODE.store(mode.min(3), Ordering::Relaxed);
}

pub fn sticky_mode() -> bool {
    FLAGS.load(Ordering::Relaxed) & FLAG_STICKY_MODE != 0
}

pub fn sniper_sens() -> i16 {
    SNIPER_SENS.load(Ordering::Relaxed).max(1) as i16
}

pub fn scroll_sens() -> i32 {
    SCROLL_SENS.load(Ordering::Relaxed).max(1) as i32
}

pub fn text_sens() -> i32 {
    TEXT_SENS.load(Ordering::Relaxed).max(1) as i32
}

pub fn invert_scroll() -> bool {
    FLAGS.load(Ordering::Relaxed) & FLAG_INVERT_SCROLL != 0
}

pub fn invert_text() -> bool {
    FLAGS.load(Ordering::Relaxed) & FLAG_INVERT_TEXT != 0
}

pub fn auto_layer() -> u8 {
    AUTO_LAYER.load(Ordering::Relaxed).min(15)
}

pub fn auto_layer_enabled_for_mode(mode: u8) -> bool {
    if auto_layer() == 0 {
        return false;
    }
    let flags = FLAGS.load(Ordering::Relaxed);
    match mode.min(3) {
        1 => flags & FLAG_AUTO_LAYER_SNIPER != 0,
        2 => flags & FLAG_AUTO_LAYER_SCROLL != 0,
        3 => flags & FLAG_AUTO_LAYER_TEXT != 0,
        _ => true,
    }
}

pub fn get_setting(qsid: u16, out: &mut [u8]) -> Option<usize> {
    if out.is_empty() {
        return None;
    }

    let settings = current();
    out[0] = match qsid {
        QSID_BALL_DPI => settings.ball_dpi,
        QSID_SNIPER_SENS => settings.sniper_sens,
        QSID_SCROLL_SENS => settings.scroll_sens,
        QSID_TEXT_SENS => settings.text_sens,
        QSID_BALL_AXIS => settings.ball_axis,
        QSID_MODE => settings.mode,
        QSID_INVERT_SCROLL => u8::from(settings.flags & FLAG_INVERT_SCROLL != 0),
        QSID_ACCELERATION => u8::from(settings.flags & FLAG_ACCELERATION != 0),
        QSID_STICKY_MODE => u8::from(settings.flags & FLAG_STICKY_MODE != 0),
        QSID_AUTO_LAYER_NORMAL => u8::from(settings.flags & FLAG_AUTO_LAYER_NORMAL != 0),
        QSID_AUTO_LAYER => settings.auto_layer,
        QSID_AUTO_LAYER_SNIPER => u8::from(settings.flags & FLAG_AUTO_LAYER_SNIPER != 0),
        QSID_AUTO_LAYER_SCROLL => u8::from(settings.flags & FLAG_AUTO_LAYER_SCROLL != 0),
        QSID_AUTO_LAYER_TEXT => u8::from(settings.flags & FLAG_AUTO_LAYER_TEXT != 0),
        QSID_INVERT_TEXT => u8::from(settings.flags & FLAG_INVERT_TEXT != 0),
        _ => return None,
    };
    Some(1)
}

pub fn set_setting(qsid: u16, input: &[u8]) -> Option<VialSettingsData> {
    let value = *input.first()?;
    let mut settings = current();

    match qsid {
        QSID_BALL_DPI => settings.ball_dpi = value,
        QSID_SNIPER_SENS => settings.sniper_sens = value,
        QSID_SCROLL_SENS => settings.scroll_sens = value,
        QSID_TEXT_SENS => settings.text_sens = value,
        QSID_BALL_AXIS => settings.ball_axis = value,
        QSID_MODE => settings.mode = value,
        QSID_INVERT_SCROLL => set_flag(&mut settings.flags, FLAG_INVERT_SCROLL, value != 0),
        QSID_ACCELERATION => set_flag(&mut settings.flags, FLAG_ACCELERATION, value != 0),
        QSID_STICKY_MODE => set_flag(&mut settings.flags, FLAG_STICKY_MODE, value != 0),
        QSID_AUTO_LAYER_NORMAL => set_flag(&mut settings.flags, FLAG_AUTO_LAYER_NORMAL, value != 0),
        QSID_AUTO_LAYER => settings.auto_layer = value,
        QSID_AUTO_LAYER_SNIPER => set_flag(&mut settings.flags, FLAG_AUTO_LAYER_SNIPER, value != 0),
        QSID_AUTO_LAYER_SCROLL => set_flag(&mut settings.flags, FLAG_AUTO_LAYER_SCROLL, value != 0),
        QSID_AUTO_LAYER_TEXT => set_flag(&mut settings.flags, FLAG_AUTO_LAYER_TEXT, value != 0),
        QSID_INVERT_TEXT => set_flag(&mut settings.flags, FLAG_INVERT_TEXT, value != 0),
        _ => return None,
    }

    Some(apply(settings))
}

pub fn adjust_scroll_sens(delta: i8) {
    let mut settings = current();
    settings.scroll_sens = adjust(settings.scroll_sens, delta, 1, 255);
    apply(settings);
}

pub fn adjust_sniper_sens(delta: i8) {
    let mut settings = current();
    settings.sniper_sens = adjust(settings.sniper_sens, delta, 1, 255);
    apply(settings);
}

fn set_flag(flags: &mut u8, bit: u8, enabled: bool) {
    if enabled {
        *flags |= bit;
    } else {
        *flags &= !bit;
    }
}

fn adjust(value: u8, delta: i8, min: u8, max: u8) -> u8 {
    if delta.is_negative() {
        value.saturating_sub(delta.unsigned_abs()).max(min)
    } else {
        value.saturating_add(delta as u8).min(max)
    }
}
