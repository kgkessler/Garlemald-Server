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

//! Leftover root-level packets: SetMap/Music/Weather/Dalamud + GameMessage +
//! SendMessage.

use std::io::Cursor;

use byteorder::{LittleEndian, WriteBytesExt};
use common::luaparam::{self, LuaParam};
use common::subpacket::SubPacket;

use super::super::opcodes::*;
use super::{body, write_padded_ascii};

/// Synthetic actor id of the global "WorldMaster" actor — Project Meteor's
/// `WorldManager.GetActor()`. Used as the source actor for every
/// receiver-targeted system message that doesn't have a real
/// in-world sender (quest-accept ding, quest-complete banner, NpcLs
/// linkpearl-obtained toast, …). Per Map Server/Actors/WorldMaster.cs.
pub const WORLD_MASTER_ACTOR_ID: u32 = 0x5FF8_0001;

/// Synthetic actor id of the "Debug" actor — `/System/Debug.prog`. Used
/// as the sender for the `/_init` debug-channel events. Sibling
/// constant to `WORLD_MASTER_ACTOR_ID` — both are hardcoded in
/// Meteor's `WorldManager.LoadServerActors`.
pub const DEBUG_ACTOR_ID: u32 = 0x5FF8_0002;

/// 0x0005 SetMap — loads a zone/region map on the client side. Wire layout
/// mirrors `Map Server/Packets/Send/SetMapPacket.cs`: `region_id` first,
/// `zone_actor_id` second, then the magic 0x28 at offset 0x08. The C# param
/// names are misleading — its `mapID` parameter actually receives `zone.regionId`
/// and its `regionID` receives `zone.actorId`. Built as a game-message subpacket
/// (the C# `new SubPacket(OPCODE, ...)` overload defaults to `isGameMessage=true`).
pub fn build_set_map(actor_id: u32, region_id: u32, zone_actor_id: u32) -> SubPacket {
    let mut data = body(0x30);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u32::<LittleEndian>(region_id).unwrap();
    c.write_u32::<LittleEndian>(zone_actor_id).unwrap();
    c.write_u32::<LittleEndian>(0x28).unwrap();
    SubPacket::new(OP_SET_MAP, actor_id, data)
}

/// 0x000C SetMusic. Built as a game-message subpacket — the C# `new SubPacket(
/// OPCODE, ...)` overload defaults to `isGameMessage=true`, so the client
/// expects a type=0x03 frame with the opcode in the game-message header.
pub fn build_set_music(actor_id: u32, music_id: u16, music_track_mode: u16) -> SubPacket {
    let mut data = body(0x28);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u16::<LittleEndian>(music_id).unwrap();
    c.write_u16::<LittleEndian>(music_track_mode).unwrap();
    SubPacket::new(OP_SET_MUSIC, actor_id, data)
}

/// 0x000D SetWeather. Game-message subpacket (same reasoning as SetMusic).
pub fn build_set_weather(actor_id: u32, weather_id: u16, transition_time: u16) -> SubPacket {
    let mut data = body(0x28);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u16::<LittleEndian>(weather_id).unwrap();
    c.write_u16::<LittleEndian>(transition_time).unwrap();
    SubPacket::new(OP_SET_WEATHER, actor_id, data)
}

/// 0x0010 SetDalamud — gating for Dalamud features, one signed byte.
/// Game-message subpacket (same reasoning as SetMusic).
pub fn build_set_dalamud(actor_id: u32, dalamud_level: i8) -> SubPacket {
    let mut data = body(0x28);
    data[0] = dalamud_level as u8;
    SubPacket::new(OP_SET_DALAMUD, actor_id, data)
}

// --- Game messages / chat ---------------------------------------------------

/// Game-message options shared with the older `build_game_message`.
pub struct GameMessageOptions {
    pub sender_actor_id: u32,
    pub receiver_actor_id: u32,
    pub text_id: u16,
    pub log: u8,
    pub display_id: Option<u32>,
    pub custom_sender: Option<String>,
    pub lua_params: Vec<LuaParam>,
}

/// 0x01FD GameMessagePacket (default).
pub fn build_game_message(source_actor_id: u32, opts: GameMessageOptions) -> SubPacket {
    let mut body = Vec::<u8>::with_capacity(0x40 + opts.lua_params.len() * 8);
    body.write_u32::<LittleEndian>(opts.receiver_actor_id)
        .unwrap();
    body.write_u32::<LittleEndian>(opts.sender_actor_id)
        .unwrap();
    body.write_u16::<LittleEndian>(opts.text_id).unwrap();
    body.write_u8(opts.log).unwrap();
    body.write_u8(0).unwrap();
    if let Some(id) = opts.display_id {
        body.write_u32::<LittleEndian>(id).unwrap();
    } else if let Some(ref name) = opts.custom_sender {
        write_padded_ascii(&mut body, name, 0x20);
    }
    luaparam::write_lua_params(&mut body, &opts.lua_params).unwrap();
    SubPacket::new(OP_GAME_MESSAGE, source_actor_id, body)
}

