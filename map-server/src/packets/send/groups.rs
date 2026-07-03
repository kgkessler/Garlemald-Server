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

//! Group/party/linkshell sync packets. Similar chunked shape to inventory:
//! `GroupMembersBegin` → one or more `GroupMembers{X08,X16,X32,X64}` →
//! `GroupMembersEnd`, plus the optional named/content/sync variants.

use std::io::{Cursor, Write};

use byteorder::{LittleEndian, WriteBytesExt};
use common::subpacket::SubPacket;

use super::super::opcodes::*;
use super::{body, write_padded_ascii};

/// One per-member row serialized into the X08/X16/X32/X64 containers.
/// Fields mirror Meteor's `GroupMember` (used by `GroupMembersXnn.buildPacket`),
/// not the party-UI fields the earlier port invented. The client's
/// group-table parser expects a fixed 0x30-byte slot with this layout;
/// writing the HP/class/etc. shape we had before was what turned the
/// empty-retainer-sync packets into a hard Wine crash.
#[derive(Debug, Clone, Default)]
pub struct GroupMember {
    pub actor_id: u32,
    /// C# `localizedName` — signed 32-bit (retainer/linkshell title id).
    pub localized_name: i32,
    /// Opaque 32-bit slot — zero for retainers, class-bit-flags for LS.
    pub unknown2: u32,
    /// `flag1` in C# (leader / officer bit).
    pub flag1: bool,
    pub is_online: bool,
    /// Up to 0x20 bytes of ASCII name.
    pub name: String,
}

impl GroupMember {
    /// Party-row encoding for an actor in the extended-temp group 10001
    /// (0x2711) — the ONLY group the client's party HUD reads: decomp
    /// `CharaBaseClass:getPlayerParty` is literally
    /// `self:_getExtendedTemporaryGroup(10001)`, and the
    /// PartyParameterWidget (bottom-right ally rows — DESIGNED for NPC
    /// members, it renders `???` when `actor:isPlayer() == false`)
    /// resolves each row's label from these fields:
    ///
    /// * PLAYER (non-empty display name): `localized_name = -1`
    ///   ("custom name used", per the 0x017C wiki note) + the name
    ///   string in the 0x20-byte slot field.
    /// * NPC (empty display name, e.g. a `currentParty:AddMember`'d
    ///   tutorial ally like Y'shtola): retail's 10001 X08 rows carry
    ///   the NPC's LOCALIZED DISPLAY-NAME ID in `localized_name` with
    ///   an empty name string — the client looks the label up in its
    ///   own text sheets. Garlemald's emitters used to ship
    ///   `{ localized_name: -1, name: "" }` for NPCs (their
    ///   `display_name()` is empty), which the widget draws as nothing
    ///   — the round-1 "party-adds don't render" break.
    ///
    /// `BaseActor::new` seeds `display_name_id = 0xFFFFFFFF`, so an NPC
    /// without a real display-name id degrades to `-1` — the previous
    /// (blank-row) behaviour, never garbage. (Garlemald-Server #46,
    /// round 4.)
    pub fn row_for_actor(actor_id: u32, display_name: &str, display_name_id: u32) -> Self {
        if display_name.is_empty() {
            Self {
                actor_id,
                localized_name: display_name_id as i32,
                unknown2: 0,
                flag1: false,
                is_online: true,
                name: String::new(),
            }
        } else {
            Self {
                actor_id,
                localized_name: -1,
                unknown2: 0,
                flag1: false,
                is_online: true,
                name: display_name.to_string(),
            }
        }
    }
}

/// Fixed member-slot size in Meteor: u32+i32+u32+byte+byte+name[0x20] =
/// 0x2E bytes written, slot is 0x30 with two trailing pad bytes.
const GROUP_MEMBER_SLOT_BYTES: usize = 0x30;

fn encode_group_member_at(data: &mut [u8], slot_offset: usize, m: &GroupMember) {
    let mut c = Cursor::new(&mut data[slot_offset..slot_offset + GROUP_MEMBER_SLOT_BYTES]);
    c.write_u32::<LittleEndian>(m.actor_id).unwrap();
    c.write_i32::<LittleEndian>(m.localized_name).unwrap();
    c.write_u32::<LittleEndian>(m.unknown2).unwrap();
    c.write_u8(if m.flag1 { 1 } else { 0 }).unwrap();
    c.write_u8(if m.is_online { 1 } else { 0 }).unwrap();
    write_padded_ascii(&mut c, &m.name, 0x20);
}

/// 0x017C GroupHeaderPacket — first packet in a group sync.
///
/// Meteor layout (body size 0x78):
///   0x00  locationCode        u64  (zone id)
///   0x08  sequenceId          u64  (timestamp)
///   0x10  const               u64  = 3
///   0x18  group_index         u64
///   0x20  const               u64  = 0
///   0x28  group_index         u64
///   0x30  type_id             u32
///   0x40  localized_name      u32  (linkshell display id, 0 otherwise)
///   0x44  group_name (ASCII)  up to 0x20 bytes
///   0x64  const               u32  = 0x6D
///   0x68  const               u32  = 0x6D
///   0x6C  const               u32  = 0x6D
///   0x70  const               u32  = 0x6D
///   0x74  member_count        u32
#[allow(clippy::too_many_arguments)]
pub fn build_group_header(
    source_actor_id: u32,
    location_code: u64,
    sequence_id: u64,
    group_index: u64,
    type_id: u32,
    localized_name: i32,
    group_name: &str,
    member_count: u32,
) -> SubPacket {
    let mut data = body(0x98);
    {
        let mut c = Cursor::new(&mut data[..]);
        c.write_u64::<LittleEndian>(location_code).unwrap();
        c.write_u64::<LittleEndian>(sequence_id).unwrap();
        c.write_u64::<LittleEndian>(3).unwrap();
        c.write_u64::<LittleEndian>(group_index).unwrap();
        c.write_u64::<LittleEndian>(0).unwrap();
        c.write_u64::<LittleEndian>(group_index).unwrap();
        c.write_u32::<LittleEndian>(type_id).unwrap();
    }
    // 0x40 region — C# seeks past the u32 type_id before writing the
    // linkshell display bits.
    {
        let mut c = Cursor::new(&mut data[..]);
        c.set_position(0x40);
        c.write_i32::<LittleEndian>(localized_name).unwrap();
        write_padded_ascii(&mut c, group_name, 0x20);
        c.set_position(0x64);
        c.write_u32::<LittleEndian>(0x6D).unwrap();
        c.write_u32::<LittleEndian>(0x6D).unwrap();
        c.write_u32::<LittleEndian>(0x6D).unwrap();
        c.write_u32::<LittleEndian>(0x6D).unwrap();
        c.write_u32::<LittleEndian>(member_count).unwrap();
    }
    SubPacket::new(OP_GROUP_HEADER, source_actor_id, data)
}

