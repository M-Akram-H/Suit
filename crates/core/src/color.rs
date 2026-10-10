use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};

/// 8-bit sRGB with alpha. Written `#rrggbb` or `#rrggbbaa`, as Manim writes colours.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Rgba(pub [u8; 4]);

impl Rgba {
    pub const BLACK: Rgba = Rgba([0, 0, 0, 255]);
    pub const WHITE: Rgba = Rgba([255, 255, 255, 255]);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Rgba {
        Rgba([r, g, b, 255])
    }

    pub fn parse(s: &str) -> Option<Rgba> {
        let hex = s.strip_prefix('#')?;
        if !hex.is_ascii() || (hex.len() != 6 && hex.len() != 8) {
            return None;
        }
        let byte = |i: usize| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).ok();
        let alpha = if hex.len() == 8 { byte(3)? } else { 255 };
        Some(Rgba([byte(0)?, byte(1)?, byte(2)?, alpha]))
    }

    /// `#rrggbb`, dropping the alpha.
    pub fn hex_rgb(self) -> String {
        let [r, g, b, _] = self.0;
        format!("#{r:02x}{g:02x}{b:02x}")
    }

    pub fn hex(self) -> String {
        match self.0[3] {
            255 => self.hex_rgb(),
            a => format!("{}{a:02x}", self.hex_rgb()),
        }
    }
}

impl Serialize for Rgba {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.hex())
    }
}

impl<'de> Deserialize<'de> for Rgba {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Rgba::parse(&s).ok_or_else(|| D::Error::custom(format!("not a colour: {s:?}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trip() {
        assert_eq!(Rgba::parse("#58C4DD"), Some(Rgba::rgb(0x58, 0xc4, 0xdd)));
        assert_eq!(Rgba::parse("#00000080").unwrap().hex(), "#00000080");
        assert_eq!(Rgba::WHITE.hex(), "#ffffff");
        assert_eq!(Rgba::parse("58c4dd"), None);
        assert_eq!(Rgba::parse("#58c4d"), None);
    }
}