/// 0x0157..0x015B GameMessageWithActor1..5 — actor-scoped variants.
#[allow(clippy::too_many_arguments)]
pub fn build_game_message_with_actors(
    source_actor_id: u32,
    actor_count: u8,
    actors: &[u32; 5],
    text_id: u16,
    log: u8,
    params: &[LuaParam],
) -> SubPacket {
    let opcode = match actor_count {
        1 => OP_GAME_MESSAGE_ACTOR1,
        2 => OP_GAME_MESSAGE_ACTOR2,
        3 => OP_GAME_MESSAGE_ACTOR3,
        4 => OP_GAME_MESSAGE_ACTOR4,
        _ => OP_GAME_MESSAGE_ACTOR5,
    };
    let mut body = Vec::<u8>::with_capacity(0x40 + params.len() * 8);
    for i in 0..actor_count.min(5) {
        body.write_u32::<LittleEndian>(actors[i as usize]).unwrap();
    }
    body.write_u16::<LittleEndian>(text_id).unwrap();
    body.write_u8(log).unwrap();
    body.write_u8(0).unwrap();
    luaparam::write_lua_params(&mut body, params).unwrap();
    SubPacket::new(opcode, source_actor_id, body)
}

/// 0x0157 GameMessage "with actor ×1", param-less tier — the exact
/// Meteor `GameMessagePacket.BuildPacket(sourceActorId, actorId,
/// textOwnerActorId, textId, log)` shape (PACKET_SIZE 0x30 → 0x10
/// body): `u32 actorId` + `u32 textOwnerActorId` + `u16 textId` +
/// `u16 log`. Used by `player:SendGameMessage(...)` — e.g. the opening
/// stoppers' 34109 "off limits" caution line. The client resolves
/// `textId` against `textOwnerActorId`'s text sheet (WorldMaster for
/// system lines).
pub fn build_game_message_actor1(
    source_actor_id: u32,
    actor_id: u32,
    text_owner_actor_id: u32,
    text_id: u16,
    log: u8,
) -> SubPacket {
    let mut data = body(0x30);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u32::<LittleEndian>(actor_id).unwrap();
    c.write_u32::<LittleEndian>(text_owner_actor_id).unwrap();
    c.write_u16::<LittleEndian>(text_id).unwrap();
    c.write_u16::<LittleEndian>(log as u16).unwrap();
    SubPacket::new(OP_GAME_MESSAGE_ACTOR1, source_actor_id, data)
}

/// pmeteor `GameMessagePacket.BuildPacket(sourceId, actorId,
/// textOwnerActorId, textId, log, lParams)` — the WITH-LuaParams overload of
/// `build_game_message_actor1` (GameMessagePacket.cs:100-145). The opcode is
/// chosen by the serialized LuaParam byte size: `<= 0x8` → GameMessageWithActor2
/// (0x0158, 0x38), `<= 0x10` → Actor3 (0x0159, 0x40), `<= 0x20` → Actor4
/// (0x015A, 0x50), else Actor5 (0x015B, 0x70). Body layout is
/// `[actorId u32][textOwnerActorId u32][textId u16][log u16][LuaParams]`, with
/// a MANDATORY `u32 = 8` params-region marker at body offset 0x14 when the
/// params are small (`<= 0x8` bytes) — the 1.23b client validates that marker
/// (the same `_invalid_parameter` crash path the 0x0167 no-source tier hits if
/// it's omitted). Used for quest "You obtain <item>" toasts (text 25117 + the
/// item-id param) so the item name resolves instead of rendering blank.
/// (Garlemald-Server #46.)
pub fn build_game_message_actor1_with_params(
    source_actor_id: u32,
    actor_id: u32,
    text_owner_actor_id: u32,
    text_id: u16,
    log: u8,
    params: &[LuaParam],
) -> SubPacket {
    let mut pbytes = Vec::<u8>::new();
    luaparam::write_lua_params(&mut pbytes, params).unwrap();
    let (opcode, packet_size) = if pbytes.len() <= 0x8 {
        (OP_GAME_MESSAGE_ACTOR2, 0x38usize)
    } else if pbytes.len() <= 0x10 {
        (OP_GAME_MESSAGE_ACTOR3, 0x40)
    } else if pbytes.len() <= 0x20 {
        (OP_GAME_MESSAGE_ACTOR4, 0x50)
    } else {
        (OP_GAME_MESSAGE_ACTOR5, 0x70)
    };
    let mut body_buf = Vec::<u8>::with_capacity(packet_size - 0x20);
    body_buf.write_u32::<LittleEndian>(actor_id).unwrap();
    body_buf
        .write_u32::<LittleEndian>(text_owner_actor_id)
        .unwrap();
    body_buf.write_u16::<LittleEndian>(text_id).unwrap();
    body_buf.write_u16::<LittleEndian>(log as u16).unwrap();
    body_buf.extend_from_slice(&pbytes);
    let mut data = body(packet_size);
    data[..body_buf.len()].copy_from_slice(&body_buf);
    // Small-params tier (Actor2): the client requires the `u32 = 8` marker at
    // body offset 0x14 — pmeteor seeks to 0x14 and writes it after the params.
    if pbytes.len() <= 0x8 {
        data[0x14..0x18].copy_from_slice(&8u32.to_le_bytes());
    }
    SubPacket::new(opcode, source_actor_id, data)
}