/// 0x017D GroupMembersBeginPacket. Body layout (0x20 bytes):
///   0x00 locationCode  u64
///   0x08 sequenceId    u64
///   0x10 group_index   u64
///   0x18 member_count  u32
pub fn build_group_members_begin(
    source_actor_id: u32,
    location_code: u64,
    sequence_id: u64,
    group_index: u64,
    member_count: u32,
) -> SubPacket {
    let mut data = body(0x40);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u64::<LittleEndian>(location_code).unwrap();
    c.write_u64::<LittleEndian>(sequence_id).unwrap();
    c.write_u64::<LittleEndian>(group_index).unwrap();
    c.write_u32::<LittleEndian>(member_count).unwrap();
    SubPacket::new(OP_GROUP_MEMBERS_BEGIN, source_actor_id, data)
}

/// 0x017E GroupMembersEndPacket. Body layout (0x18 bytes):
///   0x00 locationCode  u64
///   0x08 sequenceId    u64
///   0x10 group_index   u64
pub fn build_group_members_end(
    source_actor_id: u32,
    location_code: u64,
    sequence_id: u64,
    group_index: u64,
) -> SubPacket {
    let mut data = body(0x38);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u64::<LittleEndian>(location_code).unwrap();
    c.write_u64::<LittleEndian>(sequence_id).unwrap();
    c.write_u64::<LittleEndian>(group_index).unwrap();
    SubPacket::new(OP_GROUP_MEMBERS_END, source_actor_id, data)
}

/// 0x017F GroupMembersX08Packet — up to 8 members per packet.
///
/// Meteor's `GroupMembersX08Packet.buildPacket` lays the body as:
///   0x00  locationCode            u64
///   0x08  sequenceId              u64
///   0x10  member_slot[8] @ 0x30   (empty slots stay zero-filled)
///   0x190 count                   u32  — number of members written
/// The client indexes the slot array by position (not by walking a
/// sequential stream), so we must respect the fixed 0x30 spacing even
/// for empty slots.
pub fn build_group_members_x08(
    source_actor_id: u32,
    location_code: u64,
    sequence_id: u64,
    members: &[GroupMember],
    list_offset: &mut usize,
) -> SubPacket {
    build_group_members_n(
        source_actor_id,
        location_code,
        sequence_id,
        members,
        list_offset,
        8,
        OP_GROUP_MEMBERS_X08,
        0x1B8,
    )
}

pub fn build_group_members_x16(
    source_actor_id: u32,
    location_code: u64,
    sequence_id: u64,
    members: &[GroupMember],
    list_offset: &mut usize,
) -> SubPacket {
    build_group_members_n(
        source_actor_id,
        location_code,
        sequence_id,
        members,
        list_offset,
        16,
        OP_GROUP_MEMBERS_X16,
        0x330,
    )
}

pub fn build_group_members_x32(
    source_actor_id: u32,
    location_code: u64,
    sequence_id: u64,
    members: &[GroupMember],
    list_offset: &mut usize,
) -> SubPacket {
    build_group_members_n(
        source_actor_id,
        location_code,
        sequence_id,
        members,
        list_offset,
        32,
        OP_GROUP_MEMBERS_X32,
        0x630,
    )
}

pub fn build_group_members_x64(
    source_actor_id: u32,
    location_code: u64,
    sequence_id: u64,
    members: &[GroupMember],
    list_offset: &mut usize,
) -> SubPacket {
    build_group_members_n(
        source_actor_id,
        location_code,
        sequence_id,
        members,
        list_offset,
        64,
        OP_GROUP_MEMBERS_X64,
        0xC30,
    )
}

#[allow(clippy::too_many_arguments)]
fn build_group_members_n(
    source_actor_id: u32,
    location_code: u64,
    sequence_id: u64,
    members: &[GroupMember],
    list_offset: &mut usize,
    cap: usize,
    opcode: u16,
    packet_size: usize,
) -> SubPacket {
    let mut data = body(packet_size);
    let max = members.len().saturating_sub(*list_offset).min(cap);
    // Header prefix.
    {
        let mut c = Cursor::new(&mut data[..]);
        c.write_u64::<LittleEndian>(location_code).unwrap();
        c.write_u64::<LittleEndian>(sequence_id).unwrap();
    }
    // Member slots — fixed 0x30 spacing starting at body offset 0x10.
    for i in 0..max {
        encode_group_member_at(
            &mut data,
            0x10 + (GROUP_MEMBER_SLOT_BYTES * i),
            &members[*list_offset + i],
        );
    }
    // Count — Meteor writes it at body offset 0x10 + 0x30*cap (past the
    // last slot). For X08 that's 0x190; X16/X32/X64 scale.
    let count_offset = 0x10 + (GROUP_MEMBER_SLOT_BYTES * cap);
    if count_offset + 4 <= data.len() {
        data[count_offset..count_offset + 4].copy_from_slice(&(max as u32).to_le_bytes());
    }
    *list_offset += max;
    SubPacket::new(opcode, source_actor_id, data)
}

