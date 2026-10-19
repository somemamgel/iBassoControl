//! DC04 Pro wire protocol, reconstructed from the bundled iBasso UAC APK.
//! Pure encoding/decoding: no USB access and no implicit restoration of settings.

use std::{error::Error, fmt};

pub const VID: u16 = 0x262a;
pub const PID: u16 = 0x187f;
pub const PRODUCT: &str = "iBasso-DC04-Pro";
pub const VOLUME_TABLE: [u8; 101] = [
    255, 155, 150, 145, 140, 135, 130, 125, 120, 115, 110, 109, 108, 107, 106, 105, 104, 103, 102,
    101, 100, 99, 98, 97, 96, 95, 94, 93, 92, 91, 90, 88, 86, 84, 82, 80, 78, 76, 74, 72, 70, 68,
    66, 64, 62, 60, 58, 56, 54, 52, 50, 49, 48, 47, 46, 45, 44, 43, 42, 41, 40, 39, 38, 37, 36, 35,
    34, 33, 32, 31, 30, 29, 28, 27, 26, 25, 24, 23, 22, 21, 20, 19, 18, 17, 16, 15, 14, 13, 12, 11,
    10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolError(pub String);
impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for ProtocolError {}

/// Sixteen wire bytes. The OS-level zero report ID is added by the HID layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Report(pub [u8; 16]);
impl Report {
    pub fn id(self) -> u8 {
        self.0[0]
    }

    /// CS43131 reads use the previously selected MAP. Select its three address
    /// bytes and control byte without sending register data (datasheet 4.13.1).
    pub fn read_preamble(self) -> Option<Self> {
        if self.0[2] != 0xc8 {
            return None;
        }
        let mut bytes = self.0;
        bytes[0] = 0x52;
        bytes[1] = 0x11;
        bytes[2] = 0x88;
        bytes[6] = 4;
        bytes[11] = 0;
        Some(Self(bytes))
    }
}

fn extended(id: u8, address: u8, register: u8, offset: u8, value: Option<u8>) -> Report {
    let mut bytes = [0; 16];
    bytes[0] = id;
    bytes[1] = if value.is_some() { 0x11 } else { 0x12 };
    bytes[2] = if value.is_some() { 0x88 } else { 0xc8 };
    bytes[3] = address;
    bytes[6] = if value.is_some() { 5 } else { 4 };
    bytes[7] = register;
    bytes[9] = offset;
    bytes[11] = value.unwrap_or(0xff);
    Report(bytes)
}

fn medium(id: u8, offset: u8, value: Option<u8>) -> Report {
    let mut bytes = [0; 16];
    bytes[0] = id;
    bytes[1] = if value.is_some() { 0x11 } else { 0x12 };
    bytes[2] = if value.is_some() { 0xa0 } else { 0xe4 };
    bytes[3] = 0xa2;
    bytes[5] = offset;
    bytes[6] = 1;
    bytes[7] = value.unwrap_or(0);
    Report(bytes)
}

fn short(id: u8, register: u8, value: Option<u8>) -> Report {
    let mut bytes = [0; 16];
    bytes[0] = id;
    bytes[1] = if value.is_some() { 0x11 } else { 0x12 };
    bytes[2] = if value.is_some() { 0x98 } else { 0xdc };
    bytes[3] = 0x2c;
    bytes[4] = register;
    bytes[6] = 1;
    bytes[7] = value.unwrap_or(0);
    Report(bytes)
}

#[derive(Debug, Clone, Copy)]
pub enum Query {
    Firmware,
    VolumeLeft,
    VolumeRight,
    FilterLeft,
    FilterRight,
    GainLeft,
    GainRight,
    OutputLeft,
    OutputRight,
    Eco,
}
impl Query {
    pub fn report(self) -> Report {
        match self {
            Self::Firmware => short(0x58, 4, None),
            Self::VolumeLeft => medium(0x41, 0x10, None),
            Self::VolumeRight => medium(0x45, 0x11, None),
            Self::FilterLeft => extended(0x4b, 0x60, 9, 0, None),
            Self::FilterRight => extended(0x4c, 0x62, 9, 0, None),
            Self::GainLeft => extended(0x54, 0x60, 8, 0, None),
            Self::GainRight => extended(0x55, 0x62, 8, 0, None),
            Self::OutputLeft => extended(0x56, 0x60, 11, 0, None),
            Self::OutputRight => extended(0x57, 0x62, 11, 0, None),
            Self::Eco => short(0x51, 2, None),
        }
    }
}

/// Names follow the APK's click handler (its initial display mapping is inconsistent).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    D1,
    D2,
    D3,
    D4,
    D5,
}
impl Filter {
    pub const ALL: [Self; 5] = [Self::D1, Self::D2, Self::D3, Self::D4, Self::D5];
    pub fn raw(self) -> u8 {
        match self {
            Self::D1 => 0x42,
            Self::D2 => 0x82,
            Self::D3 => 0x02,
            Self::D4 => 0xc2,
            Self::D5 => 0x22,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::D1 => "d1",
            Self::D2 => "d2",
            Self::D3 => "d3",
            Self::D4 => "d4",
            Self::D5 => "d5",
        }
    }
    pub fn from_raw(raw: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.raw() == raw)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Gain {
    Low,
    Medium,
    High,
}
impl Gain {
    pub fn raw(self) -> u8 {
        match self {
            Self::Low => 0,
            Self::Medium => 0x20,
            Self::High => 0x31,
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub enum Output {
    Normal,
    Turbo,
}
impl Output {
    pub fn raw(self) -> u8 {
        match self {
            Self::Normal => 0x1c,
            Self::Turbo => 0x1e,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Setting {
    /// APK scale 0..=100; signed balance -10..=10 follows the APK register calculation.
    Volume {
        level: u8,
        balance: i8,
    },
    Filter(Filter),
    Gain(Gain),
    Output(Output),
    Eco(bool),
}

#[derive(Debug, Clone, Copy)]
pub struct WriteStep {
    pub write: Report,
    pub readback: Report,
    pub expected: u8,
}

fn codec_step(id: u8, address: u8, register: u8, offset: u8, value: u8) -> WriteStep {
    WriteStep {
        write: extended(id, address, register, offset, Some(value)),
        readback: extended(0x51, address, register, offset, None),
        expected: value,
    }
}

fn mirror_step(id: u8, offset: u8, value: u8) -> WriteStep {
    WriteStep {
        write: medium(id, offset, Some(value)),
        readback: medium(0x51, offset, None),
        expected: value,
    }
}

impl Setting {
    /// Validates the entire operation before any device I/O is performed.
    pub fn plan(self) -> Result<Vec<WriteStep>, ProtocolError> {
        let mut steps = Vec::new();
        match self {
            Self::Volume { level, balance } => {
                if level > 100 || !(-10..=10).contains(&balance) {
                    return Err(ProtocolError(
                        "volume must be 0..100 and balance -10..10".into(),
                    ));
                }
                let base = VOLUME_TABLE[usize::from(level)];
                let mut left = base;
                let mut right = base;
                if base != 255 {
                    if balance < 0 {
                        left = base.saturating_sub(balance.unsigned_abs());
                    } else {
                        right = base.saturating_sub(balance as u8);
                    }
                }
                // PCM registers first, then DSD registers and persistent volume mirrors,
                // in exactly the order used by enqueueStereoVolumeCommands in the APK.
                let ids = if balance == 0 {
                    [1, 2, 3, 4, 9, 10, 11, 12]
                } else {
                    [5, 6, 7, 8, 13, 14, 15, 16]
                };
                steps.extend([
                    codec_step(ids[0], 0x60, 9, 1, left),
                    codec_step(ids[1], 0x60, 9, 2, left),
                    codec_step(ids[2], 0x62, 9, 1, right),
                    codec_step(ids[3], 0x62, 9, 2, right),
                    codec_step(ids[4], 0x60, 7, 0, left),
                    codec_step(ids[5], 0x60, 7, 1, left),
                    mirror_step(0x13, 0x10, left),
                    codec_step(ids[6], 0x62, 7, 0, right),
                    codec_step(ids[7], 0x62, 7, 1, right),
                    mirror_step(0x14, 0x11, right),
                ]);
            }
            Self::Filter(filter) => steps.extend([
                codec_step(0x11, 0x60, 9, 0, filter.raw()),
                codec_step(0x12, 0x62, 9, 0, filter.raw()),
            ]),
            Self::Gain(gain) => steps.extend([
                codec_step(0x15, 0x60, 8, 0, gain.raw()),
                codec_step(0x16, 0x62, 8, 0, gain.raw()),
            ]),
            Self::Output(output) => steps.extend([
                codec_step(0x17, 0x60, 11, 0, output.raw()),
                codec_step(0x18, 0x62, 11, 0, output.raw()),
            ]),
            Self::Eco(enabled) => {
                let value = if enabled { 0x20 } else { 0 };
                steps.push(WriteStep {
                    write: short(0x19, 2, Some(value)),
                    readback: Query::Eco.report(),
                    expected: value,
                });
                steps.push(mirror_step(0x20, 0x12, value));
            }
        }
        Ok(steps)
    }
}

/// Matches a 32-byte DC04 Pro input report. Ignores unrelated/native notifications.
/// hidapi strips the unnumbered input report's OS-level zero ID.
pub fn response_data(bytes: &[u8], expected_id: u8) -> Result<Option<&[u8]>, ProtocolError> {
    if bytes.len() < 8 {
        return Err(ProtocolError(format!(
            "short HID response: {} bytes",
            bytes.len()
        )));
    }
    if bytes[4] == 0xfe && bytes[5] == 1 {
        return Ok(None);
    }
    if bytes[6] != expected_id {
        return Ok(None);
    }
    if bytes[..6] != [0; 6] {
        return Err(ProtocolError("unexpected response header".into()));
    }
    let length = usize::from(bytes[7]);
    bytes
        .get(8..8 + length)
        .map(Some)
        .ok_or_else(|| ProtocolError("truncated response payload".into()))
}

pub fn volume_level(raw: u8) -> Option<u8> {
    VOLUME_TABLE
        .iter()
        .position(|&value| value == raw)
        .map(|i| i as u8)
}

/// Validate the effective data length of a single register read.
pub fn response_value(report: Report, data: &[u8]) -> Result<u8, ProtocolError> {
    let expected = if report.0[2] == 0xc8 { 4 } else { 1 };
    if data.len() != expected {
        return Err(ProtocolError(format!(
            "Command 0x{:02x}: expected {expected} bytes, received {}",
            report.id(),
            data.len()
        )));
    }
    Ok(data[0])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn golden_apk_packets() {
        assert_eq!(
            Query::Firmware.report().0,
            [0x58, 0x12, 0xdc, 0x2c, 4, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(
            Query::FilterLeft.report().0,
            [
                0x4b, 0x12, 0xc8, 0x60, 0, 0, 4, 9, 0, 0, 0, 0xff, 0, 0, 0, 0
            ]
        );
        assert_eq!(
            Setting::Gain(Gain::High).plan().unwrap()[0].write.0,
            [
                0x15, 0x11, 0x88, 0x60, 0, 0, 5, 8, 0, 0, 0, 0x31, 0, 0, 0, 0
            ]
        );
        assert_eq!(
            Setting::Eco(true).plan().unwrap()[1].write.0,
            [
                0x20, 0x11, 0xa0, 0xa2, 0, 0x12, 1, 0x20, 0, 0, 0, 0, 0, 0, 0, 0
            ]
        );
    }
    #[test]
    fn volume_boundaries_and_mute_balance() {
        assert!(
            Setting::Volume {
                level: 101,
                balance: 0
            }
            .plan()
            .is_err()
        );
        assert!(
            Setting::Volume {
                level: 20,
                balance: -11
            }
            .plan()
            .is_err()
        );
        for balance in -10..=10 {
            assert!(
                Setting::Volume { level: 0, balance }
                    .plan()
                    .unwrap()
                    .iter()
                    .all(|s| s.expected == 255)
            );
            assert!(
                Setting::Volume {
                    level: 100,
                    balance
                }
                .plan()
                .unwrap()
                .iter()
                .all(|s| s.expected == 0)
            );
        }
        let steps = Setting::Volume {
            level: 50,
            balance: 5,
        }
        .plan()
        .unwrap();
        assert_eq!(steps.len(), 10);
        assert_eq!((steps[0].expected, steps[2].expected), (50, 45));
        assert_eq!(
            steps[6].write.0,
            [
                0x13, 0x11, 0xa0, 0xa2, 0, 0x10, 1, 50, 0, 0, 0, 0, 0, 0, 0, 0
            ]
        );
        for level in 0..=100 {
            assert_eq!(volume_level(VOLUME_TABLE[level as usize]), Some(level));
        }
        assert_eq!(volume_level(254), None);
    }
    #[test]
    fn response_validation() {
        let mut reply = [0; 32];
        reply[6] = 0x58;
        reply[7] = 1;
        reply[8] = 0x58;
        assert_eq!(response_data(&reply, 0x58).unwrap(), Some(&[0x58][..]));
        assert_eq!(response_data(&reply, 0x41).unwrap(), None);
        reply[7] = 25;
        assert!(response_data(&reply, 0x58).is_err());
        assert!(response_data(&[0; 7], 0x58).is_err());
        reply[4] = 0xfe;
        reply[5] = 1;
        assert_eq!(response_data(&reply, 0x58).unwrap(), None);
    }
}
