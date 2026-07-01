// garlemald-server — Rust port of a FINAL FANTASY XIV v1.23b server emulator (lobby/world/map)
// Copyright (C) 2026  Samuel Stegall
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published
// by the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.
//
// SPDX-License-Identifier: AGPL-3.0-or-later

//! [`RegionalLeveResolver`] — the read-only fieldcraft/battlecraft
//! catalog. Loaded once at boot from `gamedata_regional_leves`, shared
//! across Lua VMs via an `Arc` on [`crate::lua::catalogs::Catalogs`].
//!
//! Two secondary indexes are populated alongside the primary
//! `by_id` map:
//!
//! * `by_fieldcraft_target: HashMap<u32, Vec<u32>>` — item catalog id
//!   → leve ids whose *band-0* objective targets that item. Band-0
//!   targets are used as the index key because every seeded leve
//!   today uses the same target across all four bands (they differ
//!   only in quantity); a follow-up refactor can widen this to every
//!   band if a seeded leve ever varies the target per-band.
//! * `by_battlecraft_target: HashMap<u32, Vec<u32>>` — analogous but
//!   keyed by BattleNpc actor-class id.
//! * `by_plate_id: HashMap<u32, u32>` — guildleve UI *plate* (card) id
//!   → leve id. The levemete NPC script (`PopulaceGuildlevePublisher`)
//!   renders a grid of hardcoded plate/card ids; this index maps the
//!   card the player clicks back to the catalog leve so the accept /
//!   hand-in branches can drive `player:AcceptRegionalLeve` /
//!   `HandInRegionalLeve`. Plate id is unique per leve (a card renders
//!   exactly one leve), so unlike the target indexes it maps to a
//!   single id rather than a `Vec`.
//!
//! The secondary indexes let the progress hooks answer "which active
//! leves care about *this* item / *this* killed mob?" in O(1) per
//! event instead of walking every active leve on every tick.

use std::collections::HashMap;

use super::data::{LeveType, RegionalLeveData};

#[derive(Debug, Default)]
pub struct RegionalLeveResolver {
    by_id: HashMap<u32, RegionalLeveData>,
    by_fieldcraft_target: HashMap<u32, Vec<u32>>,
    by_battlecraft_target: HashMap<u32, Vec<u32>>,
    by_plate_id: HashMap<u32, u32>,
}

