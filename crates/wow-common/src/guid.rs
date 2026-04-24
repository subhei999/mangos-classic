use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::io::{self, Read, Write};

/// High-part GUID type identifiers for WoW 1.12.x.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u16)]
pub enum HighGuid {
    Player = 0x0000,
    Item = 0x4000,
    GameObject = 0xF110,
    Transport = 0xF120,
    Unit = 0xF130,
    Pet = 0xF140,
    DynamicObject = 0xF100,
    Corpse = 0xF101,
    MoTransport = 0x1FC0,
    Instance = 0x1F40,
    Group = 0x1F50,
}

impl HighGuid {
    /// Try to convert a raw `u16` high value to a known `HighGuid` variant.
    pub fn from_raw(value: u16) -> Option<Self> {
        match value {
            0x0000 => Some(Self::Player),
            0x4000 => Some(Self::Item),
            0xF110 => Some(Self::GameObject),
            0xF120 => Some(Self::Transport),
            0xF130 => Some(Self::Unit),
            0xF140 => Some(Self::Pet),
            0xF100 => Some(Self::DynamicObject),
            0xF101 => Some(Self::Corpse),
            0x1FC0 => Some(Self::MoTransport),
            0x1F40 => Some(Self::Instance),
            0x1F50 => Some(Self::Group),
            _ => None,
        }
    }
}

/// A 64-bit object GUID used throughout the WoW 1.12.x protocol.
///
/// Layout varies by type:
/// - Player/Item: `[high:16][unused:16][counter:32]`
/// - Unit/GameObject: `[high:16][entry:16][counter:32]` (entry is a 24-bit field in full layout)
///
/// For creature/gameobject GUIDs the full 64-bit layout is:
///   bits 48..63 = high type marker
///   bits 24..47 = entry (template id)
///   bits  0..23 = spawn counter
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct ObjectGuid(u64);

impl ObjectGuid {
    /// The empty/null GUID.
    pub const EMPTY: Self = Self(0);

    /// Create a GUID from raw parts.
    ///
    /// For creature/gameobject types, `entry` is the template id and `counter` is the spawn id.
    /// For player/item types, `entry` should be 0 and `counter` is the database id.
    pub fn new(high: HighGuid, entry: u32, counter: u32) -> Self {
        let h = (high as u16 as u64) << 48;
        let e = ((entry & 0x00FF_FFFF) as u64) << 24;
        let c = (counter & 0x00FF_FFFF) as u64;
        Self(h | e | c)
    }

    /// Create a GUID from a raw 64-bit value.
    pub fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// Return the raw 64-bit value.
    pub fn raw(self) -> u64 {
        self.0
    }

    /// Return `true` if the GUID is zero / empty.
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Extract the high-type portion (upper 16 bits).
    pub fn high_raw(self) -> u16 {
        (self.0 >> 48) as u16
    }

    /// Extract the `HighGuid` enum, if the high part matches a known type.
    pub fn high_type(self) -> Option<HighGuid> {
        HighGuid::from_raw(self.high_raw())
    }

    /// Extract the entry (template) id (bits 24..47).
    ///
    /// Only meaningful for creature / gameobject GUIDs.
    pub fn entry(self) -> u32 {
        ((self.0 >> 24) & 0x00FF_FFFF) as u32
    }

    /// Extract the counter / spawn id (bits 0..23).
    pub fn counter(self) -> u32 {
        (self.0 & 0x00FF_FFFF) as u32
    }

    // --- convenience type checks ---

    pub fn is_player(self) -> bool {
        self.high_raw() == HighGuid::Player as u16 && !self.is_empty()
    }

    pub fn is_creature(self) -> bool {
        self.high_raw() == HighGuid::Unit as u16
    }

    pub fn is_pet(self) -> bool {
        self.high_raw() == HighGuid::Pet as u16
    }

    pub fn is_game_object(self) -> bool {
        self.high_raw() == HighGuid::GameObject as u16
    }

    pub fn is_item(self) -> bool {
        self.high_raw() == HighGuid::Item as u16
    }

    pub fn is_transport(self) -> bool {
        self.high_raw() == HighGuid::Transport as u16
    }

    pub fn is_dynamic_object(self) -> bool {
        self.high_raw() == HighGuid::DynamicObject as u16
    }

    pub fn is_corpse(self) -> bool {
        self.high_raw() == HighGuid::Corpse as u16
    }

    pub fn is_group(self) -> bool {
        self.high_raw() == HighGuid::Group as u16
    }
}

impl fmt::Debug for ObjectGuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ObjectGuid(0x{:016X})", self.0)
    }
}

