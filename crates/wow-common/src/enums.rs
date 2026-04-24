use serde::{Deserialize, Serialize};

use crate::error::CommonError;

// ---------------------------------------------------------------------------
// Race
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Race {
    Human = 1,
    Orc = 2,
    Dwarf = 3,
    NightElf = 4,
    Undead = 5,
    Tauren = 6,
    Gnome = 7,
    Troll = 8,
}

impl TryFrom<u8> for Race {
    type Error = CommonError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Human),
            2 => Ok(Self::Orc),
            3 => Ok(Self::Dwarf),
            4 => Ok(Self::NightElf),
            5 => Ok(Self::Undead),
            6 => Ok(Self::Tauren),
            7 => Ok(Self::Gnome),
            8 => Ok(Self::Troll),
            _ => Err(CommonError::InvalidEnumValue {
                enum_name: "Race",
                value: value as u64,
            }),
        }
    }
}

impl Race {
    /// Returns `true` if this race belongs to the Alliance faction.
    pub fn is_alliance(self) -> bool {
        matches!(
            self,
            Self::Human | Self::Dwarf | Self::NightElf | Self::Gnome
        )
    }

    /// Returns `true` if this race belongs to the Horde faction.
    pub fn is_horde(self) -> bool {
        matches!(self, Self::Orc | Self::Undead | Self::Tauren | Self::Troll)
    }
}

// ---------------------------------------------------------------------------
// Class
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Class {
    Warrior = 1,
    Paladin = 2,
    Hunter = 3,
    Rogue = 4,
    Priest = 5,
    Shaman = 7,
    Mage = 8,
    Warlock = 9,
    Druid = 11,
}

impl TryFrom<u8> for Class {
    type Error = CommonError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Warrior),
            2 => Ok(Self::Paladin),
            3 => Ok(Self::Hunter),
            4 => Ok(Self::Rogue),
            5 => Ok(Self::Priest),
            7 => Ok(Self::Shaman),
            8 => Ok(Self::Mage),
            9 => Ok(Self::Warlock),
            11 => Ok(Self::Druid),
            _ => Err(CommonError::InvalidEnumValue {
                enum_name: "Class",
                value: value as u64,
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Gender
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Gender {
    Male = 0,
    Female = 1,
}

impl TryFrom<u8> for Gender {
    type Error = CommonError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Male),
            1 => Ok(Self::Female),
            _ => Err(CommonError::InvalidEnumValue {
                enum_name: "Gender",
                value: value as u64,
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Power
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Power {
    Mana = 0,
    Rage = 1,
    Focus = 2,
    Energy = 3,
    Happiness = 4,
}

impl TryFrom<u8> for Power {
    type Error = CommonError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Mana),
            1 => Ok(Self::Rage),
            2 => Ok(Self::Focus),
            3 => Ok(Self::Energy),
            4 => Ok(Self::Happiness),
            _ => Err(CommonError::InvalidEnumValue {
                enum_name: "Power",
                value: value as u64,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn race_try_from_valid() {
        assert_eq!(Race::try_from(1).unwrap(), Race::Human);
        assert_eq!(Race::try_from(8).unwrap(), Race::Troll);
    }

    #[test]
    fn race_try_from_invalid() {
        assert!(Race::try_from(0).is_err());
        assert!(Race::try_from(9).is_err());
    }

    #[test]
    fn race_faction() {
        assert!(Race::Human.is_alliance());
        assert!(!Race::Human.is_horde());
        assert!(Race::Orc.is_horde());
        assert!(!Race::Orc.is_alliance());
    }

    #[test]
    fn class_try_from_valid() {
        assert_eq!(Class::try_from(1).unwrap(), Class::Warrior);
        assert_eq!(Class::try_from(7).unwrap(), Class::Shaman);
        assert_eq!(Class::try_from(11).unwrap(), Class::Druid);
    }

    #[test]
    fn class_try_from_skipped_values() {
        assert!(Class::try_from(6).is_err());
        assert!(Class::try_from(10).is_err());
    }

    #[test]
    fn gender_try_from() {
        assert_eq!(Gender::try_from(0).unwrap(), Gender::Male);
        assert_eq!(Gender::try_from(1).unwrap(), Gender::Female);
        assert!(Gender::try_from(2).is_err());
    }

    #[test]
    fn power_try_from() {
        assert_eq!(Power::try_from(0).unwrap(), Power::Mana);
        assert_eq!(Power::try_from(4).unwrap(), Power::Happiness);
        assert!(Power::try_from(5).is_err());
    }
}