// ---------------------------------------------------------------------------
// 0x0166-0x016A "Text Sheet Message (No Source Actor)" family — system
// messages routed through a static sender (WorldMaster, gamedata id, etc.)
// rather than a runtime actor in the world.
//
// Wire format (per retail bytes from `ffxiv_traces/gather_wood.pcapng`,
// `ffxiv_traces/accept_quest.pcapng`, etc., decoded via
// `packet-diff/cargo run --bin pcap-survey -- … --dump-opcode 0x016X`):
//
//   u32 sender_actor_id   (4 bytes — typically 0x5FF80001 WorldMaster
//                          or a 0xA0F-prefixed static gamedata id)
//   u16 text_id           (2 bytes — index into the client's text-sheet
//                          table)
//   u8  log_flag          (1 byte — captured 0x20, matches the existing
//                          MESSAGE_TYPE_SYSTEM constant for system log)
//   u8  pad               (1 byte, zero)
//   LuaParams             (variable — 0..N tiers per opcode, see table)
//
// Tier table (size figures are SubPacket total = 0x10 header + 0x10 GMHeader
// + body):
//   0x0166 (28b) — body  8, params capacity  0  — header-only message
//   0x0167 (38b) — body 24, params capacity  8  — + u32 `8` marker at +0x10
//   0x0168 (38b) — body 24, params capacity 16  — no marker
//   0x0169 (48b) — body 40, params capacity 32  — ~4 params
//   0x016A (68b) — body 72, params capacity 64  — ~8 params
//
// pmeteor DOES implement this family: `GameMessagePacket.BuildPacket`'s
// WITHOUT_ACTOR overload (GameMessagePacket.cs:296-341) — opcode by params
// size (<=0x8 -> 0x0167 + a mandatory `u32 = 8` marker at body+0x10,
// <=0x10 -> 0x0168, <=0x20 -> 0x0169, else 0x016A), header source = the
// message sender (WorldMaster 0x5FF80001 for system messages — the 1.x
// client dispatches by header source, so it MUST be an actor that always
// exists client-side, never the player). The No-Source variants are what
// retail uses for system feedback like "You harvest a Maple Log",
// "Quest accepted", etc.

/// Common 8-byte header for the Text Sheet (No Source Actor) family.
fn write_text_sheet_no_source_header(
    out: &mut Vec<u8>,
    sender_actor_id: u32,
    text_id: u16,
    log_flag: u8,
) {
    out.write_u32::<LittleEndian>(sender_actor_id).unwrap();
    out.write_u16::<LittleEndian>(text_id).unwrap();
    out.write_u8(log_flag).unwrap();
    out.write_u8(0).unwrap();
}

/// 0x0166 Text Sheet Message (No Source Actor) (28b) — header only;
/// no LuaParams. Smallest tier; the simplest "fire a system text id"
/// emission.
pub fn build_text_sheet_no_source_x28(
    source_actor_id: u32,
    text_owner_actor_id: u32,
    text_id: u16,
    log_flag: u8,
) -> SubPacket {
    let mut body_buf = Vec::<u8>::with_capacity(8);
    write_text_sheet_no_source_header(&mut body_buf, text_owner_actor_id, text_id, log_flag);
    let mut data = body(0x28);
    data[..body_buf.len()].copy_from_slice(&body_buf);
    SubPacket::new(OP_TEXT_SHEET_NO_ACTOR_X28, source_actor_id, data)
}

/// 0x0167 Text Sheet Message (No Source Actor) (38b). Up to 8 bytes of
/// LuaParams (one typical int param), with a MANDATORY `u32 = 8`
/// params-region marker at body offset 0x10.
///
/// pmeteor `GameMessagePacket.BuildPacket` (WITHOUT_ACTOR family,
/// GameMessagePacket.cs:296-341): the `lParamsSize <= 0x8` tier picks
/// opcode 0x0167 and then `binWriter.Seek(0x10); binWriter.Write((UInt32)8)`.
/// 0x0167 and 0x0168 share the same 0x38 packet size — the marker is
/// what distinguishes the 8-byte-params layout from 0x0168's 16-byte
/// one, and the 1.23b client VALIDATES it: omitting the marker (as this
/// builder originally did) makes the client's 0x0167 handler abort the
/// process via the CRT `_invalid_parameter` path (exception 0xC000000D
/// at the login zone-in — the post-PR-#35 character-creation crash).
/// Byte-validated against pmeteor's login toast in
/// `captures/pmeteor-quest/20260426-160210-gridania-manual3/map-packets.log:829`.
pub fn build_text_sheet_no_source_x38(
    source_actor_id: u32,
    text_owner_actor_id: u32,
    text_id: u16,
    log_flag: u8,
    lua_params: &[LuaParam],
) -> SubPacket {
    let mut sub = build_text_sheet_no_source_n(
        source_actor_id,
        text_owner_actor_id,
        text_id,
        log_flag,
        lua_params,
        OP_TEXT_SHEET_NO_ACTOR_X38,
        0x38,
    );
    // Params region is body bytes 0x8..0x10; the marker lives at 0x10.
    sub.data[0x10..0x14].copy_from_slice(&8u32.to_le_bytes());
    sub
}

/// 0x0168 Text Sheet Message (No Source Actor) (38b alt). Same body
/// size as 0x0167; the captures don't reveal an unambiguous semantic
/// distinction. Captured in different feature areas than 0x0167
/// (`gather_wood`, `harvest`, `local_leve_complete` for 0x0168 vs.
/// `accept_leve`, `accept_quest`, `sell_item` for 0x0167). Caller
/// picks based on the message's intended display / log routing.
pub fn build_text_sheet_no_source_x38_alt(
    source_actor_id: u32,
    text_owner_actor_id: u32,
    text_id: u16,
    log_flag: u8,
    lua_params: &[LuaParam],
) -> SubPacket {
    build_text_sheet_no_source_n(
        source_actor_id,
        text_owner_actor_id,
        text_id,
        log_flag,
        lua_params,
        OP_TEXT_SHEET_NO_ACTOR_X38_ALT,
        0x38,
    )
}

