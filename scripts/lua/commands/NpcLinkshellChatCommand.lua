require ("global")

--[[

NpcLinkshellChatCommand Script

NOTE (Garlemald-Server #46): the NPC-linkshell chat is handled
ENGINE-SIDE, not by this script. The map-server routes the client's
EventStart against the NpcLinkshellChatCommand static actor
(0xA0F05E95) directly through `PacketProcessor::handle_npc_ls_chat`
(a port of pmeteor `Player.HandleNpcLs`): it finds the active quest
whose `npcLsFrom` matches the clicked linkshell id and fires that
quest's `onNpcLS(player, quest, from, msgStep)` hook. So this script
is intentionally NOT mapped in `command_script_name` and is never
invoked. It is kept only as documentation of the command's identity.

The prior on-disk copy was a stale Project Meteor revision with a
hand-maintained `npcLsHandlers` table and a different `onEventStarted`
arity — superseded by the quest-driven onNpcLS dispatch.

--]]

function onEventStarted(player, command, eventType, eventName, npcLsId)
	-- Defensive no-op: the engine never routes here. If a future change
	-- DID map this command to the script path, close the event so the
	-- client doesn't soft-lock in the linkshell window.
	player:EndEvent();
end