/// Per-member stride for the CONTENT (instance/duty) group list — distinct
/// from the party `GROUP_MEMBER_SLOT_BYTES` (0x30). pmeteor
/// `ContentMembersX08Packet.cs` writes a 12-byte entry: `(actorId u32,
/// 1001 layoutId u32, 1 u32)`.
const CONTENT_MEMBER_SLOT_BYTES: usize = 0x0C;

/// Content (instance/duty) member list body. CRITICAL: this is NOT the party
/// (0x017F) layout — the 1.x client parses the content opcodes (0x0183-0x0186)
/// with a 12-byte stride `(actorId, 1001, 1)`, count written only when it fits
/// (X08 at body 0x70; X16/X32/X64 fill the body so the count is omitted, which
/// the offset guard below reproduces — matching pmeteor exactly). The prior
/// code reused `build_group_members_n` (the 0x30 party stride), so the client
/// misread the content roster and Yda/Papalymo never appeared in the panel.
/// (Garlemald-Server #28; pmeteor `ContentMembersX08Packet.cs:49-62`.)
#[allow(clippy::too_many_arguments)]
fn build_content_members_n(
    source_actor_id: u32,
    location_code: u64,
    sequence_id: u64,
    members: &[GroupMember],
    list_offset: &mut usize,
    cap: usize,
    opcode: u16,
    packet_size: usize,
) -> SubPacket {
    let mut data = body(packet_size);
    let max = members.len().saturating_sub(*list_offset).min(cap);
    {
        let mut c = Cursor::new(&mut data[..]);
        c.write_u64::<LittleEndian>(location_code).unwrap();
        c.write_u64::<LittleEndian>(sequence_id).unwrap();
    }
    for i in 0..max {
        let off = 0x10 + (CONTENT_MEMBER_SLOT_BYTES * i);
        let mut c = Cursor::new(&mut data[off..off + CONTENT_MEMBER_SLOT_BYTES]);
        c.write_u32::<LittleEndian>(members[*list_offset + i].actor_id)
            .unwrap();
        c.write_u32::<LittleEndian>(1001).unwrap(); // layout id (pmeteor constant)
        c.write_u32::<LittleEndian>(1).unwrap();
    }
    // Count at body 0x10 + 0xC*cap (0x70 for X08). For X16/X32/X64 this offset
    // lands exactly at the end of the body, so the guard skips it — pmeteor
    // omits the count for those variants too.
    let count_offset = 0x10 + (CONTENT_MEMBER_SLOT_BYTES * cap);
    if count_offset + 4 <= data.len() {
        data[count_offset..count_offset + 4].copy_from_slice(&(max as u32).to_le_bytes());
    }
    *list_offset += max;
    SubPacket::new(opcode, source_actor_id, data)
}

/// Content (instance/duty) member variants — distinct 0xC stride per member.
pub fn build_content_members_x08(
    source_actor_id: u32,
    location_code: u64,
    sequence_id: u64,
    members: &[GroupMember],
    list_offset: &mut usize,
) -> SubPacket {
    build_content_members_n(
        source_actor_id,
        location_code,
        sequence_id,
        members,
        list_offset,
        8,
        OP_CONTENT_MEMBERS_X08,
        0x1B8,
    )
}
pub fn build_content_members_x16(
    source_actor_id: u32,
    location_code: u64,
    sequence_id: u64,
    members: &[GroupMember],
    list_offset: &mut usize,
) -> SubPacket {
    build_content_members_n(
        source_actor_id,
        location_code,
        sequence_id,
        members,
        list_offset,
        16,
        OP_CONTENT_MEMBERS_X16,
        0xF0,
    )
}
pub fn build_content_members_x32(
    source_actor_id: u32,
    location_code: u64,
    sequence_id: u64,
    members: &[GroupMember],
    list_offset: &mut usize,
) -> SubPacket {
    build_content_members_n(
        source_actor_id,
        location_code,
        sequence_id,
        members,
        list_offset,
        32,
        OP_CONTENT_MEMBERS_X32,
        0x1B0,
    )
}
pub fn build_content_members_x64(
    source_actor_id: u32,
    location_code: u64,
    sequence_id: u64,
    members: &[GroupMember],
    list_offset: &mut usize,
) -> SubPacket {
    build_content_members_n(
        source_actor_id,
        location_code,
        sequence_id,
        members,
        list_offset,
        64,
        OP_CONTENT_MEMBERS_X64,
        0x330,
    )
}

/// 0x0188 CreateNamedGroup — announce a new group by name.
pub fn build_create_named_group(
    source_actor_id: u32,
    group_id: u64,
    group_type: u16,
    name: &str,
    master: &str,
) -> SubPacket {
    let mut data = body(0x60);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u64::<LittleEndian>(group_id).unwrap();
    c.write_u16::<LittleEndian>(group_type).unwrap();
    write_padded_ascii(&mut c, name, 0x20);
    write_padded_ascii(&mut c, master, 0x20);
    SubPacket::new(OP_CREATE_NAMED_GROUP, source_actor_id, data)
}

/// 0x0189 CreateNamedGroupMultiple — batch LS list.
pub fn build_create_named_group_multiple(
    source_actor_id: u32,
    linkshells: &[(u64, String, u16)],
) -> SubPacket {
    let mut data = body(0x228);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u16::<LittleEndian>(linkshells.len() as u16)
        .unwrap();
    c.write_u16::<LittleEndian>(0).unwrap();
    for (gid, name, gtype) in linkshells {
        c.write_u64::<LittleEndian>(*gid).unwrap();
        c.write_u16::<LittleEndian>(*gtype).unwrap();
        write_padded_ascii(&mut c, name, 0x20);
    }
    SubPacket::new(OP_CREATE_NAMED_GROUP_MULTIPLE, source_actor_id, data)
}