/// 0x0169 Text Sheet Message (No Source Actor) (48b). Up to 32 bytes
/// of LuaParams.
pub fn build_text_sheet_no_source_x48(
    source_actor_id: u32,
    text_owner_actor_id: u32,
    text_id: u16,
    log_flag: u8,
    lua_params: &[LuaParam],
) -> SubPacket {
    build_text_sheet_no_source_n(
        source_actor_id,
        text_owner_actor_id,
        text_id,
        log_flag,
        lua_params,
        OP_TEXT_SHEET_NO_ACTOR_X48,
        0x48,
    )
}

/// 0x016A Text Sheet Message (No Source Actor) (68b). Up to 64 bytes
/// of LuaParams. Not observed in the survey but defined for symmetry.
pub fn build_text_sheet_no_source_x68(
    source_actor_id: u32,
    text_owner_actor_id: u32,
    text_id: u16,
    log_flag: u8,
    lua_params: &[LuaParam],
) -> SubPacket {
    build_text_sheet_no_source_n(
        source_actor_id,
        text_owner_actor_id,
        text_id,
        log_flag,
        lua_params,
        OP_TEXT_SHEET_NO_ACTOR_X68,
        0x68,
    )
}

fn build_text_sheet_no_source_n(
    source_actor_id: u32,
    text_owner_actor_id: u32,
    text_id: u16,
    log_flag: u8,
    lua_params: &[LuaParam],
    opcode: u16,
    packet_size: usize,
) -> SubPacket {
    let mut body_buf = Vec::<u8>::with_capacity(packet_size.saturating_sub(0x20));
    write_text_sheet_no_source_header(&mut body_buf, text_owner_actor_id, text_id, log_flag);
    luaparam::write_lua_params(&mut body_buf, lua_params).unwrap();
    let mut data = body(packet_size);
    let n = body_buf.len().min(data.len());
    data[..n].copy_from_slice(&body_buf[..n]);
    SubPacket::new(opcode, source_actor_id, data)
}

/// Convenience: pick the smallest tier that fits the LuaParam payload.
/// Captures show retail uses 0x0167 vs. 0x0168 with the same body size
/// for routing reasons — the auto-tier picker defaults to the
/// "primary" 0x0167 / 0x0168 style based on `prefer_alt`.
pub fn build_text_sheet_no_source_auto(
    source_actor_id: u32,
    text_owner_actor_id: u32,
    text_id: u16,
    log_flag: u8,
    lua_params: &[LuaParam],
    prefer_alt: bool,
) -> SubPacket {
    if lua_params.is_empty() {
        return build_text_sheet_no_source_x28(
            source_actor_id,
            text_owner_actor_id,
            text_id,
            log_flag,
        );
    }
    // Probe param byte length by serializing into a temp buffer. The
    // thresholds mirror pmeteor's `findSizeOfParams` tiers
    // (GameMessagePacket.cs:302-321): <=0x8 -> 0x0167 (with marker),
    // <=0x10 -> 0x0168, <=0x20 -> 0x0169, else 0x016A. `prefer_alt`
    // forces 0x0168 for small payloads where a capture shows retail
    // using the 16-byte layout.
    let mut probe = Vec::<u8>::new();
    luaparam::write_lua_params(&mut probe, lua_params).unwrap();
    let p_len = probe.len();
    if p_len <= 8 && !prefer_alt {
        return build_text_sheet_no_source_x38(
            source_actor_id,
            text_owner_actor_id,
            text_id,
            log_flag,
            lua_params,
        );
    }
    if p_len <= 16 {
        return build_text_sheet_no_source_x38_alt(
            source_actor_id,
            text_owner_actor_id,
            text_id,
            log_flag,
            lua_params,
        );
    }
    if p_len <= 32 {
        return build_text_sheet_no_source_x48(
            source_actor_id,
            text_owner_actor_id,
            text_id,
            log_flag,
            lua_params,
        );
    }
    build_text_sheet_no_source_x68(
        source_actor_id,
        text_owner_actor_id,
        text_id,
        log_flag,
        lua_params,
    )
}

pub const MESSAGE_TYPE_SAY: u8 = 0x01;
pub const MESSAGE_TYPE_SHOUT: u8 = 0x02;
pub const MESSAGE_TYPE_TELL: u8 = 0x03;
pub const MESSAGE_TYPE_PARTY: u8 = 0x04;
pub const MESSAGE_TYPE_LS: u8 = 0x05;
pub const MESSAGE_TYPE_YELL: u8 = 0x1D;
pub const MESSAGE_TYPE_SYSTEM: u8 = 0x20;
pub const MESSAGE_TYPE_SYSTEM_ERROR: u8 = 0x21;
/// Captured `log_flag` value on 0x0161 records in
/// `ffxiv_traces/accept_leve.pcapng` — `0x23`. Different from
/// `MESSAGE_TYPE_SYSTEM` (0x20); appears to be the "leve / quest"
/// log channel in 1.x.
pub const MESSAGE_TYPE_LEVE: u8 = 0x23;

