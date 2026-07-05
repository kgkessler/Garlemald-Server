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

//! Script path resolver. Ported from the `FILEPATH_*` constants in
//! `LuaEngine.cs`. Every function takes a script root and returns an
//! absolute path; callers decide whether the file exists.
//!
//! Lookups are CASE-INSENSITIVE on case-sensitive filesystems: the C#
//! original only ever ran on Windows/NTFS (and this port's macOS dev
//! box is case-insensitive APFS), so both the gamedata class paths and
//! the code's constructed paths freely disagree with the on-disk casing
//! (`/chara/npc/monster/fighter/…` vs `monster/Fighter/…`,
//! `privatearea/` vs `PrivateArea/`, `quests/dft/dftsea.lua` vs
//! `DftSea.lua`). On ext4 those lookups silently miss — live-proven by
//! the Ubuntu opening-quest kick: the zone-193 BattleNpc init scripts
//! (`base/chara/npc/monster/Fighter|Jellyfish/…`) failed `exists()`, the
//! ActorInstantiate tail fell back to the populace shape, and the retail
//! client's `DepictionJudge:judgeNameplate` crashed on the resulting nil
//! `charaWork.parameterTemp` → kick to character select. Every builder
//! therefore falls back to a per-component case-insensitive scan when
//! the literal path does not exist, restoring APFS/NTFS semantics.

use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone)]
pub struct PathResolver {
    pub root: PathBuf,
}

impl PathResolver {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Join `rel` onto the script root, falling back to a
    /// case-insensitive component walk when the literal path is absent.
    /// Returns the literal join when no case-insensitive match exists
    /// either, so callers' `exists()` gates fail exactly as before.
    fn resolve(&self, rel: impl AsRef<Path>) -> PathBuf {
        let exact = self.root.join(rel.as_ref());
        if exact.exists() {
            return exact;
        }
        match case_insensitive_lookup(&self.root, rel.as_ref()) {
            Some(found) => {
                tracing::debug!(
                    requested = %exact.display(),
                    resolved = %found.display(),
                    "script path resolved case-insensitively — fix the source casing",
                );
                found
            }
            None => exact,
        }
    }

    pub fn player(&self) -> PathBuf {
        self.resolve("player.lua")
    }

    pub fn zone(&self, zone_name: &str) -> PathBuf {
        // C# `LuaEngine.GetLuaScriptPath` for `Area` targets returns
        // `./scripts/unique/{zoneName}/zone.lua`. In both Project Meteor's
        // `Data/scripts/` snapshot and our own `scripts/lua/`, the actual
        // on-disk `zone.lua` lives one level deeper — under
        // `unique/{zoneName}/PopulaceStandard/`. Prefer the flat path when
        // it exists (in case a zone has been promoted to the canonical
        // location) and fall back to the PopulaceStandard subdir so
        // `ocn0Battle02` (and every other tutorial/town/field zone) resolves.
        let flat = self.resolve(format!("unique/{zone_name}/zone.lua"));
        if flat.exists() {
            return flat;
        }
        self.resolve(format!("unique/{zone_name}/PopulaceStandard/zone.lua"))
    }

    pub fn npc(&self, zone_name: &str, class_name: &str, unique_id: &str) -> PathBuf {
        self.resolve(format!("unique/{zone_name}/{class_name}/{unique_id}.lua"))
    }

    pub fn npc_in_private_area(
        &self,
        zone_name: &str,
        area_name: &str,
        area_type: u32,
        class_name: &str,
        unique_id: &str,
    ) -> PathBuf {
        self.resolve(format!(
            "unique/{zone_name}/privatearea/{area_name}_{area_type}/{class_name}/{unique_id}.lua"
        ))
    }

    pub fn base_class(&self, class_path: &str) -> PathBuf {
        self.resolve(format!("base/{class_path}.lua"))
    }

    pub fn content(&self, content_name: &str) -> PathBuf {
        self.resolve(format!("content/{content_name}.lua"))
    }

    pub fn gm_command(&self, cmd: &str) -> PathBuf {
        self.resolve(format!("commands/gm/{}.lua", cmd.to_lowercase()))
    }

    /// A client command static actor's script: `commands/<Name>.lua`
    /// (case-preserving, matching pmeteor `LuaEngine.FILEPATH_COMMANDS`).
    /// Used to dispatch e.g. `ActivateCommand`. (Garlemald-Server #28.)
    pub fn command(&self, name: &str) -> PathBuf {
        self.resolve(format!("commands/{name}.lua"))
    }

    pub fn battle_command(&self, folder: &str, command: &str) -> PathBuf {
        self.resolve(format!("commands/{folder}/{command}.lua"))
    }

    pub fn battle_command_default(&self, folder: &str) -> PathBuf {
        self.resolve(format!("commands/{folder}/default.lua"))
    }

    pub fn status_effect(&self, name: &str) -> PathBuf {
        self.resolve(format!("effects/{name}.lua"))
    }

    pub fn status_effect_default(&self) -> PathBuf {
        self.resolve("effects/default.lua")
    }

    pub fn director(&self, name: &str) -> PathBuf {
        self.resolve(format!("directors/{name}.lua"))
    }

