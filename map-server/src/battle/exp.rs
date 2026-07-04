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

//! Kill-EXP formula. Port of pmeteor's experience helpers
//! (`Map Server/Actors/Chara/Ai/Utils/BattleUtils.cs:860-1032`,
//! "See 1.19 patch notes for exp info") with the low-level base band
//! recalibrated against retail man0l1 escort footage.

/// Base EXP per kill, indexed by `min(player_level, mob_level) - 1`.
///
/// Levels 19-50 are pmeteor's `BASEEXP` table verbatim
/// (`BattleUtils.cs:127`). pmeteor guesses a flat 150 for levels 1-18
/// ("might have been a change in base exp amounts after 1.19"), which
/// contradicts the two retail data points we hold: the man0l1 escort
/// screenshots show 625 EXP earned at player level 5 and 601 EXP x3 at
/// player level 6, all with no "(+%)" bonus suffix — i.e. base-table
/// rows at dlvl 0, not dlvl variants (pmeteor's ±20%/level steps cannot
/// connect 601 to 625). Calibration: rows 5/6 pinned at 625/601, rows
/// 1-4 flat at the level-5 value (mirroring pmeteor's flat low-band
/// shape), rows 7-18 linearly interpolated from 601 down to the
/// attested 160 at level 19. Refine from more correlated footage
/// (youtube-watcher OCR "experience points" samples) as it lands.
pub const BASE_EXP: [u16; 50] = [
    625, 625, 625, 625, 625, // 1-5 (5 pinned: retail 625 @ player lvl 5)
    601, // 6 (pinned: retail 601 x3 @ player lvl 6)
    567, 533, 499, 465, // 7-10 (interpolated 601 → 160)
    431, 397, 364, 330, 296, 262, 228, 194, // 11-18 (interpolated)
    160, 170, // 19-20 (pmeteor verbatim from here on)
    180, 190, 190, 200, 210, 220, 230, 240, 250, 260, // 21-30
    270, 280, 290, 300, 310, 320, 330, 340, 350, 360, // 31-40
    370, 380, 380, 390, 400, 410, 420, 430, 430, 440, // 41-50
];

/// EXP bonus (%) when 1/2/3/4+ enemies link — pmeteor
/// `BattleUtils.cs:GetLinkBonus`.
pub const LINK_BONUS: [u8; 5] = [0, 25, 50, 75, 100];

/// EXP-chain bonus (%) per chain tier 1-5+ — pmeteor
/// `BattleUtils.cs:GetChainBonus`.
pub const CHAIN_BONUS: [u8; 6] = [0, 20, 25, 30, 40, 50];

/// Seconds the next kill must land in to extend a chain at each tier —
/// pmeteor `BattleUtils.cs:GetChainTimeLimit`.
pub const CHAIN_TIME_LIMIT_S: [u8; 5] = [100, 80, 60, 20, 10];

/// Below this level difference a kill grants no EXP and no message —
/// pmeteor `BattleUtils.cs:GetBaseEXP` ("Less than -19 dlvl gives 0 exp
/// and no message is sent").
const MIN_DLVL: i32 = -20;

/// Base kill EXP before link/chain/food/rested bonuses — pmeteor
/// `BattleUtils.cs:GetBaseEXP`: base row for the lower of the two
/// levels, scaled by the dlvl curve (+20%/level above, quadratic
/// falloff below) and the flat 0.667 party modifier. Truncates like
/// the upstream `(ushort)` cast.
pub fn base_kill_exp(player_level: i16, mob_level: i16, party_size: u32) -> u16 {
    let dlvl = mob_level as i32 - player_level as i32;
    if dlvl <= MIN_DLVL {
        return 0;
    }
    let base_level = player_level.min(mob_level).clamp(1, BASE_EXP.len() as i16);
    let base = BASE_EXP[(base_level - 1) as usize] as f64;
    let dlvl_mod = if dlvl >= 0 {
        1.0 + 0.2 * dlvl as f64
    } else {
        // 0.1x + 0.0025x^2 — BattleUtils.cs:889.
        1.0 + 0.1 * dlvl as f64 + 0.0025 * (dlvl * dlvl) as f64
    };
    let party_mod = if party_size <= 1 { 1.0 } else { 0.667 };
    (base * dlvl_mod * party_mod) as u16
}

/// Link bonus (%) for `link_count` linked enemies (caps at 4+).
pub fn link_bonus(link_count: u16) -> u8 {
    LINK_BONUS[(link_count as usize).min(LINK_BONUS.len() - 1)]
}

/// Chain bonus (%) for a kill made at chain tier `tier` (caps at 5+).
pub fn chain_bonus(tier: u8) -> u8 {
    CHAIN_BONUS[(tier as usize).min(CHAIN_BONUS.len() - 1)]
}

/// Window (seconds) the next kill must land in to extend a chain that
/// is currently at `tier`.
pub fn chain_time_limit_s(tier: u8) -> u8 {
    CHAIN_TIME_LIMIT_S[(tier as usize).min(CHAIN_TIME_LIMIT_S.len() - 1)]
}