// ---------------------------------------------------------------------------
// 0x0161-0x0165 "Text Sheet Message (DispId Sender)" family — system
// messages where the sender is identified by a display id (e.g. a leve
// content card's catalog id) rather than a runtime actor. Surveyed
// retail emissions are concentrated in `accept_leve.pcapng` (4× at
// 0x0161 30b tier); the larger 0x0162-0x0165 tiers had no emissions
// in the corpus but the family is here for symmetry.
//
// Wire format (decoded from `accept_leve.pcapng` 0x0161 records):
//
//   u32 disp_id          — display-id of the sender (catalog/leve id)
//   u32 actor_id         — contextualizing actor (varies; in the
//                           captures it's a 0x44D80000-prefix
//                           "leve content" actor)
//   u16 text_id          — text-sheet index
//   u8  log_flag         — captured 0x23 = MESSAGE_TYPE_LEVE
//   u8  pad
//   LuaParams            — variable, capacity per tier
//
// Tier table (size figures = SubPacket total):
//   0x0161 (30b) — body 16, params capacity  4  — header + 1 param
//   0x0162 (38b) — body 24, params capacity 12  — ~2 params
//   0x0163 (40b) — body 32, params capacity 20  — ~3 params
//   0x0164 (50b) — body 48, params capacity 36  — ~5 params
//   0x0165 (60b) — body 64, params capacity 52  — ~7 params

fn write_text_sheet_dispid_header(
    out: &mut Vec<u8>,
    disp_id: u32,
    actor_id: u32,
    text_id: u16,
    log_flag: u8,
) {
    out.write_u32::<LittleEndian>(disp_id).unwrap();
    out.write_u32::<LittleEndian>(actor_id).unwrap();
    out.write_u16::<LittleEndian>(text_id).unwrap();
    out.write_u8(log_flag).unwrap();
    out.write_u8(0).unwrap();
}

/// 0x0161 Text Sheet Message (DispId Sender) (30b). Body = 16 bytes
/// (12-byte header + 4 bytes for ~1 small LuaParam).
pub fn build_text_sheet_dispid_x30(
    receiver_actor_id: u32,
    disp_id: u32,
    sender_actor_id: u32,
    text_id: u16,
    log_flag: u8,
    lua_params: &[LuaParam],
) -> SubPacket {
    build_text_sheet_dispid_n(
        receiver_actor_id,
        disp_id,
        sender_actor_id,
        text_id,
        log_flag,
        lua_params,
        OP_TEXT_SHEET_DISPID_SENDER_X30,
        0x30,
    )
}

/// 0x0162 Text Sheet Message (DispId Sender) (38b). Body = 24 bytes.
pub fn build_text_sheet_dispid_x38(
    receiver_actor_id: u32,
    disp_id: u32,
    sender_actor_id: u32,
    text_id: u16,
    log_flag: u8,
    lua_params: &[LuaParam],
) -> SubPacket {
    build_text_sheet_dispid_n(
        receiver_actor_id,
        disp_id,
        sender_actor_id,
        text_id,
        log_flag,
        lua_params,
        OP_TEXT_SHEET_DISPID_SENDER_X38,
        0x38,
    )
}

/// 0x0163 Text Sheet Message (DispId Sender) (40b). Body = 32 bytes.
pub fn build_text_sheet_dispid_x40(
    receiver_actor_id: u32,
    disp_id: u32,
    sender_actor_id: u32,
    text_id: u16,
    log_flag: u8,
    lua_params: &[LuaParam],
) -> SubPacket {
    build_text_sheet_dispid_n(
        receiver_actor_id,
        disp_id,
        sender_actor_id,
        text_id,
        log_flag,
        lua_params,
        OP_TEXT_SHEET_DISPID_SENDER_X40,
        0x40,
    )
}

/// 0x0164 Text Sheet Message (DispId Sender) (50b). Body = 48 bytes.
pub fn build_text_sheet_dispid_x50(
    receiver_actor_id: u32,
    disp_id: u32,
    sender_actor_id: u32,
    text_id: u16,
    log_flag: u8,
    lua_params: &[LuaParam],
) -> SubPacket {
    build_text_sheet_dispid_n(
        receiver_actor_id,
        disp_id,
        sender_actor_id,
        text_id,
        log_flag,
        lua_params,
        OP_TEXT_SHEET_DISPID_SENDER_X50,
        0x50,
    )
}

/// 0x0165 Text Sheet Message (DispId Sender) (60b). Body = 64 bytes.
pub fn build_text_sheet_dispid_x60(
    receiver_actor_id: u32,
    disp_id: u32,
    sender_actor_id: u32,
    text_id: u16,
    log_flag: u8,
    lua_params: &[LuaParam],
) -> SubPacket {
    build_text_sheet_dispid_n(
        receiver_actor_id,
        disp_id,
        sender_actor_id,
        text_id,
        log_flag,
        lua_params,
        OP_TEXT_SHEET_DISPID_SENDER_X60,
        0x60,
    )
}