impl fmt::Display for ObjectGuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(high) = self.high_type() {
            write!(
                f,
                "{:?}(entry={}, counter={})",
                high,
                self.entry(),
                self.counter()
            )
        } else {
            write!(f, "Guid(0x{:016X})", self.0)
        }
    }
}

// ---------------------------------------------------------------------------
// Packed GUID — variable-length encoding used on the wire
// ---------------------------------------------------------------------------

/// A packed GUID is encoded as:
///   1 byte  — bitmask indicating which of the 8 bytes of the u64 are non-zero
///   N bytes — only the non-zero bytes, in order from least-significant
pub struct PackedGuid;

impl PackedGuid {
    /// Write a packed GUID into `writer`.
    pub fn write<W: Write>(writer: &mut W, guid: ObjectGuid) -> io::Result<()> {
        let value = guid.raw();
        let mut mask: u8 = 0;
        let mut bytes = [0u8; 8];
        let mut count = 0usize;

        for i in 0..8u8 {
            let byte = ((value >> (i * 8)) & 0xFF) as u8;
            if byte != 0 {
                mask |= 1 << i;
                bytes[count] = byte;
                count += 1;
            }
        }

        writer.write_u8(mask)?;
        writer.write_all(&bytes[..count])?;
        Ok(())
    }

    /// Read a packed GUID from `reader`.
    pub fn read<R: Read>(reader: &mut R) -> io::Result<ObjectGuid> {
        let mask = reader.read_u8()?;
        let mut value: u64 = 0;

        for i in 0..8u8 {
            if mask & (1 << i) != 0 {
                let byte = reader.read_u8()? as u64;
                value |= byte << (i * 8);
            }
        }

        Ok(ObjectGuid::from_raw(value))
    }

    /// Return the packed size in bytes (1 byte mask + non-zero byte count).
    pub fn packed_size(guid: ObjectGuid) -> usize {
        let value = guid.raw();
        let mut count = 1usize; // mask byte
        for i in 0..8u8 {
            if ((value >> (i * 8)) & 0xFF) != 0 {
                count += 1;
            }
        }
        count
    }
}

/// Convenience: write a full (non-packed) 8-byte GUID in little-endian.
pub fn write_guid<W: Write>(writer: &mut W, guid: ObjectGuid) -> io::Result<()> {
    writer.write_u64::<LittleEndian>(guid.raw())
}

/// Convenience: read a full (non-packed) 8-byte GUID in little-endian.
pub fn read_guid<R: Read>(reader: &mut R) -> io::Result<ObjectGuid> {
    let raw = reader.read_u64::<LittleEndian>()?;
    Ok(ObjectGuid::from_raw(raw))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn player_guid_round_trip() {
        let guid = ObjectGuid::new(HighGuid::Player, 0, 42);
        assert!(guid.is_player());
        assert!(!guid.is_creature());
        assert_eq!(guid.counter(), 42);
        assert_eq!(guid.entry(), 0);
        assert_eq!(guid.high_type(), Some(HighGuid::Player));
    }

    #[test]
    fn creature_guid_round_trip() {
        let guid = ObjectGuid::new(HighGuid::Unit, 1234, 5678);
        assert!(guid.is_creature());
        assert!(!guid.is_player());
        assert_eq!(guid.entry(), 1234);
        assert_eq!(guid.counter(), 5678);
    }

    #[test]
    fn empty_guid() {
        assert!(ObjectGuid::EMPTY.is_empty());
        assert!(!ObjectGuid::EMPTY.is_player());
    }

    #[test]
    fn packed_guid_round_trip() {
        let guid = ObjectGuid::new(HighGuid::Unit, 100, 7);
        let mut buf = Vec::new();
        PackedGuid::write(&mut buf, guid).unwrap();

        let mut cursor = Cursor::new(&buf);
        let decoded = PackedGuid::read(&mut cursor).unwrap();
        assert_eq!(guid, decoded);
    }

    #[test]
    fn packed_guid_empty() {
        let guid = ObjectGuid::EMPTY;
        let mut buf = Vec::new();
        PackedGuid::write(&mut buf, guid).unwrap();
        assert_eq!(buf, vec![0x00]); // mask only, no data bytes

        let mut cursor = Cursor::new(&buf);
        let decoded = PackedGuid::read(&mut cursor).unwrap();
        assert_eq!(guid, decoded);
    }

    #[test]
    fn full_guid_read_write() {
        let guid = ObjectGuid::new(HighGuid::GameObject, 555, 999);
        let mut buf = Vec::new();
        write_guid(&mut buf, guid).unwrap();
        assert_eq!(buf.len(), 8);

        let mut cursor = Cursor::new(&buf);
        let decoded = read_guid(&mut cursor).unwrap();
        assert_eq!(guid, decoded);
    }
}
