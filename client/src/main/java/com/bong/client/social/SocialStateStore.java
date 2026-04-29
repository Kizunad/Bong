package com.bong.client.social;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/** Client-side mirror for plan-social-v1 server_data payloads. */
public final class SocialStateStore {
    private static final int MAX_EVENTS = 32;

    private static volatile SocialAnonymitySnapshot anonymity = SocialAnonymitySnapshot.empty();
    private static volatile List<SocialExposure> exposures = List.of();
    private static volatile List<SocialRelationshipSignal> relationships = List.of();
    private static volatile List<SocialRenownDelta> renownDeltas = List.of();
    private static volatile SparringInvite sparringInvite = null;

    private SocialStateStore() {
    }

    public static SocialAnonymitySnapshot anonymity() {
        return anonymity;
    }

    public static List<SocialExposure> exposures() {
        return exposures;
    }

    public static List<SocialRelationshipSignal> relationships() {
        return relationships;
    }

    public static List<SocialRenownDelta> renownDeltas() {
        return renownDeltas;
    }

    public static SparringInvite sparringInvite() {
        return sparringInvite;
    }

    public static void replaceAnonymity(String viewer, List<SocialRemoteIdentity> remotes) {
        LinkedHashMap<String, SocialRemoteIdentity> byUuid = new LinkedHashMap<>();
        for (SocialRemoteIdentity remote : safeList(remotes)) {
            if (remote.playerUuid().isBlank()) continue;
            byUuid.put(remote.playerUuid(), remote);
        }
        anonymity = new SocialAnonymitySnapshot(viewer, byUuid);
    }

    public static synchronized void recordExposure(SocialExposure exposure) {
        exposures = appendBounded(exposures, exposure);
    }

    public static synchronized void recordRelationship(SocialRelationshipSignal relationship) {
        relationships = appendBounded(relationships, relationship);
    }

    public static synchronized void recordRenownDelta(SocialRenownDelta delta) {
        renownDeltas = appendBounded(renownDeltas, delta);
    }

    public static void replaceSparringInvite(SparringInvite invite) {
        sparringInvite = invite;
    }

    public static void clearOnDisconnect() {
        anonymity = SocialAnonymitySnapshot.empty();
        exposures = List.of();
        relationships = List.of();
        renownDeltas = List.of();
        sparringInvite = null;
    }

    public static void resetForTests() {
        clearOnDisconnect();
    }

    private static <T> List<T> appendBounded(List<T> previous, T entry) {
        if (entry == null) return previous;
        ArrayList<T> next = new ArrayList<>(previous.size() + 1);
        next.add(entry);
        next.addAll(previous);
        if (next.size() > MAX_EVENTS) {
            next.subList(MAX_EVENTS, next.size()).clear();
        }
        return List.copyOf(next);
    }

    private static <T> List<T> safeList(List<T> value) {
        return value == null ? List.of() : value;
    }

    private static String normalize(String value) {
        return value == null ? "" : value.trim();
    }

    public record SocialAnonymitySnapshot(String viewer, Map<String, SocialRemoteIdentity> remotesByUuid) {
        public SocialAnonymitySnapshot {
            viewer = normalize(viewer);
            remotesByUuid = Map.copyOf(remotesByUuid == null ? Map.of() : remotesByUuid);
        }

        public static SocialAnonymitySnapshot empty() {
            return new SocialAnonymitySnapshot("", Map.of());
        }
    }

    public record SocialRemoteIdentity(
        String playerUuid,
        boolean anonymous,
        String displayName,
        String realmBand,
        String breathHint,
        List<String> renownTags
    ) {
        public SocialRemoteIdentity {
            playerUuid = normalize(playerUuid);
            displayName = normalize(displayName);
            realmBand = normalize(realmBand);
            breathHint = normalize(breathHint);
            renownTags = List.copyOf(safeList(renownTags));
        }
    }

    public record SocialExposure(
        String actor,
        String kind,
        List<String> witnesses,
        long tick,
        String zone
    ) {
        public SocialExposure {
            actor = normalize(actor);
            kind = normalize(kind);
            witnesses = List.copyOf(safeList(witnesses));
            tick = Math.max(0L, tick);
            zone = normalize(zone);
        }
    }

    public record SocialRelationshipSignal(
        String kind,
        String left,
        String right,
        String terms,
        boolean broken,
        long tick,
        String place
    ) {
        public SocialRelationshipSignal {
            kind = normalize(kind);
            left = normalize(left);
            right = normalize(right);
            terms = normalize(terms);
            tick = Math.max(0L, tick);
            place = normalize(place);
        }
    }

    public record RenownTag(String tag, double weight, long lastSeenTick, boolean permanent) {
        public RenownTag {
            tag = normalize(tag);
            lastSeenTick = Math.max(0L, lastSeenTick);
        }
    }

    public record SocialRenownDelta(
        String charId,
        int fameDelta,
        int notorietyDelta,
        List<RenownTag> tagsAdded,
        long tick,
        String reason
    ) {
        public SocialRenownDelta {
            charId = normalize(charId);
            tagsAdded = List.copyOf(safeList(tagsAdded));
            tick = Math.max(0L, tick);
            reason = normalize(reason);
        }
    }

    public record SparringInvite(
        String inviteId,
        String initiator,
        String target,
        String realmBand,
        String breathHint,
        String terms,
        long expiresAtMs
    ) {
        public SparringInvite {
            inviteId = normalize(inviteId);
            initiator = normalize(initiator);
            target = normalize(target);
            realmBand = normalize(realmBand);
            breathHint = normalize(breathHint);
            terms = normalize(terms);
            expiresAtMs = Math.max(0L, expiresAtMs);
        }
    }
}
