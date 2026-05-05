package com.bong.client.network;

import com.bong.client.combat.UnifiedEvent;
import com.bong.client.combat.UnifiedEventStore;
import com.bong.client.social.NicheIntrusionAlertHandler;
import com.bong.client.social.SocialStateStore;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

import java.util.ArrayList;
import java.util.List;
import java.util.Set;
import java.util.regex.Pattern;

public final class SocialServerDataHandler implements ServerDataHandler {
    private static final Pattern INTEGER_TOKEN_PATTERN = Pattern.compile("-?(0|[1-9]\\d*)");
    private static final Set<String> EXPOSURE_KINDS = Set.of("chat", "trade", "divine", "death");
    private static final int SOCIAL_COLOR = 0xFFA0C0FF;
    private static final int WARNING_COLOR = 0xFFFFAA55;

    @Override
    public ServerDataDispatch handle(ServerDataEnvelope envelope) {
        return switch (envelope.type()) {
            case "social_anonymity" -> handleAnonymity(envelope);
            case "social_exposure" -> handleExposure(envelope);
            case "social_pact" -> handlePact(envelope);
            case "social_feud" -> handleFeud(envelope);
            case "social_renown_delta" -> handleRenownDelta(envelope);
            case "niche_intrusion" -> handleNicheIntrusion(envelope);
            case "niche_guardian_fatigue" -> handleNicheGuardianFatigue(envelope);
            case "niche_guardian_broken" -> handleNicheGuardianBroken(envelope);
            case "sparring_invite" -> handleSparringInvite(envelope);
            case "trade_offer" -> handleTradeOffer(envelope);
            default -> ServerDataDispatch.noOp(envelope.type(), "Ignoring unsupported social payload type");
        };
    }

    private ServerDataDispatch handleAnonymity(ServerDataEnvelope envelope) {
        JsonObject p = envelope.payload();
        String viewer = readString(p, "viewer");
        JsonArray remotesArray = readArray(p, "remotes");
        if (viewer == null || remotesArray == null) {
            return ServerDataDispatch.noOp(envelope.type(), "Ignoring social_anonymity: viewer/remotes missing");
        }

        List<SocialStateStore.SocialRemoteIdentity> remotes = new ArrayList<>();
        for (JsonElement remoteElement : remotesArray) {
            SocialStateStore.SocialRemoteIdentity remote = parseRemote(remoteElement);
            if (remote == null) {
                return ServerDataDispatch.noOp(envelope.type(), "Ignoring social_anonymity: malformed remote entry");
            }
            remotes.add(remote);
        }

        SocialStateStore.replaceAnonymity(viewer, remotes);
        return ServerDataDispatch.handled(envelope.type(), "Applied social_anonymity (" + remotes.size() + " remotes)");
    }

    private ServerDataDispatch handleExposure(ServerDataEnvelope envelope) {
        JsonObject p = envelope.payload();
        String actor = readString(p, "actor");
        String kind = readString(p, "kind");
        JsonArray witnessesArray = readArray(p, "witnesses");
        Long tick = readLong(p, "tick");
        if (actor == null || kind == null || !EXPOSURE_KINDS.contains(kind) || witnessesArray == null || tick == null) {
            return ServerDataDispatch.noOp(envelope.type(), "Ignoring social_exposure: invalid actor/kind/witnesses/tick");
        }

        List<String> witnesses = readStringArray(witnessesArray);
        SocialStateStore.SocialExposure exposure = new SocialStateStore.SocialExposure(
            actor,
            kind,
            witnesses,
            tick,
            readString(p, "zone")
        );
        SocialStateStore.recordExposure(exposure);
        publishSocialEvent(
            UnifiedEvent.Priority.P1_IMPORTANT,
            "social_exposure:" + actor,
            "身份暴露：" + actor + " → " + witnesses.size() + "人",
            WARNING_COLOR
        );
        return ServerDataDispatch.handled(envelope.type(), "Recorded social_exposure for " + actor);
    }