/// 0x0143 DeleteGroupPacket.
pub fn build_delete_group(source_actor_id: u32, group_id: u64) -> SubPacket {
    let mut data = body(0x40);
    data[..8].copy_from_slice(&group_id.to_le_bytes());
    SubPacket::new(OP_DELETE_GROUP, source_actor_id, data)
}

/// 0x017A SynchGroupWorkValuesPacket — raw work-value blob (Phase-4 placeholder).
pub fn build_synch_group_work_values(
    source_actor_id: u32,
    group_id: u64,
    work_blob: &[u8],
) -> SubPacket {
    let mut data = body(0xB0);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u64::<LittleEndian>(group_id).unwrap();
    let n = work_blob.len().min(0xA0);
    for &b in work_blob.iter().take(n) {
        c.write_u8(b).unwrap();
    }
    SubPacket::new(OP_SYNCH_GROUP_WORK_VALUES, source_actor_id, data)
}

/// 0x017A SynchGroupWorkValuesPacket — content-group `/_init` reply.
///
/// Mirrors pmeteor `ContentGroup.SendInitWorkValues` (Map Server/Actors/
/// Group/ContentGroup.cs:105):
/// ```csharp
/// SynchGroupWorkValuesPacket groupWork = new SynchGroupWorkValuesPacket(groupIndex);
/// groupWork.addProperty(this, "contentGroupWork._globalTemp.director");
/// groupWork.addByte(MurmurHash2("contentGroupWork.property[0]", 0), 1);
/// groupWork.setTarget("/_init");
/// ```
///
/// The 1.x client sends 0x0133 GroupCreated for the director's `/_init`
/// after constructing the director-side group (in response to the
/// director's own `/_init` SetActorProperty in the spawn bundle). Without
/// this reply, the client's content-group state machine sits forever
/// waiting for the director property, so the cinematic body
/// (RunEventFunction sequence) never fires and "Now Loading" never
/// clears.
///
/// Wire layout (per pmeteor's `SynchGroupWorkValuesPacket.cs`):
///   body[0..8]   = group_id (u64)
///   body[8]      = runningByteTotal (u8) — total bytes of property
///                  entries + target, written last
///   body[9..]    = property entries:
///     - int property: byte(type=4) + u32(id) + u32(value)         → 9 bytes
///     - byte property: byte(type=1) + u32(id) + byte(value)        → 6 bytes
///   then target:
///     - byte(0x82 + len) + ASCII bytes                             → 1 + len
///   remainder: zero padding to 0x90 bytes total body
pub fn build_synch_group_work_values_content_init(
    source_actor_id: u32,
    group_id: u64,
    director_actor_id: u32,
) -> SubPacket {
    let mut data = body(0xB0);
    let mut c = Cursor::new(&mut data[..]);
    // Group id at offset 0..8.
    c.write_u64::<LittleEndian>(group_id).unwrap();
    // Reserve offset 8 for runningByteTotal (written at end).
    let mut running: u8 = 0;
    c.set_position(9);
    // Property entry 1: long contentGroupWork._globalTemp.director.
    //
    // Pmeteor `ContentGroup.cs:55` stores the director-id field as
    // `(ulong)director.Id << 32` — a 64-bit value with the
    // director_actor_id in the HIGH 32 bits and zero in the LOW.
    // `addProperty` then dispatches through the property-type table
    // and selects `addLong` (Long-typed property entry), which
    // writes `byte(8) + u32(hash) + u64(value)` = 13 bytes total.
    //
    // Earlier garlemald iteration (commit 93eb62b) used `addInt`
    // shape (`byte(4) + u32(hash) + u32(value)` = 9 bytes), which
    // made the value's u32 overlap the next entry's type byte and
    // the client rejected the SyncWriter buffer mid-parse — the
    // WorkSyncUpdater never marked the director event-ready, IN
    // 0x012D for director never fired, and the SEQ_005 cinematic
    // never started. Verified vs pmeteor's reference capture
    // (captures/pmeteor-quest/20260426-160210-gridania-manual3/
    // map-packets.log line 33943) which writes type=8 with body
    // bytes `00 00 00 00 03 00 30 65` for director_actor_id
    // 0x65300003. Garlemald's d9896de capture (line 10686) wrote
    // type=4 with body bytes `03 00 30 65` — confirming the
    // mismatch.
    c.write_u8(8).unwrap(); // type=long
    c.write_u32::<LittleEndian>(common::utils::murmur_hash2(
        "contentGroupWork._globalTemp.director",
        0,
    ))
    .unwrap();
    c.write_u64::<LittleEndian>((director_actor_id as u64) << 32)
        .unwrap();
    running += 13;
    // Property entry 2: byte contentGroupWork.property[0] = 1
    c.write_u8(1).unwrap(); // type=byte
    c.write_u32::<LittleEndian>(common::utils::murmur_hash2(
        "contentGroupWork.property[0]",
        0,
    ))
    .unwrap();
    c.write_u8(1).unwrap();
    running += 6;
    // Target marker: 0x82 + target.len(), then ASCII bytes.
    let target = b"/_init";
    c.write_u8(0x82u8 + target.len() as u8).unwrap();
    c.write_all(target).unwrap();
    running += 1 + target.len() as u8;
    // Backfill runningByteTotal at offset 8.
    data[8] = running;
    SubPacket::new(OP_SYNCH_GROUP_WORK_VALUES, source_actor_id, data)
}

