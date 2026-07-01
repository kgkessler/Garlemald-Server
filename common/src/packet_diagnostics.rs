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

//! Unknown-packet diagnostics + a small classification registry.
//!
//! The per-server dispatchers ([`crate::subpacket`] fan-out in each
//! `processor.rs`) fall through to a catch-all arm whenever they meet a
//! subpacket `type` or game-message `opcode` they don't yet handle. Those
//! arms already emit a terse `tracing::debug!`, which is invisible in a
//! Release build and carries no payload bytes. During the ongoing
//! clean-room decomp of the FFXIV 1.23b client that opacity is expensive:
//! the fastest way to recover an opcode's meaning is to see its raw body
//! next to the context it arrived in.
//!
//! This module augments (never replaces) those catch-alls with a
//! Release-visible `tracing::info!` record carrying the header fields, a
//! bounded hex preview of the payload, and a best-effort `classification`
//! label drawn from a re-derived [`classify`] registry. To keep a normal
//! Release run quiet, the Info emission is gated behind the
//! [`ENV_VAR`] environment variable, mirroring
//! [`crate::packet_log`]'s `GARLEMALD_PACKET_LOG_DIR` pattern: unset means
//! a single `std::env::var_os` load and an early return, no allocation.
//!
//! The classification rules are heuristic hypotheses ("candidate"
//! suffixes) recovered by cross-referencing live garlemald captures with
//! the Project Meteor / AetherXIV reference dispatchers — they name a
//! *likely* meaning, not a confirmed one.

use std::fmt::Write as _;

use crate::packet::BasePacket;
use crate::subpacket::{SUBPACKET_TYPE_GAMEMESSAGE, SubPacket};

/// Environment variable that gates the Release-visible Info emission.
/// When unset, the `log_unknown_*` helpers do a single `var_os` load and
/// return without allocating — same degrade-to-noop shape as
/// [`crate::packet_log`]'s `GARLEMALD_PACKET_LOG_DIR`.
const ENV_VAR: &str = "GARLEMALD_UNKNOWN_PACKET_DIAG";

/// Upper bound on how many payload bytes a diagnostic record renders as a
/// single-line hex preview. Keeps the Info line readable and bounds the
/// per-record allocation regardless of subpacket size.
const HEX_PREVIEW_BYTES: usize = 128;

/// Subpacket `type` for the world-side zoning / heartbeat channel (the C#
/// `0x08` "zoning packet stub" arm).
const TYPE_WORLD_ZONING: u16 = 0x08;

/// Observed `subpacket_size` of the world zoning-heartbeat candidate.
const SIZE_WORLD_HEARTBEAT: u16 = 0x18;

/// Game-message opcode observed for the tutorial UI-state message.
const OPCODE_TUTORIAL_UI_STATE: u16 = 0x00CE;

/// Game-message opcode observed for the login zone-bootstrap message.
const OPCODE_LOGIN_ZONE_BOOTSTRAP: u16 = 0x0002;

/// Label used when [`classify`] has no hypothesis for a packet.
const UNCLASSIFIED: &str = "unclassified";

/// Render up to `max` bytes of `bytes` as a single-line, space-separated
/// lowercase hex string. If the slice is longer than `max`, a
/// `(+N more)` suffix records how many bytes were elided so a reader can
/// tell the preview is truncated rather than the whole payload.
fn hex_preview(bytes: &[u8], max: usize) -> String {
    let shown = bytes.len().min(max);
    let mut out = String::with_capacity(shown * 3 + 16);
    for (i, b) in bytes[..shown].iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        let _ = write!(out, "{b:02x}");
    }
    if bytes.len() > shown {
        let _ = write!(out, " (+{} more)", bytes.len() - shown);
    }
    out
}

/// Best-effort classification of an unhandled subpacket into a stable,
/// grep-able label. `context` is the dispatcher's server tag (`"map"`,
/// `"world"`, `"lobby"`) and is matched case-insensitively.
///
/// Returns `None` when no rule matches — callers fall back to
/// [`UNCLASSIFIED`]. Every label carries a `-candidate` suffix because the
/// mapping is a re-derived hypothesis, not a confirmed opcode name.
pub fn classify(context: &str, sub: &SubPacket) -> Option<&'static str> {
    let ctx = context.to_lowercase();
    let ty = sub.header.r#type;
    let size = sub.header.subpacket_size;
    let opcode = sub.game_message.opcode;

    if ctx.contains("world") && ty == TYPE_WORLD_ZONING && size == SIZE_WORLD_HEARTBEAT {
        return Some("world.session-heartbeat-candidate");
    }
    if ctx.contains("map") && ty == SUBPACKET_TYPE_GAMEMESSAGE && opcode == OPCODE_TUTORIAL_UI_STATE
    {
        return Some("map.event-tutorial-ui-state-candidate");
    }
    if ctx.contains("map")
        && ty == SUBPACKET_TYPE_GAMEMESSAGE
        && opcode == OPCODE_LOGIN_ZONE_BOOTSTRAP
    {
        return Some("map.login-zone-bootstrap-candidate");
    }
    None
}

/// Whether the Release-visible Info emission is enabled for this process.
/// The `var_os` probe runs exactly once and is cached in a `OnceLock`
/// (mirrors [`crate::packet_log`]'s one-time init), so the disabled
/// fast-path collapses to a single atomic load with no per-call syscall.
fn diag_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os(ENV_VAR).is_some())
}