/// Fold a percentage bonus into a gain the way pmeteor's
/// `Player.AddExp` does (`exp += Math.Ceiling(exp * bonus / 100)`) —
/// byte-compatible with the Lua `AddExp` binding's fold
/// (`lua/userdata.rs`), i64 intermediate included.
pub fn with_bonus(exp: i32, bonus_percent: u8) -> i32 {
    if exp <= 0 || bonus_percent == 0 {
        return exp;
    }
    let bonused = exp as i64 + (exp as i64 * bonus_percent as i64 + 99) / 100;
    bonused.min(i32::MAX as i64) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_exp_and_no_message_at_or_below_minus_20_dlvl() {
        assert_eq!(base_kill_exp(50, 30, 1), 0);
        assert_eq!(base_kill_exp(50, 25, 1), 0);
        assert_eq!(base_kill_exp(21, 1, 1), 0);
    }

    #[test]
    fn dlvl_minus_19_still_pays() {
        // dlvl -19 modifier: 1 - 1.9 + 0.0025*361 = 0.0025; base row is
        // the mob's (lower) level 6 → 601 * 0.0025 = 1.50, trunc → 1.
        assert_eq!(base_kill_exp(25, 6, 1), 1);
    }

    #[test]
    fn retail_calibration_ankle_biters() {
        // "The Ankle Biters, which are Level 5, do NOT award experience
        // when they are killed" / "Screenshots indicate that Ankle
        // Biters may give a variable amount of experience." — retail
        // screenshots: 625 EXP at player level 5, 601 x3 at player
        // level 6, no (+%) suffix (dlvl 0 base rows).
        assert_eq!(base_kill_exp(5, 5, 1), 625);
        assert_eq!(base_kill_exp(6, 6, 1), 601);
    }

    #[test]
    fn dlvl_zero_is_the_bare_base_row() {
        assert_eq!(base_kill_exp(1, 1, 1), BASE_EXP[0]);
        assert_eq!(base_kill_exp(45, 45, 1), 400);
        assert_eq!(base_kill_exp(50, 50, 1), 440);
    }

    #[test]
    fn positive_dlvl_adds_20_percent_per_level() {
        // Level 5 vs level 6 mob: 625 * 1.2 = 750.
        assert_eq!(base_kill_exp(5, 6, 1), 750);
        // Level 6 vs level 7 mob: 601 * 1.2 = 721.2, trunc → 721.
        assert_eq!(base_kill_exp(6, 7, 1), 721);
        // Level 5 vs level 7 mob: 625 * 1.4 = 875.
        assert_eq!(base_kill_exp(5, 7, 1), 875);
    }

    #[test]
    fn negative_dlvl_quadratic_falloff() {
        // pmeteor's worked example (BattleUtils.cs:867-872): level 50
        // vs a level 45 enemy — base 400, dlvl -5 modifier 0.5625.
        assert_eq!(base_kill_exp(50, 45, 1), 225);
    }

    #[test]
    fn party_modifier_is_flat_0_667() {
        // The tail of pmeteor's worked example: 400 * 0.5625 * 0.667 = 150.
        assert_eq!(base_kill_exp(50, 45, 2), 150);
        // Same modifier regardless of party size.
        assert_eq!(base_kill_exp(50, 45, 8), 150);
        // 601 * 0.667 = 400.867, trunc → 400.
        assert_eq!(base_kill_exp(6, 6, 2), 400);
    }

    #[test]
    fn out_of_range_levels_clamp_into_the_table() {
        assert_eq!(base_kill_exp(0, 0, 1), BASE_EXP[0]);
        assert_eq!(base_kill_exp(60, 60, 1), BASE_EXP[49]);
    }

    #[test]
    fn link_bonus_tiers() {
        assert_eq!(link_bonus(0), 0);
        assert_eq!(link_bonus(1), 25);
        assert_eq!(link_bonus(2), 50);
        assert_eq!(link_bonus(3), 75);
        assert_eq!(link_bonus(4), 100);
        assert_eq!(link_bonus(9), 100, "caps at 4+ links");
    }

    #[test]
    fn chain_bonus_tiers() {
        assert_eq!(chain_bonus(0), 0);
        assert_eq!(chain_bonus(1), 20);
        assert_eq!(chain_bonus(2), 25);
        assert_eq!(chain_bonus(3), 30);
        assert_eq!(chain_bonus(4), 40);
        assert_eq!(chain_bonus(5), 50);
        assert_eq!(chain_bonus(200), 50, "caps at tier 5+");
    }

    #[test]
    fn chain_windows_shrink_per_tier() {
        assert_eq!(chain_time_limit_s(0), 100);
        assert_eq!(chain_time_limit_s(1), 80);
        assert_eq!(chain_time_limit_s(2), 60);
        assert_eq!(chain_time_limit_s(3), 20);
        assert_eq!(chain_time_limit_s(4), 10);
        assert_eq!(chain_time_limit_s(50), 10, "caps at tier 4+");
    }

    #[test]
    fn with_bonus_folds_like_player_add_exp() {
        assert_eq!(with_bonus(601, 0), 601);
        // ceil(601 * 20 / 100) = ceil(120.2) = 121.
        assert_eq!(with_bonus(601, 20), 722);
        // Exact division has nothing to round up.
        assert_eq!(with_bonus(100, 25), 125);
        assert_eq!(with_bonus(0, 50), 0);
        assert_eq!(with_bonus(-10, 50), -10);
        assert_eq!(with_bonus(i32::MAX, 100), i32::MAX, "saturates");
    }

    #[test]
    fn base_table_resumes_pmeteor_values_at_19() {
        // Spot-check the verbatim pmeteor band (BattleUtils.cs:127).
        assert_eq!(BASE_EXP[18], 160); // level 19
        assert_eq!(BASE_EXP[22], 190); // level 23 (repeated row upstream)
        assert_eq!(BASE_EXP[44], 400); // level 45 (worked example row)
        assert_eq!(BASE_EXP[47], 430); // level 48 (repeated row upstream)
    }
}