// ---------------------------------------------------------------------------
// 0x018D PartyMapMarkerUpdate — party-member icon overlay on the world map.
//
// Wire format (per wiki + retail bytes from `ffxiv_traces/chat_say.pcapng`
// record #1 of opcode 0x018D, decoded byte-by-byte):
//
//   body size = 0x298 (664 bytes)
//   0x00  u64 player_group_id          — solo retail uses 0x80000000_0077E9AC,
//                                         party uses Meteor's
//                                         `((leader_id as u64) << 32) | 0xB36F92`
//   0x08  u32 group_type               — 10001 (0x2711) = PlayerPartyGroup
//   0x0C  u32 zero/padding
//   0x10  marker[16] @ 40 bytes each   = 640 bytes
//   0x290 u32 num_entries              — count of populated marker slots
//   0x294 u32 zero/padding
//
// Per-marker layout (40 bytes), at marker-relative offsets:
//   0x00  u32 player_id (actor id)
//   0x04  u32 zero/padding
//   0x08  u32 unknown                  — wiki: "each player has a different
//                                         value" — likely a per-character
//                                         hash or session salt; client
//                                         appears not to validate
//   0x0C  u64 zero/padding
//   0x14  f32 x
//   0x18  f32 y
//   0x1C  f32 z
//   0x20  f32 orientation
//   0x24  u32 zero/padding
//
// Retail emits this on a regular interval (every position broadcast in
// our captures); see the wiki note: "Sent from the server at a regular
// interval, likely due to client not being programmed to send a request
// for such data when the player opens the map."
//
// Project Meteor never implemented this; with the retail-pcap audit
// (2026-05-02), garlemald becomes the first 1.x port to emit it.

/// One marker slot inside a 0x018D packet. `unknown` is opaque per the
/// wiki — pass 0 for a clean default; production code may want to seed
/// it from a per-character salt for full retail conformance.
#[derive(Debug, Clone, Copy, Default)]
pub struct PartyMapMarker {
    pub player_id: u32,
    pub unknown: u32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub orientation: f32,
}

/// 0x10001 / 10001 — `Group::PlayerPartyGroup` per Meteor
/// `World Server/DataObjects/Group/Group.cs`.
pub const PARTY_MAP_MARKER_GROUP_TYPE_PLAYER_PARTY: u32 = 10001;

/// playerGroupID retail uses for an unparty'd player. Magic constant
/// captured from `ffxiv_traces/chat_say.pcapng`; the high 0x80000000
/// bit looks like a "synthetic / solo group" flag, but we don't have
/// enough datapoints to confirm. Use this verbatim for solo emissions
/// until we capture another player's solo packet.
pub const PARTY_MAP_MARKER_SOLO_GROUP_ID: u64 = 0x8000_0000_0077_E9AC;

/// 0x018D PartyMapMarkerUpdate. Up to 16 markers per packet — extra
/// markers in `markers` are silently truncated.
pub fn build_party_map_marker_update(
    actor_id: u32,
    player_group_id: u64,
    group_type: u32,
    markers: &[PartyMapMarker],
) -> SubPacket {
    const PACKET_SIZE: usize = 0x2B8;
    const MARKER_SIZE: usize = 0x28;
    const MARKERS_OFFSET: usize = 0x10;
    const NUM_ENTRIES_OFFSET: usize = 0x290;
    const MAX_MARKERS: usize = 16;

    let mut data = body(PACKET_SIZE);
    {
        let mut c = Cursor::new(&mut data[..]);
        c.write_u64::<LittleEndian>(player_group_id).unwrap();
        c.write_u32::<LittleEndian>(group_type).unwrap();
        // 4-byte pad at 0x0C..0x10 already zero from `body()`.
    }
    let n = markers.len().min(MAX_MARKERS);
    for (i, m) in markers.iter().take(MAX_MARKERS).enumerate() {
        let off = MARKERS_OFFSET + i * MARKER_SIZE;
        let mut c = Cursor::new(&mut data[off..off + MARKER_SIZE]);
        c.write_u32::<LittleEndian>(m.player_id).unwrap();
        c.write_u32::<LittleEndian>(0).unwrap(); // 0x04 pad
        c.write_u32::<LittleEndian>(m.unknown).unwrap();
        c.write_u64::<LittleEndian>(0).unwrap(); // 0x0C..0x14 pad
        c.write_f32::<LittleEndian>(m.x).unwrap();
        c.write_f32::<LittleEndian>(m.y).unwrap();
        c.write_f32::<LittleEndian>(m.z).unwrap();
        c.write_f32::<LittleEndian>(m.orientation).unwrap();
        c.write_u32::<LittleEndian>(0).unwrap(); // 0x24 pad
    }
    data[NUM_ENTRIES_OFFSET..NUM_ENTRIES_OFFSET + 4].copy_from_slice(&(n as u32).to_le_bytes());
    SubPacket::new(OP_PARTY_MAP_MARKER_UPDATE, actor_id, data)
}

// ---------------------------------------------------------------------------
// 0x018B SetGroupLayoutID — per-member party-list row update.
//
// Wire format (per wiki + retail bytes from
// `ffxiv_traces/combat_skills.pcapng` record #1 of opcode 0x018B):
//
//   body size = 0x38 (56 bytes) — fixed
//   0x00  u64 group_id              — same playerGroupID space as 0x018D;
//                                     captured as 0x80000000_0077E9AC
//                                     for the solo player
//   0x08  u32 actor_id              — actor being updated
//   0x0C  u32 display_name_id       — 0xFFFFFFFF for player characters
//                                     (and presumably custom names);
//                                     real id for game-data NPCs
//   0x10  u32 layout_id             — wiki: "Map layout the actorID is in"
//                                     (mapobj layoutId space, per Meteor's
//                                     `SpawnLocation.mapObjLayoutId`).
//                                     Retail captured value: 0x131 (305)
//                                     for the solo player; default 0 is
//                                     safe for a player not bound to a
//                                     mapobj.
//   0x14  u8  unknown1              — wiki: "Sometimes 0, sometimes 1.
//                                     Online state? Verify". Retail
//                                     captured 0; pass 1 for an online
//                                     party member.
//   0x15  u8  unknown2              — wiki: "Always 1?". Retail confirms 1.
//   0x16  string actor_name (34 B)  — null-padded ASCII; client uses this
//                                     for the party-list row when the
//                                     actor isn't in render range
//
// Wiki note: "Has something to do with keeping party members updated when
// they're not in range of the player. Possibly related to map markers?" —
// confirmed by the captured groupID matching 0x018D's solo group id, so
// 0x018B is the row-level companion to 0x018D's whole-party overlay.

