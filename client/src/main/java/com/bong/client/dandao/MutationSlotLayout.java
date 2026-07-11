package com.bong.client.dandao;

import java.util.Map;

/**
 * plan-race-system-v1 P0 review r3 (major x3 收口) -- Client-side mirror of the
 * shared BodySlot -&gt; BodyPartId contract declared in
 * {@code server/assets/body_plans/plans/humanoid.json}'s {@code mutation_slot_mapping}
 * field, plus per-part rendering anchor data extracted from
 * {@link MutationFeatureRenderer}'s previous hardcoded per-mutation-kind offset hack.
 *
 * <p>This is the single source of truth for "which body part does this BodySlot
 * attach to, and where should the overlay render" on the client -- it replaces the
 * private hardcoded {@code MutationKind.defaultBodySlot} table (which was unused dead
 * weight duplicating server data) and the ordinal-based z-fight scale hack in
 * {@code MutationFeatureRenderer}.
 *
 * <p>Loaded from classpath resource
 * {@code assets/bong/body_plans/humanoid_mutation_slots.json} by
 * {@link MutationSlotLayoutRegistry}; the {@code slots} map keys are the exact
 * PascalCase {@code BodySlot} wire strings ("Head"/"Forearm"/"Back"/"Torso"/"Lower",
 * see {@code dandao::mutation::BodySlot} Rust Debug output) sent over
 * {@code bong:mutation_visual} and stored in
 * {@link MutationVisualState.MutationSlotEntry#bodySlot()}.
 */
public record MutationSlotLayout(String bodyPlanId, Map<String, SlotEntry> slots) {
    public MutationSlotLayout {
        bodyPlanId = bodyPlanId == null || bodyPlanId.isBlank() ? "humanoid" : bodyPlanId;
        slots = Map.copyOf(slots == null ? Map.of() : slots);
    }

    /**
     * Looks up the layout entry for a wire {@code body_slot} string. Returns
     * {@code null} for an unknown/unmapped slot -- callers (see
     * {@link MutationFeatureRenderer#resolveRenderable}) must treat this as an
     * explicit "do not render" signal, not silently fall back to some default
     * position (that was exactly the review-flagged failure mode this contract
     * replaces).
     */
    public SlotEntry forBodySlot(String bodySlot) {
        if (bodySlot == null) {
            return null;
        }
        return slots.get(bodySlot);
    }

    /** Which {@link BodyPartId} (server body_plan part id) this slot attaches to, plus the render anchor. */
    public record SlotEntry(String partId, Anchor anchor) {
        public SlotEntry {
            partId = partId == null ? "" : partId;
            anchor = anchor == null ? Anchor.IDENTITY : anchor;
        }
    }

    /** Model positioning anchor for the overlay quad: translation offset (block units) + uniform scale. */
    public record Anchor(float offsetX, float offsetY, float offsetZ, float scale) {
        public static final Anchor IDENTITY = new Anchor(0.0f, 0.0f, 0.0f, 1.0f);
    }
}