/// Auto-tier picker for the DispId-sender family — mirror of
/// `build_text_sheet_no_source_auto`. Picks the smallest 0x0161-0x0165
/// tier that fits the params (probing the serialized byte length), the
/// same way pmeteor's `GameMessagePacket.BuildPacket` DispId overload
/// selects its opcode. The man*l*1 NPC-linkshell narration passes NO
/// params → the 0x0161 (30b) tier. (Garlemald-Server #46.)
pub fn build_text_sheet_dispid_auto(
    receiver_actor_id: u32,
    disp_id: u32,
    sender_actor_id: u32,
    text_id: u16,
    log_flag: u8,
    lua_params: &[LuaParam],
) -> SubPacket {
    let mut probe = Vec::<u8>::new();
    luaparam::write_lua_params(&mut probe, lua_params).unwrap();
    let p_len = probe.len();
    let builder = if p_len <= 4 {
        build_text_sheet_dispid_x30
    } else if p_len <= 12 {
        build_text_sheet_dispid_x38
    } else if p_len <= 20 {
        build_text_sheet_dispid_x40
    } else if p_len <= 36 {
        build_text_sheet_dispid_x50
    } else {
        build_text_sheet_dispid_x60
    };
    builder(
        receiver_actor_id,
        disp_id,
        sender_actor_id,
        text_id,
        log_flag,
        lua_params,
    )
}

#[allow(clippy::too_many_arguments)]
fn build_text_sheet_dispid_n(
    receiver_actor_id: u32,
    disp_id: u32,
    sender_actor_id: u32,
    text_id: u16,
    log_flag: u8,
    lua_params: &[LuaParam],
    opcode: u16,
    packet_size: usize,
) -> SubPacket {
    let mut body_buf = Vec::<u8>::with_capacity(packet_size.saturating_sub(0x20));
    write_text_sheet_dispid_header(&mut body_buf, disp_id, sender_actor_id, text_id, log_flag);
    luaparam::write_lua_params(&mut body_buf, lua_params).unwrap();
    let mut data = body(packet_size);
    let n = body_buf.len().min(data.len());
    data[..n].copy_from_slice(&body_buf[..n]);
    SubPacket::new(opcode, receiver_actor_id, data)
}

/// 0x0003 SendMessagePacket — one chat-log line in the receiving
/// client. Wire layout from Meteor `SendMessagePacket.cs`
/// (PACKET_SIZE 0x248 → 0x228 body):
///   +0x00  sender name, ASCII, fixed 0x20 slot
///   +0x20  u32 message type (1 say … 0x20 system, 0x21 system error)
///   +0x24  message text, ASCII, max 0x200, zero-padded
///
/// The earlier port sent opcode 0x00CA with an ad-hoc layout; the
/// 1.23b client drops that frame without rendering (issue #10).
pub fn build_send_message(
    source_session: u32,
    target_session: u32,
    message_type: u8,
    sender_name: &str,
    message: &str,
) -> SubPacket {
    let mut data = body(0x248);
    let sender = sender_name.as_bytes();
    let n = sender.len().min(0x20);
    data[..n].copy_from_slice(&sender[..n]);
    data[0x20..0x24].copy_from_slice(&u32::from(message_type).to_le_bytes());
    let msg = message.as_bytes();
    let m = msg.len().min(0x200);
    data[0x24..0x24 + m].copy_from_slice(&msg[..m]);
    let mut sub = SubPacket::new(OP_SEND_MESSAGE, source_session, data);
    sub.set_target_id(target_session);
    sub
}