    private ServerDataDispatch handlePact(ServerDataEnvelope envelope) {
        JsonObject p = envelope.payload();
        String left = readString(p, "left");
        String right = readString(p, "right");
        String terms = readString(p, "terms");
        Long tick = readLong(p, "tick");
        Boolean broken = readBoolean(p, "broken");
        if (left == null || right == null || terms == null || tick == null || broken == null) {
            return ServerDataDispatch.noOp(envelope.type(), "Ignoring social_pact: missing left/right/terms/tick/broken");
        }

        SocialStateStore.SocialRelationshipSignal signal = new SocialStateStore.SocialRelationshipSignal(
            "pact",
            left,
            right,
            terms,
            broken,
            tick,
            ""
        );
        SocialStateStore.recordRelationship(signal);
        publishSocialEvent(
            broken ? UnifiedEvent.Priority.P1_IMPORTANT : UnifiedEvent.Priority.P2_NORMAL,
            "social_pact:" + left + ":" + right,
            (broken ? "盟约解除：" : "盟约建立：") + left + " / " + right,
            broken ? WARNING_COLOR : SOCIAL_COLOR
        );
        return ServerDataDispatch.handled(envelope.type(), "Recorded social_pact between " + left + " and " + right);
    }

    private ServerDataDispatch handleFeud(ServerDataEnvelope envelope) {
        JsonObject p = envelope.payload();
        String left = readString(p, "left");
        String right = readString(p, "right");
        Long tick = readLong(p, "tick");
        if (left == null || right == null || tick == null) {
            return ServerDataDispatch.noOp(envelope.type(), "Ignoring social_feud: missing left/right/tick");
        }

        String place = readString(p, "place");
        SocialStateStore.SocialRelationshipSignal signal = new SocialStateStore.SocialRelationshipSignal(
            "feud",
            left,
            right,
            "",
            false,
            tick,
            place
        );
        SocialStateStore.recordRelationship(signal);
        publishSocialEvent(
            UnifiedEvent.Priority.P1_IMPORTANT,
            "social_feud:" + left + ":" + right,
            "死仇已立：" + left + " / " + right + (place == null ? "" : " @ " + place),
            WARNING_COLOR
        );
        return ServerDataDispatch.handled(envelope.type(), "Recorded social_feud between " + left + " and " + right);
    }

    private ServerDataDispatch handleRenownDelta(ServerDataEnvelope envelope) {
        JsonObject p = envelope.payload();
        String charId = readString(p, "char_id");
        Integer fameDelta = readInt(p, "fame_delta");
        Integer notorietyDelta = readInt(p, "notoriety_delta");
        JsonArray tagsArray = readArray(p, "tags_added");
        Long tick = readLong(p, "tick");
        String reason = readString(p, "reason");
        if (charId == null || fameDelta == null || notorietyDelta == null || tagsArray == null || tick == null || reason == null) {
            return ServerDataDispatch.noOp(envelope.type(), "Ignoring social_renown_delta: missing required fields");
        }

        List<SocialStateStore.RenownTag> tags = parseRenownTags(tagsArray);
        SocialStateStore.SocialRenownDelta delta = new SocialStateStore.SocialRenownDelta(
            charId,
            fameDelta,
            notorietyDelta,
            tags,
            tick,
            reason
        );
        SocialStateStore.recordRenownDelta(delta);
        if (fameDelta != 0 || notorietyDelta != 0 || !tags.isEmpty()) {
            publishSocialEvent(
                UnifiedEvent.Priority.P2_NORMAL,
                "social_renown:" + charId,
                "声名变动：" + charId + " fame " + signed(fameDelta) + " / notoriety " + signed(notorietyDelta),
                notorietyDelta > fameDelta ? WARNING_COLOR : SOCIAL_COLOR
            );
        }
        return ServerDataDispatch.handled(envelope.type(), "Recorded social_renown_delta for " + charId);
    }

    private ServerDataDispatch handleSparringInvite(ServerDataEnvelope envelope) {
        JsonObject p = envelope.payload();
        String inviteId = readString(p, "invite_id");
        String initiator = readString(p, "initiator");
        String target = readString(p, "target");
        Long expiresAtMs = readLong(p, "expires_at_ms");
        if (inviteId == null || initiator == null || target == null || expiresAtMs == null) {
            return ServerDataDispatch.noOp(envelope.type(), "Ignoring sparring_invite: missing invite_id/initiator/target/expires_at_ms");
        }

        SocialStateStore.SparringInvite invite = new SocialStateStore.SparringInvite(
            inviteId,
            initiator,
            target,
            readString(p, "realm_band"),
            readString(p, "breath_hint"),
            readString(p, "terms"),
            expiresAtMs
        );
        SocialStateStore.replaceSparringInvite(invite);
        publishSocialEvent(
            UnifiedEvent.Priority.P1_IMPORTANT,
            "sparring_invite:" + inviteId,
            "切磋邀请：" + initiator + " → " + target,
            SOCIAL_COLOR
        );
        return ServerDataDispatch.handled(envelope.type(), "Recorded sparring_invite " + inviteId);
    }

