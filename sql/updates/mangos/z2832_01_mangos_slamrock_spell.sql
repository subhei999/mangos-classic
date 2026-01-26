-- Detect and update the latest db_version column
SET @prev_column = NULL;
SELECT COLUMN_NAME INTO @prev_column 
FROM INFORMATION_SCHEMA.COLUMNS 
WHERE TABLE_SCHEMA = DATABASE() 
  AND TABLE_NAME = 'db_version' 
  AND COLUMN_NAME LIKE 'required_z%'
ORDER BY COLUMN_NAME DESC 
LIMIT 1;

SET @sql = IFNULL(
  CONCAT('ALTER TABLE db_version CHANGE COLUMN ', @prev_column, ' required_z2832_01_mangos_slamrock_spell bit'),
  'ALTER TABLE db_version ADD COLUMN required_z2832_01_mangos_slamrock_spell bit'
);
PREPARE stmt FROM @sql;
EXECUTE stmt;
DEALLOCATE PREPARE stmt;

-- Create custom spell for Slamrock item targeting (allows both weapons and armor)
-- Based on spell 2828 (Sharpen Blade) but with EquippedItemClass = -1 to allow any item class
-- This spell is used by the Slamrock item script for client-side targeting cursor
-- Delete existing entry if it exists, then insert (idempotent)
DELETE FROM `spell_template` WHERE `Id` = 33394;

INSERT INTO `spell_template` (`Id`, `School`, `Category`, `CastUI`, `Dispel`, `Mechanic`, `Attributes`, `AttributesEx`, `AttributesEx2`, `AttributesEx3`, `AttributesEx4`, `Stances`, `StancesNot`, `Targets`, `TargetCreatureType`, `RequiresSpellFocus`, `CasterAuraState`, `TargetAuraState`, `CastingTimeIndex`, `RecoveryTime`, `CategoryRecoveryTime`, `InterruptFlags`, `AuraInterruptFlags`, `ChannelInterruptFlags`, `ProcFlags`, `ProcChance`, `ProcCharges`, `MaxLevel`, `BaseLevel`, `SpellLevel`, `DurationIndex`, `PowerType`, `ManaCost`, `ManaCostPerlevel`, `ManaPerSecond`, `ManaPerSecondPerLevel`, `RangeIndex`, `Speed`, `ModalNextSpell`, `StackAmount`, `Totem1`, `Totem2`, `Reagent1`, `Reagent2`, `Reagent3`, `Reagent4`, `Reagent5`, `Reagent6`, `Reagent7`, `Reagent8`, `ReagentCount1`, `ReagentCount2`, `ReagentCount3`, `ReagentCount4`, `ReagentCount5`, `ReagentCount6`, `ReagentCount7`, `ReagentCount8`, `EquippedItemClass`, `EquippedItemSubClassMask`, `EquippedItemInventoryTypeMask`, `Effect1`, `Effect2`, `Effect3`, `EffectDieSides1`, `EffectDieSides2`, `EffectDieSides3`, `EffectBaseDice1`, `EffectBaseDice2`, `EffectBaseDice3`, `EffectDicePerLevel1`, `EffectDicePerLevel2`, `EffectDicePerLevel3`, `EffectRealPointsPerLevel1`, `EffectRealPointsPerLevel2`, `EffectRealPointsPerLevel3`, `EffectBasePoints1`, `EffectBasePoints2`, `EffectBasePoints3`, `EffectMechanic1`, `EffectMechanic2`, `EffectMechanic3`, `EffectImplicitTargetA1`, `EffectImplicitTargetA2`, `EffectImplicitTargetA3`, `EffectImplicitTargetB1`, `EffectImplicitTargetB2`, `EffectImplicitTargetB3`, `EffectRadiusIndex1`, `EffectRadiusIndex2`, `EffectRadiusIndex3`, `EffectApplyAuraName1`, `EffectApplyAuraName2`, `EffectApplyAuraName3`, `EffectAmplitude1`, `EffectAmplitude2`, `EffectAmplitude3`, `EffectMultipleValue1`, `EffectMultipleValue2`, `EffectMultipleValue3`, `EffectChainTarget1`, `EffectChainTarget2`, `EffectChainTarget3`, `EffectItemType1`, `EffectItemType2`, `EffectItemType3`, `EffectMiscValue1`, `EffectMiscValue2`, `EffectMiscValue3`, `EffectTriggerSpell1`, `EffectTriggerSpell2`, `EffectTriggerSpell3`, `EffectPointsPerComboPoint1`, `EffectPointsPerComboPoint2`, `EffectPointsPerComboPoint3`, `SpellVisual`, `SpellIconID`, `ActiveIconID`, `SpellPriority`, `SpellName`, `SpellName2`, `SpellName3`, `SpellName4`, `SpellName5`, `SpellName6`, `SpellName7`, `SpellName8`, `Rank1`, `Rank2`, `Rank3`, `Rank4`, `Rank5`, `Rank6`, `Rank7`, `Rank8`, `ManaCostPercentage`, `StartRecoveryCategory`, `StartRecoveryTime`, `MaxTargetLevel`, `SpellFamilyName`, `SpellFamilyFlags`, `MaxAffectedTargets`, `DmgClass`, `PreventionType`, `StanceBarOrder`, `DmgMultiplier1`, `DmgMultiplier2`, `DmgMultiplier3`, `MinFactionId`, `MinReputation`, `RequiredAuraVision`, `EffectBonusCoefficient1`, `EffectBonusCoefficient2`, `EffectBonusCoefficient3`, `EffectBonusCoefficientFromAP1`, `EffectBonusCoefficientFromAP2`, `EffectBonusCoefficientFromAP3`, `IsServerSide`, `AttributesServerside`) VALUES (33394, 0, 0, 0, 0, 0, 262144, 0, 0, 0, 0, 0, 0, 16, 0, 0, 0, 0, 14, 0, 0, 15, 0, 0, 0, 101, 0, 0, 0, 5, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -1, 0, 0, 54, 0, 0, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1799, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 40, 0, 0, 0, 0, 0, 0, 0, 0, 3324, 1, 0, 0, 'Slam Item', '', '', '', '', '', '', '', 'Rank 1', '', '', '', '', '', '', '', 0, 0, 0, 0, 0, 0, 0, 0, 0, -1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0);
