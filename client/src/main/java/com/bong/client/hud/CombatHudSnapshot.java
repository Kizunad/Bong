package com.bong.client.hud;

import com.bong.client.combat.CastState;
import com.bong.client.combat.CombatHudState;
import com.bong.client.combat.DefenseWindowState;
import com.bong.client.combat.QuickSlotConfig;
import com.bong.client.combat.SkillBarConfig;
import com.bong.client.combat.SpellVolumeState;
import com.bong.client.combat.UnifiedEventStream;
import com.bong.client.combat.UnlockedStyles;
import com.bong.client.combat.store.CarrierStateStore;
import com.bong.client.inventory.model.PhysicalBody;

/**
 * Per-frame bundle of combat HUD state passed from {@code BongHud.render()} into
 * {@link BongHudOrchestrator}. Kept separate from {@link BongHudStateSnapshot}
 * so legacy callers (zone / narration / visual) are untouched.
 */
public final class CombatHudSnapshot {
    private static final CombatHudSnapshot EMPTY = new CombatHudSnapshot(
        CombatHudState.empty(),
        null,
        QuickSlotConfig.empty(),
        SkillBarConfig.empty(),
        -1,
        CastState.idle(),
        UnifiedEventStream.empty(),
        SpellVolumeState.idle(),
        CarrierStateStore.State.NONE,
        DefenseWindowState.idle(),
        UnlockedStyles.none()
    );

    private final CombatHudState combatHudState;
    private final PhysicalBody physicalBody;
    private final QuickSlotConfig quickSlotConfig;
    private final SkillBarConfig skillBarConfig;
    private final int selectedHotbarSlot;
    private final CastState castState;
    private final UnifiedEventStream eventStream;
    private final SpellVolumeState spellVolumeState;
    private final CarrierStateStore.State carrierState;
    private final DefenseWindowState defenseWindowState;
    private final UnlockedStyles unlockedStyles;

    private CombatHudSnapshot(
        CombatHudState combatHudState,
        PhysicalBody physicalBody,
        QuickSlotConfig quickSlotConfig,
        SkillBarConfig skillBarConfig,
        int selectedHotbarSlot,
        CastState castState,
        UnifiedEventStream eventStream,
        SpellVolumeState spellVolumeState,
        CarrierStateStore.State carrierState,
        DefenseWindowState defenseWindowState,
        UnlockedStyles unlockedStyles
    ) {
        this.combatHudState = combatHudState;
        this.physicalBody = physicalBody;
        this.quickSlotConfig = quickSlotConfig;
        this.skillBarConfig = skillBarConfig;
        this.selectedHotbarSlot = selectedHotbarSlot;
        this.castState = castState;
        this.eventStream = eventStream;
        this.spellVolumeState = spellVolumeState;
        this.carrierState = carrierState;
        this.defenseWindowState = defenseWindowState;
        this.unlockedStyles = unlockedStyles;
    }

    public static CombatHudSnapshot empty() {
        return EMPTY;
    }

    public static CombatHudSnapshot create(
        CombatHudState combatHudState,
        PhysicalBody physicalBody,
        QuickSlotConfig quickSlotConfig,
        SkillBarConfig skillBarConfig,
        int selectedHotbarSlot,
        CastState castState,
        UnifiedEventStream eventStream,
        SpellVolumeState spellVolumeState,
        CarrierStateStore.State carrierState,
        DefenseWindowState defenseWindowState,
        UnlockedStyles unlockedStyles
    ) {
        return new CombatHudSnapshot(
            combatHudState == null ? CombatHudState.empty() : combatHudState,
            physicalBody,
            quickSlotConfig == null ? QuickSlotConfig.empty() : quickSlotConfig,
            skillBarConfig == null ? SkillBarConfig.empty() : skillBarConfig,
            selectedHotbarSlot,
            castState == null ? CastState.idle() : castState,
            eventStream == null ? UnifiedEventStream.empty() : eventStream,
            spellVolumeState == null ? SpellVolumeState.idle() : spellVolumeState,
            carrierState == null ? CarrierStateStore.State.NONE : carrierState,
            defenseWindowState == null ? DefenseWindowState.idle() : defenseWindowState,
            unlockedStyles == null ? UnlockedStyles.none() : unlockedStyles
        );
    }

    public CombatHudState combatHudState() { return combatHudState; }
    public PhysicalBody physicalBody() { return physicalBody; }
    public QuickSlotConfig quickSlotConfig() { return quickSlotConfig; }
    public SkillBarConfig skillBarConfig() { return skillBarConfig; }
    public int selectedHotbarSlot() { return selectedHotbarSlot; }
    public CastState castState() { return castState; }
    public UnifiedEventStream eventStream() { return eventStream; }
    public SpellVolumeState spellVolumeState() { return spellVolumeState; }
    public CarrierStateStore.State carrierState() { return carrierState; }
    public DefenseWindowState defenseWindowState() { return defenseWindowState; }
    public UnlockedStyles unlockedStyles() { return unlockedStyles; }
}