    private ServerDataDispatch handleNicheIntrusion(ServerDataEnvelope envelope) {
        JsonObject p = envelope.payload();
        String intruderId = readString(p, "intruder_id");
        JsonArray itemsTakenArray = readArray(p, "items_taken");
        Double taintDelta = readDouble(p, "taint_delta");
        if (intruderId == null || itemsTakenArray == null || taintDelta == null) {
            return ServerDataDispatch.noOp(envelope.type(), "Ignoring niche_intrusion: missing intruder/items/taint");
        }
        NicheIntrusionAlertHandler.recordIntrusion(intruderId, readLongArray(itemsTakenArray), taintDelta);
        return ServerDataDispatch.handled(envelope.type(), "Recorded niche_intrusion for " + intruderId);
    }

    private ServerDataDispatch handleNicheGuardianFatigue(ServerDataEnvelope envelope) {
        JsonObject p = envelope.payload();
        String guardianKind = readString(p, "guardian_kind");
        Integer chargesRemaining = readInt(p, "charges_remaining");
        if (guardianKind == null || chargesRemaining == null) {
            return ServerDataDispatch.noOp(envelope.type(), "Ignoring niche_guardian_fatigue: missing guardian_kind/charges");
        }
        NicheIntrusionAlertHandler.recordGuardianFatigue(guardianKind, chargesRemaining);
        return ServerDataDispatch.handled(envelope.type(), "Recorded niche_guardian_fatigue " + guardianKind);
    }

    private ServerDataDispatch handleNicheGuardianBroken(ServerDataEnvelope envelope) {
        JsonObject p = envelope.payload();
        String guardianKind = readString(p, "guardian_kind");
        String intruderId = readString(p, "intruder_id");
        if (guardianKind == null || intruderId == null) {
            return ServerDataDispatch.noOp(envelope.type(), "Ignoring niche_guardian_broken: missing guardian_kind/intruder");
        }
        NicheIntrusionAlertHandler.recordGuardianBroken(guardianKind, intruderId);
        return ServerDataDispatch.handled(envelope.type(), "Recorded niche_guardian_broken " + guardianKind);
    }

    private ServerDataDispatch handleTradeOffer(ServerDataEnvelope envelope) {
        JsonObject p = envelope.payload();
        String offerId = readString(p, "offer_id");
        String initiator = readString(p, "initiator");
        String target = readString(p, "target");
        Long expiresAtMs = readLong(p, "expires_at_ms");
        SocialStateStore.TradeItemSummary offeredItem = parseTradeItem(p.get("offered_item"));
        JsonArray requestedArray = readArray(p, "requested_items");
        if (offerId == null || initiator == null || target == null || expiresAtMs == null || offeredItem == null || requestedArray == null) {
            return ServerDataDispatch.noOp(envelope.type(), "Ignoring trade_offer: missing offer_id/initiator/target/offered_item/requested_items/expires_at_ms");
        }

        List<SocialStateStore.TradeItemSummary> requestedItems = new ArrayList<>();
        for (JsonElement element : requestedArray) {
            SocialStateStore.TradeItemSummary item = parseTradeItem(element);
            if (item != null) requestedItems.add(item);
        }
        SocialStateStore.TradeOffer offer = new SocialStateStore.TradeOffer(
            offerId,
            initiator,
            target,
            offeredItem,
            requestedItems,
            expiresAtMs
        );
        SocialStateStore.replaceTradeOffer(offer);
        publishSocialEvent(
            UnifiedEvent.Priority.P1_IMPORTANT,
            "trade_offer:" + offerId,
            "交易邀请：" + initiator + " 提供 " + offeredItem.displayName(),
            SOCIAL_COLOR
        );
        return ServerDataDispatch.handled(envelope.type(), "Recorded trade_offer " + offerId);
    }

    private static SocialStateStore.SocialRemoteIdentity parseRemote(JsonElement element) {
        if (element == null || element.isJsonNull() || !element.isJsonObject()) return null;
        JsonObject remote = element.getAsJsonObject();
        String playerUuid = readString(remote, "player_uuid");
        Boolean anonymous = readBoolean(remote, "anonymous");
        if (playerUuid == null || anonymous == null) return null;
        JsonArray tagsArray = readArray(remote, "renown_tags");
        return new SocialStateStore.SocialRemoteIdentity(
            playerUuid,
            anonymous,
            readString(remote, "display_name"),
            readString(remote, "realm_band"),
            readString(remote, "breath_hint"),
            tagsArray == null ? List.of() : readStringArray(tagsArray)
        );
    }

