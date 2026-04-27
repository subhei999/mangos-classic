#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_bytes_match_cmangos_layout() {
        assert_eq!(player_bytes(1, 2, 3, 4), 0x0403_0201);
    }

    #[test]
    fn starter_skill_values_match_basic_cmangos_ranges() {
        assert_eq!(starter_skill_value(Some("Language: Common")), (300, 300));
        assert_eq!(starter_skill_value(Some("Armor: Cloth")), (1, 1));
        assert_eq!(starter_skill_value(Some("Warrior: Arms")), (1, 5));
    }

    #[test]
    fn player_world_stats_apply_cmangos_stamina_and_intellect_bonuses() {
        let warrior = PlayerWorldStats {
            base_health: 20,
            base_mana: 0,
            stats: [23, 20, 22, 20, 21],
            next_level_xp: 400,
        };
        let mage = PlayerWorldStats {
            base_health: 31,
            base_mana: 100,
            stats: [15, 23, 19, 26, 22],
            next_level_xp: 400,
        };

        assert_eq!(warrior.max_health(), 60);
        assert_eq!(warrior.max_mana(), 0);
        assert_eq!(mage.max_health(), 50);
        assert_eq!(mage.max_mana(), 210);
    }

    #[test]
    fn human_warrior_outfit_matches_archived_cmangos_rows() {
        let items = starter_outfit_items(1, 1).unwrap();

        assert_eq!(items[0].item_id, 38);
        assert_eq!(items[0].slot, 3);
        assert!(items
            .iter()
            .any(|item| item.item_id == 25 && item.slot == 15));
        assert!(items
            .iter()
            .any(|item| item.item_id == 2362 && item.slot == 16));
    }

    #[test]
    fn non_human_starter_outfit_rows_cover_existing_race_class_pairs() {
        let cases: &[(u8, u8, u32, u8)] = &[
            (2, 1, 6125, 3),
            (2, 3, 127, 3),
            (3, 2, 45, 3),
            (4, 11, 6123, 4),
            (5, 8, 6096, 3),
            (7, 1, 38, 4),
            (8, 7, 6134, 3),
        ];

        for (race, class, item_id, slot) in cases {
            let items = starter_outfit_items(*race, *class)
                .unwrap_or_else(|| panic!("missing starter outfit for {race}/{class}"));
            assert!(items
                .iter()
                .any(|item| item.item_id == *item_id && item.slot == *slot));
            assert!(!items.is_empty());
        }
    }

    #[test]
    fn starter_item_template_refs_include_context_for_all_seeded_items() {
        let refs = starter_item_template_refs();
        assert!(refs.iter().any(|item| item.race == 1
            && item.class == 1
            && item.item_id == 25
            && item.slot == 15));
        assert!(refs
            .iter()
            .any(|item| item.race == 8 && item.class == 8 && item.item_id == 6948));
        assert!(refs.iter().all(|item| item.amount > 0));
    }

    #[test]
    fn starter_item_template_refs_replace_archived_custom_ids() {
        let refs = starter_item_template_refs();
        assert!(!refs
            .iter()
            .any(|item| matches!(item.item_id, 129 | 65020..=65027)));
        assert!(refs.iter().any(|item| item.race == 4
            && item.class == 3
            && item.slot == 7
            && item.item_id == 6127));
        assert!(refs.iter().any(|item| item.race == 3
            && item.class == 3
            && item.slot == 7
            && item.item_id == 6127));
        assert!(refs.iter().any(|item| item.item_id == 117));
        assert!(refs.iter().any(|item| item.item_id == 159));
        assert!(refs.iter().any(|item| item.item_id == 2947));
        assert!(refs.iter().any(|item| item.item_id == 25861));
    }

    #[test]
    fn equipment_cache_uses_item_id_enchant_pairs() {
        let mut equipment = [0u32; ENUM_EQUIPMENT_CACHE_SLOTS];
        equipment[3] = 38;
        equipment[15] = 25;

        let cache = format_equipment_cache(&equipment);

        assert!(cache.starts_with("0 0 0 0 0 0 38 0"));
        assert_eq!(
            cache.split_whitespace().count(),
            ENUM_EQUIPMENT_CACHE_SLOTS * 2
        );
    }
}
