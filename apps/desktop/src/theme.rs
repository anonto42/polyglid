use gpui::Rgba;

const fn color(hex: u32) -> Rgba {
    Rgba {
        r: ((hex >> 16) & 0xff) as f32 / 255.0,
        g: ((hex >> 8) & 0xff) as f32 / 255.0,
        b: (hex & 0xff) as f32 / 255.0,
        a: 1.0,
    }
}

pub const EDITOR: Rgba = color(0x24293b);
pub const TOP_BAR: Rgba = color(0x1c2132);
pub const EXPLORER: Rgba = color(0x202536);
pub const CHIP: Rgba = color(0x252b40);
pub const CHIP_BORDER: Rgba = color(0x303750);
pub const DARK_BORDER: Rgba = color(0x171b29);
pub const TEXT: Rgba = color(0xd7dbea);
pub const EXPLORER_TEXT: Rgba = color(0xb7c3e6);
pub const MUTED: Rgba = color(0x7885ad);
pub const BLUE: Rgba = color(0x77a5ff);
pub const PINK: Rgba = color(0xff5d80);
pub const GREEN: Rgba = color(0x9bdc69);
pub const ORANGE: Rgba = color(0xf4b35e);
pub const YELLOW: Rgba = color(0xe7c65a);