/// 0x018B SetGroupLayoutID — emit one packet per party member.
pub fn build_set_group_layout_id(
    actor_id: u32,
    group_id: u64,
    member_actor_id: u32,
    display_name_id: u32,
    layout_id: u32,
    unknown1: u8,
    actor_name: &str,
) -> SubPacket {
    let mut data = body(0x58);
    {
        let mut c = Cursor::new(&mut data[..]);
        c.write_u64::<LittleEndian>(group_id).unwrap();
        c.write_u32::<LittleEndian>(member_actor_id).unwrap();
        c.write_u32::<LittleEndian>(display_name_id).unwrap();
        c.write_u32::<LittleEndian>(layout_id).unwrap();
        c.write_u8(unknown1).unwrap();
        c.write_u8(1).unwrap(); // unknown2 — wiki says always 1, captured retail confirms
    }
    // Name string lives at body offset 0x16; null-padded to 34 bytes
    // (the rest of the body).
    {
        let mut c = Cursor::new(&mut data[0x16..0x38]);
        write_padded_ascii(&mut c, actor_name, 34);
    }
    SubPacket::new(OP_SET_GROUP_LAYOUT_ID, actor_id, data)
}

/// `displayNameID` retail uses for player characters / custom-named
/// actors. Real (game-data) NPCs use their populace name id instead.
pub const SET_GROUP_LAYOUT_ID_PLAYER_DISPLAY_NAME: u32 = 0xFFFF_FFFF;

// ---------------------------------------------------------------------------
// 0x0187 SetOccupancyGroup — claim/unclaim mob group ownership.
//
// Wire format (per wiki + retail bytes from
// `ffxiv_traces/combat_skills.pcapng` 0x0187 records #1 and #2):
//
//   body size = 0x40 (64 bytes) — fixed
//   0x00  u64 monster_group_id      — the mob group being (un)claimed
//   0x08  u32 group_type            — 10002 (0x2712) = MonsterPartyGroup
//                                     for field mobs; 30012 = Simple
//                                     ContentGroup (e.g. Ifrit ad clones)
//   0x0C  u32 zero/padding
//   0x10  u64 player_group_id       — claiming player party group
//                                     (0 = clear claim; same group-id
//                                     space as 0x018D / 0x018B)
//   0x18  u32 unknown               — wiki: "always 0xFFFFFFFF"
//                                     (confirmed in both captures)
//   0x1C  36 bytes zero/padding
//
// Captured behaviour: same monsterGroup emitted twice — first with
// playerGroup=0 to clear any prior claim, then with the player's
// solo group id to register fresh claim. Wiki note also says
// `hateType` modifier on the work struct depends on whether a group
// is set as occupied at the time of `hateType` being called, and is
// not retroactive — so this packet is load-bearing for nameplate
// label colour even after a single `0x00DB SetActorTarget` happens.
//
// Project Meteor never implements this; their `MonsterParty` /
// `HateContainer` plumbing exists but the claim broadcast does not.

/// `groupType` for monster parties (field mobs). Same value as
/// `GroupTypeId::MONSTER_PARTY` in the higher-level group system.
pub const SET_OCCUPANCY_GROUP_TYPE_MONSTER_PARTY: u32 = 10002;

/// `groupType` for simple content groups (e.g. Ifrit's clone-ad
/// groups in 1.x).
pub const SET_OCCUPANCY_GROUP_TYPE_SIMPLE_CONTENT: u32 = 30012;

/// `unknown` field — wiki and retail captures agree this is always
/// `0xFFFFFFFF`.
pub const SET_OCCUPANCY_GROUP_UNKNOWN_CONST: u32 = 0xFFFF_FFFF;