/// Emit an Info-level diagnostic for a subpacket that fell through a
/// dispatcher's type catch-all. No-op unless [`ENV_VAR`] is set.
pub fn log_unknown_subpacket(server: &str, context: &str, sub: &SubPacket) {
    if !diag_enabled() {
        return;
    }
    let classification = classify(context, sub).unwrap_or(UNCLASSIFIED);
    tracing::info!(
        server = server,
        context = context,
        r#type = format!("0x{:04X}", sub.header.r#type),
        opcode = format!("0x{:04X}", sub.game_message.opcode),
        source = format!("0x{:08X}", sub.header.source_id),
        target = format!("0x{:08X}", sub.header.target_id),
        size = sub.header.subpacket_size,
        hex = hex_preview(&sub.data, HEX_PREVIEW_BYTES),
        classification,
        "unknown subpacket (diag)",
    );
}

/// Emit an Info-level diagnostic for a game message that fell through a
/// dispatcher's opcode catch-all. No-op unless [`ENV_VAR`] is set.
pub fn log_unknown_game_message(server: &str, context: &str, sub: &SubPacket) {
    if !diag_enabled() {
        return;
    }
    let classification = classify(context, sub).unwrap_or(UNCLASSIFIED);
    tracing::info!(
        server = server,
        context = context,
        r#type = format!("0x{:04X}", sub.header.r#type),
        opcode = format!("0x{:04X}", sub.game_message.opcode),
        source = format!("0x{:08X}", sub.header.source_id),
        target = format!("0x{:08X}", sub.header.target_id),
        size = sub.header.subpacket_size,
        hex = hex_preview(&sub.data, HEX_PREVIEW_BYTES),
        classification,
        "unknown game message (diag)",
    );
}

/// Emit an Info-level diagnostic for a base packet whose framing could not
/// be interpreted (e.g. an unexpected `connection_type`). Base packets have
/// no subpacket header to classify, so no `classification` label applies.
/// No-op unless [`ENV_VAR`] is set.
pub fn log_unknown_base_packet(server: &str, context: &str, pkt: &BasePacket) {
    if !diag_enabled() {
        return;
    }
    tracing::info!(
        server = server,
        context = context,
        connection_type = format!("0x{:04X}", pkt.header.connection_type),
        num_subpackets = pkt.header.num_subpackets,
        size = pkt.header.packet_size,
        hex = hex_preview(&pkt.data, HEX_PREVIEW_BYTES),
        "unknown base packet (diag)",
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subpacket::{GameMessageHeader, SubPacketHeader};

    /// Build a bare subpacket carrying only the header fields the
    /// classifier reads, so each case isolates a single rule.
    fn make_sub(ty: u16, size: u16, opcode: u16) -> SubPacket {
        SubPacket {
            header: SubPacketHeader {
                r#type: ty,
                subpacket_size: size,
                ..Default::default()
            },
            game_message: GameMessageHeader {
                opcode,
                ..Default::default()
            },
            data: Vec::new(),
        }
    }

    #[test]
    fn classify_world_session_heartbeat_candidate() {
        let sub = make_sub(TYPE_WORLD_ZONING, SIZE_WORLD_HEARTBEAT, 0x0000);
        assert_eq!(
            classify("world", &sub),
            Some("world.session-heartbeat-candidate")
        );
    }

    #[test]
    fn classify_map_event_tutorial_ui_state_candidate() {
        let sub = make_sub(SUBPACKET_TYPE_GAMEMESSAGE, 0x0020, OPCODE_TUTORIAL_UI_STATE);
        assert_eq!(
            classify("map", &sub),
            Some("map.event-tutorial-ui-state-candidate")
        );
    }

    #[test]
    fn classify_map_login_zone_bootstrap_candidate() {
        let sub = make_sub(
            SUBPACKET_TYPE_GAMEMESSAGE,
            0x0020,
            OPCODE_LOGIN_ZONE_BOOTSTRAP,
        );
        assert_eq!(
            classify("map", &sub),
            Some("map.login-zone-bootstrap-candidate")
        );
    }

    #[test]
    fn classify_context_is_case_insensitive() {
        let sub = make_sub(TYPE_WORLD_ZONING, SIZE_WORLD_HEARTBEAT, 0x0000);
        assert_eq!(
            classify("WORLD-Zone", &sub),
            Some("world.session-heartbeat-candidate")
        );
    }

    #[test]
    fn classify_heartbeat_requires_world_context() {
        // Right shape, wrong server context → no world rule fires.
        let sub = make_sub(TYPE_WORLD_ZONING, SIZE_WORLD_HEARTBEAT, 0x0000);
        assert_eq!(classify("map", &sub), None);
    }

    #[test]
    fn classify_tutorial_requires_map_context() {
        // Right opcode/type, wrong server context → no map rule fires.
        let sub = make_sub(SUBPACKET_TYPE_GAMEMESSAGE, 0x0020, OPCODE_TUTORIAL_UI_STATE);
        assert_eq!(classify("world", &sub), None);
    }

    #[test]
    fn classify_heartbeat_requires_exact_size() {
        // Correct context + type but a different size is not the candidate.
        let sub = make_sub(TYPE_WORLD_ZONING, 0x0020, 0x0000);
        assert_eq!(classify("world", &sub), None);
    }

    #[test]
    fn hex_preview_renders_full_short_slice() {
        assert_eq!(hex_preview(&[0x00, 0xAB, 0xff], 128), "00 ab ff");
    }

    #[test]
    fn hex_preview_truncates_and_reports_remainder() {
        let bytes = [0x11u8; 4];
        let preview = hex_preview(&bytes, 2);
        assert_eq!(preview, "11 11 (+2 more)");
    }

    #[test]
    fn hex_preview_empty_slice_is_empty() {
        assert_eq!(hex_preview(&[], 128), "");
    }
}