impl RegionalLeveResolver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_rows(rows: impl IntoIterator<Item = RegionalLeveData>) -> Self {
        let mut this = Self::new();
        for r in rows {
            this.insert(r);
        }
        this
    }

    pub fn insert(&mut self, row: RegionalLeveData) {
        let id = row.id;
        // Band-0 target is the canonical index key (see module doc).
        let t0 = row.objective_target_id[0];
        if t0 > 0 {
            let t0 = t0 as u32;
            match row.leve_type {
                LeveType::Fieldcraft => {
                    self.by_fieldcraft_target.entry(t0).or_default().push(id);
                }
                LeveType::Battlecraft => {
                    self.by_battlecraft_target.entry(t0).or_default().push(id);
                }
            }
        }
        // Plate id 0 means "no UI card assigned" (the DB default);
        // skip it so unmapped leves don't collide on key 0.
        if row.plate_id > 0 {
            self.by_plate_id.insert(row.plate_id, id);
        }
        self.by_id.insert(id, row);
    }

    pub fn by_id(&self, leve_id: u32) -> Option<&RegionalLeveData> {
        self.by_id.get(&leve_id)
    }

    /// Resolve a guildleve UI *plate* / card id (e.g. `0x30C3`) to its
    /// catalog leve. Backs the levemete script's card-selection ->
    /// accept / hand-in flow. `None` when no seeded leve carries that
    /// plate id (unmapped card, or plate id 0).
    pub fn leve_for_plate(&self, plate_id: u32) -> Option<&RegionalLeveData> {
        let id = self.by_plate_id.get(&plate_id)?;
        self.by_id.get(id)
    }

    /// Leve ids that target `item_catalog_id` as a fieldcraft
    /// objective. Empty slice if no leve targets this item.
    pub fn fieldcraft_leves_for_item(&self, item_catalog_id: u32) -> &[u32] {
        self.by_fieldcraft_target
            .get(&item_catalog_id)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Leve ids that target `actor_class_id` as a battlecraft
    /// objective.
    pub fn battlecraft_leves_for_class(&self, actor_class_id: u32) -> &[u32] {
        self.by_battlecraft_target
            .get(&actor_class_id)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    pub fn num_leves(&self) -> usize {
        self.by_id.len()
    }

    pub fn num_fieldcraft(&self) -> usize {
        self.by_id
            .values()
            .filter(|r| r.leve_type == LeveType::Fieldcraft)
            .count()
    }

    pub fn num_battlecraft(&self) -> usize {
        self.by_id
            .values()
            .filter(|r| r.leve_type == LeveType::Battlecraft)
            .count()
    }

    pub fn iter(&self) -> impl Iterator<Item = &RegionalLeveData> {
        self.by_id.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk(id: u32, ty: LeveType, target: i32) -> RegionalLeveData {
        mk_plated(id, ty, target, 0)
    }

    fn mk_plated(id: u32, ty: LeveType, target: i32, plate_id: u32) -> RegionalLeveData {
        RegionalLeveData {
            id,
            leve_type: ty,
            plate_id,
            border_id: 0,
            recommended_class: 0,
            issuing_location: 0,
            leve_location: 0,
            delivery_display_name: 0,
            region: 0,
            objective_target_id: [target; 4],
            objective_quantity: [5, 10, 15, 20],
            recommended_level: [1, 10, 20, 30],
            reward_item_id: [0; 4],
            reward_quantity: [0; 4],
            reward_gil: [100, 200, 400, 800],
        }
    }

    #[test]
    fn resolver_indexes_by_type_and_target() {
        let r = RegionalLeveResolver::from_rows([
            mk(130_001, LeveType::Fieldcraft, 10_001_006),
            mk(130_002, LeveType::Fieldcraft, 10_008_007),
            mk(140_001, LeveType::Battlecraft, 5_000_091),
        ]);
        assert_eq!(r.num_leves(), 3);
        assert_eq!(r.num_fieldcraft(), 2);
        assert_eq!(r.num_battlecraft(), 1);

        let for_copper = r.fieldcraft_leves_for_item(10_001_006);
        assert_eq!(for_copper, &[130_001]);
        let for_walnut = r.fieldcraft_leves_for_item(10_008_007);
        assert_eq!(for_walnut, &[130_002]);
        assert!(r.fieldcraft_leves_for_item(99999).is_empty());
        let for_drake = r.battlecraft_leves_for_class(5_000_091);
        assert_eq!(for_drake, &[140_001]);
    }

    #[test]
    fn overlapping_targets_accumulate_ids() {
        let r = RegionalLeveResolver::from_rows([
            mk(130_001, LeveType::Fieldcraft, 10_001_006),
            mk(130_010, LeveType::Fieldcraft, 10_001_006),
        ]);
        let mut ids = r.fieldcraft_leves_for_item(10_001_006).to_vec();
        ids.sort();
        assert_eq!(ids, vec![130_001, 130_010]);
    }

    #[test]
    fn fieldcraft_target_does_not_appear_in_battlecraft_index() {
        let r = RegionalLeveResolver::from_rows([mk(130_001, LeveType::Fieldcraft, 10_001_006)]);
        assert!(r.battlecraft_leves_for_class(10_001_006).is_empty());
    }

    #[test]
    fn resolver_maps_plate_id_to_leve() {
        // Two battlecraft leves with distinct UI card ids (0x30C3,
        // 0x30C4) plus one leve with no plate (0) that must not
        // register a plate-index entry.
        let r = RegionalLeveResolver::from_rows([
            mk_plated(140_001, LeveType::Battlecraft, 5_000_035, 0x30C3),
            mk_plated(140_002, LeveType::Battlecraft, 5_000_076, 0x30C4),
            mk_plated(130_001, LeveType::Fieldcraft, 10_001_006, 0),
        ]);
        assert_eq!(r.leve_for_plate(0x30C3).map(|d| d.id), Some(140_001));
        assert_eq!(r.leve_for_plate(0x30C4).map(|d| d.id), Some(140_002));
        assert!(r.leve_for_plate(0x30C1).is_none(), "unmapped card -> none");
        assert!(r.leve_for_plate(0).is_none(), "plate 0 never indexed");
    }

    #[test]
    fn zero_target_is_skipped_in_indexing() {
        let r = RegionalLeveResolver::from_rows([mk(130_001, LeveType::Fieldcraft, 0)]);
        assert_eq!(r.num_leves(), 1);
        assert!(r.fieldcraft_leves_for_item(0).is_empty());
    }
}