/// 0x0003 SendMessagePublic — system-wide (login greetings, shutdown notice).
pub fn build_send_message_public(
    source_actor_id: u32,
    message_type: u32,
    sender: &str,
    message: &str,
) -> SubPacket {
    let mut body = Vec::<u8>::with_capacity(0x248);
    body.write_u32::<LittleEndian>(message_type).unwrap();
    write_padded_ascii(&mut body, sender, 0x20);
    write_padded_ascii(&mut body, message, 0x200);
    SubPacket::new_with_flag(false, OP_SEND_MESSAGE_PUBLIC, source_actor_id, body)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Meteor `SendMessagePacket.cs` layout: sender in a 0x20 ASCII
    /// slot, u32 message type at 0x20, text from 0x24, fixed 0x228
    /// body, opcode 0x0003.
    #[test]
    fn send_message_matches_meteor_layout() {
        let pkt = build_send_message(2, 2, 0x20, "Sender", "hello");
        assert_eq!(pkt.data.len(), 0x228);
        assert_eq!(&pkt.data[..6], b"Sender");
        assert!(pkt.data[6..0x20].iter().all(|b| *b == 0));
        assert_eq!(&pkt.data[0x20..0x24], &[0x20, 0, 0, 0]);
        assert_eq!(&pkt.data[0x24..0x29], b"hello");
        assert!(pkt.data[0x29..].iter().all(|b| *b == 0));
        assert_eq!(pkt.game_message.opcode, OP_SEND_MESSAGE);
    }

    /// Reproduce the body bytes of `gather_wood.pcapng` 0x0166 record #1
    /// — sender = 0xA0F4E204 (gamedata static actor), text_id = 0x0024
    /// (decimal 36), log_flag = 0x20 (system message). Header-only,
    /// no LuaParams.
    #[test]
    fn text_sheet_no_source_x28_matches_retail_capture() {
        let pkt = build_text_sheet_no_source_x28(0x029B_2941, 0xA0F4_E204, 0x0024, 0x20);
        assert_eq!(pkt.data.len(), 8);
        assert_eq!(pkt.data, [0x04, 0xE2, 0xF4, 0xA0, 0x24, 0x00, 0x20, 0x00]);
        assert_eq!(pkt.game_message.opcode, OP_TEXT_SHEET_NO_ACTOR_X28);
    }

    /// Verify the 8-byte header for the larger tiers — captured retail
    /// 0x0167 record from `accept_quest.pcapng`:
    ///   sender = 0x5FF80001 (WorldMaster), text_id = 0x6288, log = 0x20.
    #[test]
    fn text_sheet_no_source_x38_header_matches_retail() {
        let pkt = build_text_sheet_no_source_x38(0x029B_2941, 0x5FF8_0001, 0x6288, 0x20, &[]);
        assert_eq!(pkt.data.len(), 24);
        assert_eq!(pkt.game_message.opcode, OP_TEXT_SHEET_NO_ACTOR_X38);
        assert_eq!(
            &pkt.data[..8],
            &[0x01, 0x00, 0xF8, 0x5F, 0x88, 0x62, 0x20, 0x00]
        );
    }

    #[test]
    fn text_sheet_no_source_x38_alt_uses_separate_opcode() {
        let pkt = build_text_sheet_no_source_x38_alt(0x029B_2941, 0x5FF8_0001, 1, 0x20, &[]);
        assert_eq!(pkt.data.len(), 24);
        assert_eq!(pkt.game_message.opcode, OP_TEXT_SHEET_NO_ACTOR_X38_ALT);
    }

    #[test]
    fn text_sheet_no_source_x48_size() {
        let pkt = build_text_sheet_no_source_x48(0x029B_2941, 0x5FF8_0001, 1, 0x20, &[]);
        assert_eq!(pkt.data.len(), 40);
        assert_eq!(pkt.game_message.opcode, OP_TEXT_SHEET_NO_ACTOR_X48);
    }

    #[test]
    fn text_sheet_no_source_x68_size() {
        let pkt = build_text_sheet_no_source_x68(0x029B_2941, 0x5FF8_0001, 1, 0x20, &[]);
        assert_eq!(pkt.data.len(), 72);
        assert_eq!(pkt.game_message.opcode, OP_TEXT_SHEET_NO_ACTOR_X68);
    }

    /// Reproduce the captured 0x0161 record from
    /// `accept_leve.pcapng` #1 — `disp_id = 0x00124FFB`,
    /// `actor_id = 0x44D8000A`, `text_id = 0x000D`,
    /// `log_flag = 0x23` (MESSAGE_TYPE_LEVE), 4 bytes trailing zero
    /// because no LuaParams are passed.
    #[test]
    fn text_sheet_dispid_x30_matches_retail_capture() {
        let pkt = build_text_sheet_dispid_x30(
            0x029B_2941,
            0x0012_4FFB,
            0x44D8_000A,
            0x000D,
            MESSAGE_TYPE_LEVE,
            &[],
        );
        assert_eq!(pkt.data.len(), 16);
        assert_eq!(pkt.game_message.opcode, OP_TEXT_SHEET_DISPID_SENDER_X30);
        // First 12 bytes are the header (verified byte-for-byte
        // against capture); last 4 bytes contain the LUA_END marker
        // (0x0F) followed by zero pad — but we don't assert exact
        // tail bytes since LuaParam encoding is the source of the
        // mismatch noted in the No-Source family caveats.
        assert_eq!(
            &pkt.data[..12],
            &[
                0xFB, 0x4F, 0x12, 0x00, // disp_id LE
                0x0A, 0x00, 0xD8, 0x44, // actor_id LE
                0x0D, 0x00, // text_id LE
                0x23, 0x00, // log_flag + pad
            ],
        );
    }

    #[test]
    fn text_sheet_dispid_tier_sizes() {
        let cases: &[(u16, usize)] = &[
            (OP_TEXT_SHEET_DISPID_SENDER_X30, 16),
            (OP_TEXT_SHEET_DISPID_SENDER_X38, 24),
            (OP_TEXT_SHEET_DISPID_SENDER_X40, 32),
            (OP_TEXT_SHEET_DISPID_SENDER_X50, 48),
            (OP_TEXT_SHEET_DISPID_SENDER_X60, 64),
        ];
        let pkts = [
            build_text_sheet_dispid_x30(1, 2, 3, 4, 0, &[]),
            build_text_sheet_dispid_x38(1, 2, 3, 4, 0, &[]),
            build_text_sheet_dispid_x40(1, 2, 3, 4, 0, &[]),
            build_text_sheet_dispid_x50(1, 2, 3, 4, 0, &[]),
            build_text_sheet_dispid_x60(1, 2, 3, 4, 0, &[]),
        ];
        for (pkt, (opcode, body_size)) in pkts.iter().zip(cases.iter()) {
            assert_eq!(pkt.game_message.opcode, *opcode);
            assert_eq!(pkt.data.len(), *body_size);
        }
    }

    #[test]
    fn text_sheet_no_source_auto_picks_smallest_tier() {
        // No params → 0x0166 (28b)
        let p0 = build_text_sheet_no_source_auto(1, 2, 3, 0x20, &[], false);
        assert_eq!(p0.game_message.opcode, OP_TEXT_SHEET_NO_ACTOR_X28);
        assert_eq!(p0.data.len(), 8);

        // One Int32 param → 0x0167 (38b)
        let p1 = build_text_sheet_no_source_auto(1, 2, 3, 0x20, &[LuaParam::Int32(42)], false);
        assert_eq!(p1.game_message.opcode, OP_TEXT_SHEET_NO_ACTOR_X38);
        assert_eq!(p1.data.len(), 24);

        // prefer_alt swaps to 0x0168.
        let p1a = build_text_sheet_no_source_auto(1, 2, 3, 0x20, &[LuaParam::Int32(42)], true);
        assert_eq!(p1a.game_message.opcode, OP_TEXT_SHEET_NO_ACTOR_X38_ALT);

        // Many params → 0x0169 (48b) or 0x016A (68b).
        let many = vec![LuaParam::Int32(1); 4]; // 4 × 6 bytes + 1 LUA_END = 25 bytes
        let pn = build_text_sheet_no_source_auto(1, 2, 3, 0x20, &many, false);
        assert_eq!(pn.game_message.opcode, OP_TEXT_SHEET_NO_ACTOR_X48);

        let huge = vec![LuaParam::Int32(1); 8]; // 8 × 6 + 1 = 49 bytes
        let ph = build_text_sheet_no_source_auto(1, 2, 3, 0x20, &huge, false);
        assert_eq!(ph.game_message.opcode, OP_TEXT_SHEET_NO_ACTOR_X68);
    }

    /// Byte-exact regression test for the 0x0167 tier against pmeteor's
    /// captured login toast (`captures/pmeteor-quest/
    /// 20260426-160210-gridania-manual3/map-packets.log:829` — the
    /// "quest added to journal" message, text 25224, param quest
    /// 110005). Two details the 1.23b client VALIDATES (omitting either
    /// made it abort via CRT `_invalid_parameter`, exception 0xC000000D,
    /// when the toast was first actually delivered):
    ///   1. header source = WorldMaster 0x5FF80001 (per-actor receiver
    ///      dispatch keys on it),
    ///   2. the `u32 = 8` params-region marker at body offset 0x10
    ///      (GameMessagePacket.cs:332-336).
    #[test]
    fn x38_quest_toast_matches_pmeteor_capture() {
        let pkt = build_text_sheet_no_source_auto(
            WORLD_MASTER_ACTOR_ID,
            WORLD_MASTER_ACTOR_ID,
            25224,
            MESSAGE_TYPE_SYSTEM,
            &[LuaParam::UInt32(110005)],
            false,
        );
        assert_eq!(pkt.game_message.opcode, OP_TEXT_SHEET_NO_ACTOR_X38);
        assert_eq!(pkt.header.source_id, WORLD_MASTER_ACTOR_ID);
        // pmeteor capture body bytes (0x18 of them):
        //   01 00 F8 5F  88 62  20 00  00 00 01 AD B5 0F 00 00  08 00 00 00  00 00 00 00
        let expected: [u8; 0x18] = [
            0x01, 0x00, 0xF8, 0x5F, // textOwner = WorldMaster
            0x88, 0x62, // text_id 25224
            0x20, 0x00, // log 0x20
            0x00, 0x00, 0x01, 0xAD, 0xB5, 0x0F, 0x00, 0x00, // LuaParams: UInt32 110005 + end
            0x08, 0x00, 0x00, 0x00, // mandatory params-region marker
            0x00, 0x00, 0x00, 0x00,
        ];
        assert_eq!(
            pkt.data[..],
            expected[..],
            "0x0167 body must byte-match the pmeteor reference capture",
        );
    }

    /// Garlemald-Server #46 — the "You obtain <item>" toast (text 25117 +
    /// item id 11000125) must carry the item-id LuaParam so the client
    /// resolves the name (it was rendering "You obtain a ." because the
    /// param was dropped). One small int param picks GameMessageWithActor2
    /// (0x0158) and requires the `u32 = 8` marker at body 0x14.
    #[test]
    fn game_message_actor1_with_params_carries_item_id() {
        let pkt = build_game_message_actor1_with_params(
            WORLD_MASTER_ACTOR_ID, // source
            2,                     // actorId (the player)
            WORLD_MASTER_ACTOR_ID, // textOwnerActorId (system sheet)
            25117,
            0x20,
            &[LuaParam::Int32(11_000_125)],
        );
        assert_eq!(pkt.game_message.opcode, OP_GAME_MESSAGE_ACTOR2);
        assert_eq!(pkt.data.len(), 0x38 - 0x20);
        assert_eq!(u32::from_le_bytes(pkt.data[0..4].try_into().unwrap()), 2);
        assert_eq!(
            u32::from_le_bytes(pkt.data[4..8].try_into().unwrap()),
            WORLD_MASTER_ACTOR_ID
        );
        assert_eq!(
            u16::from_le_bytes(pkt.data[8..10].try_into().unwrap()),
            25117
        );
        assert_eq!(
            u16::from_le_bytes(pkt.data[10..12].try_into().unwrap()),
            0x20
        );
        // Mandatory small-params-region marker at body offset 0x14.
        assert_eq!(
            u32::from_le_bytes(pkt.data[0x14..0x18].try_into().unwrap()),
            8,
            "the WITH-params game-message needs the u32=8 marker at 0x14",
        );
        // The item id must round-trip through the LuaParam region (offset 12).
        let parsed = luaparam::read_lua_params(&pkt.data[12..]).unwrap();
        assert!(
            matches!(parsed.first(), Some(LuaParam::Int32(11_000_125))),
            "item-id param must decode back to 11000125; got {parsed:?}",
        );
    }
}