    private static List<SocialStateStore.RenownTag> parseRenownTags(JsonArray array) {
        ArrayList<SocialStateStore.RenownTag> tags = new ArrayList<>();
        for (JsonElement element : array) {
            if (element == null || element.isJsonNull() || !element.isJsonObject()) continue;
            JsonObject tagObject = element.getAsJsonObject();
            String tag = readString(tagObject, "tag");
            Double weight = readDouble(tagObject, "weight");
            Long lastSeenTick = readLong(tagObject, "last_seen_tick");
            Boolean permanent = readBoolean(tagObject, "permanent");
            if (tag == null || weight == null || lastSeenTick == null || permanent == null) continue;
            tags.add(new SocialStateStore.RenownTag(tag, weight, lastSeenTick, permanent));
        }
        return tags;
    }

    private static SocialStateStore.TradeItemSummary parseTradeItem(JsonElement element) {
        if (element == null || element.isJsonNull() || !element.isJsonObject()) return null;
        JsonObject object = element.getAsJsonObject();
        Long instanceId = readLong(object, "instance_id");
        String itemId = readString(object, "item_id");
        String displayName = readString(object, "display_name");
        Integer stackCount = readInt(object, "stack_count");
        if (instanceId == null || itemId == null || displayName == null || stackCount == null) return null;
        return new SocialStateStore.TradeItemSummary(instanceId, itemId, displayName, stackCount);
    }

    private static List<String> readStringArray(JsonArray array) {
        ArrayList<String> values = new ArrayList<>();
        for (JsonElement element : array) {
            if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) continue;
            JsonPrimitive primitive = element.getAsJsonPrimitive();
            if (!primitive.isString()) continue;
            String value = primitive.getAsString();
            if (value != null && !value.isBlank()) values.add(value);
        }
        return values;
    }

    private static List<Long> readLongArray(JsonArray array) {
        ArrayList<Long> values = new ArrayList<>();
        for (JsonElement element : array) {
            if (element == null || element.isJsonNull() || !element.isJsonPrimitive()) continue;
            JsonPrimitive primitive = element.getAsJsonPrimitive();
            if (!primitive.isNumber()) continue;
            long value = primitive.getAsLong();
            if (value >= 0L) values.add(value);
        }
        return values;
    }

    private static void publishSocialEvent(UnifiedEvent.Priority priority, String sourceTag, String text, int color) {
        UnifiedEventStore.stream().publish(
            UnifiedEvent.Channel.SOCIAL,
            priority,
            sourceTag,
            text,
            color,
            System.currentTimeMillis()
        );
    }

    private static String signed(int value) {
        return value > 0 ? "+" + value : Integer.toString(value);
    }

    private static JsonArray readArray(JsonObject object, String fieldName) {
        JsonElement element = object == null ? null : object.get(fieldName);
        return element != null && !element.isJsonNull() && element.isJsonArray() ? element.getAsJsonArray() : null;
    }

    private static JsonPrimitive readPrimitive(JsonObject object, String fieldName) {
        JsonElement element = object == null ? null : object.get(fieldName);
        return element != null && !element.isJsonNull() && element.isJsonPrimitive() ? element.getAsJsonPrimitive() : null;
    }

    private static String readString(JsonObject object, String fieldName) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        if (primitive == null || !primitive.isString()) return null;
        String value = primitive.getAsString();
        return value == null || value.isBlank() ? null : value.trim();
    }

    private static Boolean readBoolean(JsonObject object, String fieldName) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        return primitive != null && primitive.isBoolean() ? primitive.getAsBoolean() : null;
    }

    private static Double readDouble(JsonObject object, String fieldName) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        if (primitive == null || !primitive.isNumber()) return null;
        double value = primitive.getAsDouble();
        return Double.isFinite(value) ? value : null;
    }

    private static Integer readInt(JsonObject object, String fieldName) {
        Long value = readLong(object, fieldName);
        if (value == null || value < Integer.MIN_VALUE || value > Integer.MAX_VALUE) return null;
        return value.intValue();
    }

    private static Long readLong(JsonObject object, String fieldName) {
        JsonPrimitive primitive = readPrimitive(object, fieldName);
        if (primitive == null || !primitive.isNumber()) return null;
        String raw = primitive.getAsString();
        if (!INTEGER_TOKEN_PATTERN.matcher(raw).matches()) return null;
        try {
            return Long.parseLong(raw);
        } catch (NumberFormatException ignored) {
            return null;
        }
    }
}
