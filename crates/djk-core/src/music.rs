//! Musical values: tempo and key, with Camelot and standard notation.

use std::fmt;

/// Tempo in beats per minute.
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub struct Bpm(pub f32);

impl Bpm {
    /// The tempo heard when the track plays with the given pitch fraction.
    pub fn with_pitch(self, pitch: f32) -> Bpm {
        Bpm(self.0 * (1.0 + pitch))
    }
}

impl fmt::Display for Bpm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.1}", self.0)
    }
}

/// A pitch class, C = 0 … B = 11.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct PitchClass(pub u8);

impl PitchClass {
    const NAMES: [&'static str; 12] = [
        "C", "C♯", "D", "E♭", "E", "F", "F♯", "G", "A♭", "A", "B♭", "B",
    ];

    pub fn name(self) -> &'static str {
        Self::NAMES[usize::from(self.0 % 12)]
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Mode {
    Major,
    Minor,
}

/// A musical key.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Key {
    pub tonic: PitchClass,
    pub mode: Mode,
}

impl Key {
    pub fn new(tonic: u8, mode: Mode) -> Self {
        Self {
            tonic: PitchClass(tonic % 12),
            mode,
        }
    }

    /// Camelot wheel number (1–12) for this key.
    pub fn camelot_number(self) -> u8 {
        // Major keys: C = 8B, then +7 semitones per step around the wheel.
        // Minor keys: A minor = 8A, likewise.
        let reference = match self.mode {
            Mode::Major => 0, // C
            Mode::Minor => 9, // A
        };
        let semis = (i16::from(self.tonic.0) - reference).rem_euclid(12) as u8;
        // Steps of a fifth: multiply by 7 (inverse of 7 mod 12 is 7).
        let fifths = (semis * 7) % 12;
        (7 + fifths) % 12 + 1
    }

    /// Camelot notation, e.g. `8A`.
    pub fn camelot(self) -> String {
        let letter = match self.mode {
            Mode::Major => 'B',
            Mode::Minor => 'A',
        };
        format!("{}{letter}", self.camelot_number())
    }

    /// Standard notation, e.g. `A minor`.
    pub fn standard(self) -> String {
        let mode = match self.mode {
            Mode::Major => "major",
            Mode::Minor => "minor",
        };
        format!("{} {mode}", self.tonic.name())
    }

    /// Compact storage code: 0–11 major, 12–23 minor.
    pub fn to_code(self) -> u8 {
        self.tonic.0 + if self.mode == Mode::Minor { 12 } else { 0 }
    }

    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            0..=11 => Some(Self::new(code, Mode::Major)),
            12..=23 => Some(Self::new(code - 12, Mode::Minor)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camelot_mapping() {
        assert_eq!(Key::new(9, Mode::Minor).camelot(), "8A");
        assert_eq!(Key::new(0, Mode::Major).camelot(), "8B");
        assert_eq!(Key::new(7, Mode::Major).camelot(), "9B"); // G major
        assert_eq!(Key::new(5, Mode::Major).camelot(), "7B"); // F major
        assert_eq!(Key::new(4, Mode::Minor).camelot(), "9A"); // E minor
        assert_eq!(Key::new(11, Mode::Major).camelot(), "1B"); // B major
        assert_eq!(Key::new(8, Mode::Minor).camelot(), "1A"); // A♭ minor
        assert_eq!(Key::new(9, Mode::Minor).standard(), "A minor");
    }

    #[test]
    fn key_code_roundtrip() {
        for code in 0..24 {
            assert_eq!(Key::from_code(code).unwrap().to_code(), code);
        }
        assert!(Key::from_code(24).is_none());
    }

    #[test]
    fn bpm_with_pitch() {
        assert_eq!(format!("{}", Bpm(125.0).with_pitch(0.024)), "128.0");
    }
}