/// 0x0187 SetOccupancyGroup. Pass `player_group_id = 0` to clear an
/// existing claim; otherwise pass the player's party / solo group
/// id (same id space as 0x018D PartyMapMarker).
pub fn build_set_occupancy_group(
    actor_id: u32,
    monster_group_id: u64,
    group_type: u32,
    player_group_id: u64,
) -> SubPacket {
    let mut data = body(0x60);
    let mut c = Cursor::new(&mut data[..]);
    c.write_u64::<LittleEndian>(monster_group_id).unwrap();
    c.write_u32::<LittleEndian>(group_type).unwrap();
    c.write_u32::<LittleEndian>(0).unwrap(); // pad at 0x0C
    c.write_u64::<LittleEndian>(player_group_id).unwrap();
    c.write_u32::<LittleEndian>(SET_OCCUPANCY_GROUP_UNKNOWN_CONST)
        .unwrap();
    // Remaining 36 bytes already zero from `body()`.
    SubPacket::new(OP_SET_OCCUPANCY_GROUP, actor_id, data)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reproduce the body bytes captured from
    /// `ffxiv_traces/chat_say.pcapng` record #1 of opcode 0x018D — solo
    /// player at `(1822.97, 149.47, 1728.025)`, orientation -2.354 rad,
    /// actor id 0x029B2941, with the captured solo group id and the
    /// per-marker `unknown` field 0x00C17909.
    #[test]
    fn party_map_marker_matches_retail_capture() {
        let marker = PartyMapMarker {
            player_id: 0x029B_2941,
            unknown: 0x00C1_7909,
            x: f32::from_le_bytes([0x80, 0xF3, 0xE3, 0x44]), // 1822.9688
            y: f32::from_le_bytes([0xFA, 0x78, 0x15, 0x43]), // 149.47256
            z: f32::from_le_bytes([0xCB, 0x00, 0xD8, 0x44]), // 1728.0247
            orientation: f32::from_le_bytes([0x17, 0xA8, 0x16, 0xC0]), // -2.3540878
        };
        let pkt = build_party_map_marker_update(
            0x029B_2941,
            PARTY_MAP_MARKER_SOLO_GROUP_ID,
            PARTY_MAP_MARKER_GROUP_TYPE_PLAYER_PARTY,
            &[marker],
        );
        let body = &pkt.data;
        assert_eq!(body.len(), 0x298);

        // Header: playerGroupID + groupType + pad.
        assert_eq!(
            &body[0x00..0x10],
            &[
                0xAC, 0xE9, 0x77, 0x00, 0x00, 0x00, 0x00, 0x80, 0x11, 0x27, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00
            ],
        );

        // Marker 0 — full 40-byte slot.
        assert_eq!(
            &body[0x10..0x38],
            &[
                0x41, 0x29, 0x9B, 0x02, 0x00, 0x00, 0x00, 0x00, // playerID + pad
                0x09, 0x79, 0xC1, 0x00, 0x00, 0x00, 0x00, 0x00, // unknown + pad
                0x00, 0x00, 0x00, 0x00, 0x80, 0xF3, 0xE3, 0x44, // pad + X
                0xFA, 0x78, 0x15, 0x43, 0xCB, 0x00, 0xD8, 0x44, // Y + Z
                0x17, 0xA8, 0x16, 0xC0, 0x00, 0x00, 0x00, 0x00, // O + trailing pad
            ],
        );

        // Empty marker slots (15 of them, 600 bytes) must be all zero.
        assert!(body[0x38..0x290].iter().all(|b| *b == 0));

        // numEntries u32 then 4 bytes trailing pad.
        assert_eq!(&body[0x290..0x298], &[0x01, 0, 0, 0, 0, 0, 0, 0]);
    }

    /// A 3-member 10001 party trio (player leader + two NPC allies) must
    /// round-trip with count=3 and the NPC rows carrying
    /// `localizedName = displayNameId` — the encoding the client's
    /// PartyParameterWidget resolves NPC row labels from (see
    /// `GroupMember::row_for_actor`). (Garlemald-Server #46, round 4.)
    #[test]
    fn party_x08_npc_rows_carry_display_name_id() {
        const NPC_YSHTOLA_ID: u32 = 0x4534_0006;
        const NPC_STHALMANN_ID: u32 = 0x4534_0007;
        const YSHTOLA_DISPLAY_NAME_ID: u32 = 3_020_045;
        const STHALMANN_DISPLAY_NAME_ID: u32 = 3_020_046;
        let members = vec![
            // Player: non-empty display name → custom-name row.
            GroupMember::row_for_actor(0x0000_02AE, "Aert Zeverith", 0xFFFF_FFFF),
            // NPC allies: empty display name → display-name-id row.
            GroupMember::row_for_actor(NPC_YSHTOLA_ID, "", YSHTOLA_DISPLAY_NAME_ID),
            GroupMember::row_for_actor(NPC_STHALMANN_ID, "", STHALMANN_DISPLAY_NAME_ID),
        ];
        let mut offset = 0usize;
        let pkt = build_group_members_x08(0x0000_02AE, 0xA6, 0x01, &members, &mut offset);
        assert_eq!(offset, 3);
        // Count at body 0x10 + 0x30*8 = 0x190.
        assert_eq!(&pkt.data[0x190..0x194], &[3, 0, 0, 0]);
        // Slot 0 (player, body 0x10): localized_name = -1, name populated.
        assert_eq!(&pkt.data[0x14..0x18], &(-1i32).to_le_bytes());
        assert_eq!(&pkt.data[0x1E..0x22], b"Aert");
        // Slot 1 (NPC, body 0x40): actor id + localized_name =
        // display_name_id, name field all-zero.
        assert_eq!(&pkt.data[0x40..0x44], &NPC_YSHTOLA_ID.to_le_bytes());
        assert_eq!(
            &pkt.data[0x44..0x48],
            &(YSHTOLA_DISPLAY_NAME_ID as i32).to_le_bytes()
        );
        assert!(pkt.data[0x4E..0x6E].iter().all(|b| *b == 0));
        // Slot 2 (NPC, body 0x70): same shape.
        assert_eq!(&pkt.data[0x70..0x74], &NPC_STHALMANN_ID.to_le_bytes());
        assert_eq!(
            &pkt.data[0x74..0x78],
            &(STHALMANN_DISPLAY_NAME_ID as i32).to_le_bytes()
        );
    }

    #[test]
    fn content_members_x08_uses_0xc_stride_not_party_layout() {
        // 0x0183 ContentMembersX08 must use pmeteor's 12-byte stride
        // (actorId, 1001, 1) with the count at body 0x70 — NOT the 0x30 party
        // stride. Reusing the party builder made the 1.x client misread the
        // content roster so Yda/Papalymo never appeared. (Garlemald #28.)
        let members = vec![
            GroupMember {
                actor_id: 0x0000_0001,
                ..Default::default()
            },
            GroupMember {
                actor_id: 0x4534_0006,
                ..Default::default()
            },
            GroupMember {
                actor_id: 0x4534_0007,
                ..Default::default()
            },
        ];
        let mut offset = 0usize;
        let pkt = build_content_members_x08(0x0000_0001, 0xA6, 0x01, &members, &mut offset);
        assert_eq!(pkt.data.len(), 0x1B8 - 0x20);
        // Member 0 at 0x10: actorId=1, layout=1001, 1.
        assert_eq!(
            &pkt.data[0x10..0x1C],
            &[1, 0, 0, 0, 0xE9, 3, 0, 0, 1, 0, 0, 0]
        );
        // Member 1 at 0x1C (0x10 + 0xC): actorId=0x45340006.
        assert_eq!(
            &pkt.data[0x1C..0x28],
            &[0x06, 0x00, 0x34, 0x45, 0xE9, 3, 0, 0, 1, 0, 0, 0]
        );
        // Member 2 at 0x28: actorId=0x45340007.
        assert_eq!(&pkt.data[0x28..0x2C], &[0x07, 0x00, 0x34, 0x45]);
        // Count at body 0x70.
        assert_eq!(&pkt.data[0x70..0x74], &[3, 0, 0, 0]);
        assert_eq!(offset, 3);
    }

    #[test]
    fn party_map_marker_truncates_above_16() {
        let m = PartyMapMarker {
            player_id: 1,
            ..Default::default()
        };
        let pkt = build_party_map_marker_update(0x029B_2941, 0, 10001, &vec![m; 20]);
        // numEntries clamps to 16 even when the caller hands in more.
        let n = u32::from_le_bytes(pkt.data[0x290..0x294].try_into().unwrap());
        assert_eq!(n, 16);
    }

    #[test]
    fn party_map_marker_empty_yields_zero_count() {
        let pkt = build_party_map_marker_update(0x029B_2941, 0, 10001, &[]);
        assert_eq!(pkt.data.len(), 0x298);
        let n = u32::from_le_bytes(pkt.data[0x290..0x294].try_into().unwrap());
        assert_eq!(n, 0);
        // Marker block stays all zero.
        assert!(pkt.data[0x10..0x290].iter().all(|b| *b == 0));
    }

    /// Reproduce the body bytes captured from
    /// `ffxiv_traces/combat_skills.pcapng` record #1 of opcode 0x018B —
    /// solo player "Wrenix Wrong" at actor id 0x029B2941 with layoutID
    /// 0x131 and the same solo group id used by 0x018D.
    #[test]
    fn set_group_layout_id_matches_retail_capture() {
        let pkt = build_set_group_layout_id(
            0x029B_2941,
            PARTY_MAP_MARKER_SOLO_GROUP_ID,
            0x029B_2941,
            SET_GROUP_LAYOUT_ID_PLAYER_DISPLAY_NAME,
            0x0131,
            0,
            "Wrenix Wrong",
        );
        let body = &pkt.data;
        assert_eq!(body.len(), 0x38);
        #[rustfmt::skip]
        let expected: [u8; 0x38] = [
            0xAC, 0xE9, 0x77, 0x00, 0x00, 0x00, 0x00, 0x80, // groupID
            0x41, 0x29, 0x9B, 0x02,                         // actorID
            0xFF, 0xFF, 0xFF, 0xFF,                         // displayNameID
            0x31, 0x01, 0x00, 0x00,                         // layoutID = 0x131
            0x00, 0x01,                                     // unknown1, unknown2
            // actorName "Wrenix Wrong" (12 bytes) + 22 bytes of zero pad
            b'W', b'r', b'e', b'n', b'i', b'x', b' ',
            b'W', b'r', b'o', b'n', b'g',
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];
        assert_eq!(body, &expected[..]);
    }

    /// Reproduce the two 0x0187 packets captured from
    /// `ffxiv_traces/combat_skills.pcapng` — same monster group, first
    /// emitted with player_group=0 to clear claim, then with the solo
    /// player group to register fresh claim.
    #[test]
    fn set_occupancy_group_lifecycle_matches_retail_capture() {
        let monster_group: u64 = 0x2680_0000_0000_25FB;

        // #1: clear claim (player_group=0).
        let p1 = build_set_occupancy_group(
            0x029B_2941,
            monster_group,
            SET_OCCUPANCY_GROUP_TYPE_MONSTER_PARTY,
            0,
        );
        assert_eq!(p1.data.len(), 0x40);
        let mut expected1 = [0u8; 0x40];
        expected1[..8].copy_from_slice(&monster_group.to_le_bytes());
        expected1[8..12].copy_from_slice(&10002u32.to_le_bytes());
        // 0x0C..0x10 zero pad
        // 0x10..0x18 player_group_id zero
        expected1[0x18..0x1C].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        assert_eq!(p1.data, expected1);
        // Spot-check against the literal captured bytes.
        assert_eq!(
            &p1.data[..0x20],
            &[
                0xFB, 0x25, 0x00, 0x00, 0x00, 0x00, 0x80, 0x26, 0x12, 0x27, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF,
                0x00, 0x00, 0x00, 0x00,
            ],
        );

        // #2: claim by solo player group (same id space as 0x018D).
        let p2 = build_set_occupancy_group(
            0x029B_2941,
            monster_group,
            SET_OCCUPANCY_GROUP_TYPE_MONSTER_PARTY,
            PARTY_MAP_MARKER_SOLO_GROUP_ID,
        );
        assert_eq!(
            &p2.data[..0x20],
            &[
                0xFB, 0x25, 0x00, 0x00, 0x00, 0x00, 0x80, 0x26, 0x12, 0x27, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0xAC, 0xE9, 0x77, 0x00, 0x00, 0x00, 0x00, 0x80, 0xFF, 0xFF, 0xFF, 0xFF,
                0x00, 0x00, 0x00, 0x00,
            ],
        );
    }

    #[test]
    fn set_occupancy_group_simple_content_type() {
        let pkt = build_set_occupancy_group(
            1,
            0xFEDC_BA98_7654_3210,
            SET_OCCUPANCY_GROUP_TYPE_SIMPLE_CONTENT,
            0,
        );
        let group_type = u32::from_le_bytes(pkt.data[8..12].try_into().unwrap());
        assert_eq!(group_type, 30012);
    }

    #[test]
    fn set_group_layout_id_truncates_long_names() {
        // 40-char name; the 34-byte slot truncates to the first 34
        // characters and drops the trailing "BBBBBB".
        let long = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABBBBBB";
        let pkt = build_set_group_layout_id(
            0x029B_2941,
            0,
            0x029B_2941,
            SET_GROUP_LAYOUT_ID_PLAYER_DISPLAY_NAME,
            0,
            1,
            long,
        );
        let name_bytes = &pkt.data[0x16..0x38];
        assert_eq!(name_bytes.len(), 34);
        assert!(name_bytes.iter().all(|b| *b == b'A'));
    }
}
