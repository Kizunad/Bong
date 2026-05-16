package com.bong.client.dandao;

import java.util.ArrayList;
import java.util.List;

/**
 * Client-side state for the dandao mutation visual system.
 * Synced from server via CustomPayload "bong:mutation_visual".
 */
public final class MutationVisualState {
    private static int stage = 0;
    private static double cumulativeToxin = 0.0;
    private static double meridianPenalty = 0.0;
    private static final List<MutationSlotEntry> activeSlots = new ArrayList<>();

    private MutationVisualState() {}

    public static int stage() { return stage; }
    public static double cumulativeToxin() { return cumulativeToxin; }
    public static double meridianPenalty() { return meridianPenalty; }
    public static List<MutationSlotEntry> activeSlots() { return List.copyOf(activeSlots); }

    public static void update(int newStage, double newToxin, double newPenalty, List<MutationSlotEntry> slots) {
        stage = newStage;
        cumulativeToxin = newToxin;
        meridianPenalty = newPenalty;
        activeSlots.clear();
        activeSlots.addAll(slots);
    }

    public static void reset() {
        stage = 0;
        cumulativeToxin = 0.0;
        meridianPenalty = 0.0;
        activeSlots.clear();
    }

    /** Single mutation slot entry (kind + body slot + level). */
    public record MutationSlotEntry(String kind, String bodySlot, int level) {}
}