    /// Quest scripts live under `quests/<first-3-chars-of-name>/<name>.lua`
    /// in the C# original; reproducing that prefix lookup here.
    pub fn quest(&self, quest_name: &str) -> PathBuf {
        let initial: String = quest_name.chars().take(3).collect();
        self.resolve(format!("quests/{initial}/{quest_name}.lua"))
    }

    pub fn exists(path: &Path) -> bool {
        path.exists()
    }
}

/// Walk `rel` below `root` one component at a time; whenever a
/// component is missing verbatim, scan its parent directory for an
/// ASCII-case-insensitive match (script names are all ASCII). Returns
/// `None` when any component matches nothing. Repeated separators and
/// `.` components (e.g. the `base//Chara/…` shape produced by
/// `base_class` on gamedata class paths with a leading slash) are
/// normalized away by `Path::components`. Should the tree ever hold
/// case-twin entries, the lexicographically first match wins so the
/// choice is deterministic across runs and platforms.
fn case_insensitive_lookup(root: &Path, rel: &Path) -> Option<PathBuf> {
    let mut cur = root.to_path_buf();
    for comp in rel.components() {
        match comp {
            Component::Normal(name) => {
                let direct = cur.join(name);
                if direct.exists() {
                    cur = direct;
                    continue;
                }
                let want = name.to_str()?;
                let mut matches: Vec<_> = std::fs::read_dir(&cur)
                    .ok()?
                    .filter_map(|e| e.ok())
                    .map(|e| e.file_name())
                    .filter(|n| n.to_str().is_some_and(|n| n.eq_ignore_ascii_case(want)))
                    .collect();
                matches.sort();
                cur.push(matches.into_iter().next()?);
            }
            Component::CurDir => {}
            _ => return None,
        }
    }
    Some(cur)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quest_path_extracts_prefix() {
        let r = PathResolver::new("/srv");
        assert_eq!(
            r.quest("man0l0"),
            PathBuf::from("/srv/quests/man/man0l0.lua")
        );
    }

    #[test]
    fn gm_command_lowercases() {
        let r = PathResolver::new("/srv");
        assert_eq!(
            r.gm_command("WARP"),
            PathBuf::from("/srv/commands/gm/warp.lua")
        );
    }

    /// Resolver over the real repo scripts tree, exercising the
    /// case-insensitive fallback on case-sensitive filesystems (on
    /// APFS/NTFS the literal join already hits, which is the behavior
    /// the fallback reproduces).
    fn repo_resolver() -> PathResolver {
        PathResolver::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts/lua"))
    }

    /// The Ubuntu opening-quest kick regression: `lowercase_class_path`
    /// output (`…/monster/fighter/…`) must find the on-disk
    /// `monster/Fighter/…` init script, or the zone-193 BattleNpcs ship
    /// a populace-shaped ActorInstantiate tail and the retail client
    /// crashes on nil `charaWork.parameterTemp` in judgeNameplate.
    #[test]
    fn monster_genus_dir_resolves_despite_lowercased_class_path() {
        let r = repo_resolver();
        for class_path in [
            "/chara/npc/monster/fighter/FighterAllyOpeningHealer",
            "/chara/npc/monster/fighter/FighterAllyOpeningAttacker",
            "/chara/npc/monster/jellyfish/JellyfishScenarioLimsaLv00",
        ] {
            let p = r.base_class(class_path);
            assert!(
                p.exists(),
                "base class script not found for {class_path}: {p:?}"
            );
        }
    }

    /// The NPC event dispatcher passes the RAW gamedata class path
    /// (leading slash, capitalized segments — `base//Chara/…` after the
    /// format). Both the `//` and the casing must resolve.
    #[test]
    fn raw_gamedata_class_path_resolves() {
        let r = repo_resolver();
        let p = r.base_class("/Chara/Npc/Populace/PopulaceStandard");
        assert!(p.exists(), "raw class path did not resolve: {p:?}");
    }

    /// `npc_in_private_area` builds a lowercase `privatearea/` segment;
    /// every on-disk tree spells it `PrivateArea/`.
    #[test]
    fn private_area_segment_resolves_case_insensitively() {
        let r = repo_resolver();
        let p = r.npc_in_private_area(
            "sea0Town01",
            "PrivateAreaMasterPast",
            2,
            "PopulaceStandard",
            "man0l1_baderon",
        );
        assert!(p.exists(), "private-area override did not resolve: {p:?}");
    }

    /// Quest script names are lowercased by the catalog before path
    /// construction, but six scripts are stored capitalized on disk
    /// (DftSea et al.).
    #[test]
    fn capitalized_quest_script_resolves_from_lowercased_name() {
        let r = repo_resolver();
        let p = r.quest("dftsea");
        assert!(p.exists(), "quests/dft/DftSea.lua did not resolve: {p:?}");
    }

    /// A genuinely missing path must keep returning the literal join
    /// (and not exist) so callers' `exists()` gates fail as before.
    #[test]
    fn missing_path_falls_through_to_literal_join() {
        let r = repo_resolver();
        let p = r.base_class("/chara/npc/monster/fighter/NoSuchClass");
        assert!(!p.exists());
        assert!(p.ends_with("base/chara/npc/monster/fighter/NoSuchClass.lua"));
    }
}
