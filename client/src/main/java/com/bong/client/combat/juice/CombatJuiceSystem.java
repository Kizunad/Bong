package com.bong.client.combat.juice;

import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.atomic.AtomicBoolean;

public final class CombatJuiceSystem {
    private static final int FULL_CHARGE_FLASH_ARGB = 0x4DFFFFFF;
    private static final int OVERLOAD_FLASH_ARGB = 0x66FF2020;

    private static final AtomicBoolean BOOTSTRAPPED = new AtomicBoolean(false);
    private static volatile LastCommand lastCommand = LastCommand.empty();
    private static volatile Overlay activeOverlay = Overlay.none();
    private static volatile ParryDodgeJuicePlanner.ParryPlan lastParry = null;
    private static volatile ParryDodgeJuicePlanner.DodgeGhost lastGhost = null;

    private CombatJuiceSystem() {
    }

    public static void bootstrap() {
        if (!BOOTSTRAPPED.compareAndSet(false, true)) {
            return;
        }
        ClientTickEvents.END_CLIENT_TICK.register(client -> tick(System.currentTimeMillis()));
    }

    public static LastCommand accept(CombatJuiceEvent event) {
        return accept(event, event == null ? System.currentTimeMillis() : event.receivedAtMs());
    }

    public static LastCommand accept(CombatJuiceEvent event, long nowMs) {
        if (event == null) {
            return LastCommand.empty();
        }
        CombatJuiceProfile profile = CombatJuiceProfile.select(event.school(), event.tier());
        CombatJuiceProfile effectiveProfile = profile;
        List<HitStopController.Freeze> freezes = new ArrayList<>();
        EntityTintController.Tint tint = EntityTintController.Tint.none();
        CameraShakeController.Shake shake = CameraShakeController.Shake.none();

        switch (event.kind()) {
            case HIT -> {
                freezes.addAll(HitStopController.request(event.attackerUuid(), event.targetUuid(), profile, nowMs));
                shake = CameraShakeController.trigger(profile, event.directionX(), event.directionZ(), nowMs);
            }
            case QI_COLLISION -> {
                tint = EntityTintController.trigger(event.targetUuid(), profile, nowMs);
                activeOverlay = Overlay.screenTint(profile.qiColorArgb(), 4, nowMs, false);
            }
            case FULL_CHARGE -> {
                CombatJuiceProfile critical = CombatJuiceProfile.select(event.school(), CombatJuiceTier.CRITICAL);
                effectiveProfile = critical;
                freezes.addAll(HitStopController.request(event.attackerUuid(), event.targetUuid(), critical, nowMs));
                shake = CameraShakeController.trigger(critical, event.directionX(), event.directionZ(), nowMs);
                tint = EntityTintController.trigger(event.targetUuid(), critical, nowMs);
                activeOverlay = Overlay.screenTint(FULL_CHARGE_FLASH_ARGB, 2, nowMs, false);
            }
            case OVERLOAD -> {
                CombatJuiceProfile critical = CombatJuiceProfile.select(event.school(), CombatJuiceTier.CRITICAL);
                effectiveProfile = critical;
                freezes.addAll(HitStopController.request(event.attackerUuid(), event.targetUuid(), critical, nowMs));
                shake = CameraShakeController.trigger(critical, event.directionX(), event.directionZ(), nowMs);
                activeOverlay = Overlay.screenTint(OVERLOAD_FLASH_ARGB, 10, nowMs, true);
            }
            case PARRY, PERFECT_PARRY -> {
                boolean perfect = event.kind() == CombatJuiceEvent.Kind.PERFECT_PARRY;
                lastParry = ParryDodgeJuicePlanner.parry(event, perfect);
                activeOverlay = Overlay.screenTint(lastParry.screenFlashArgb(), perfect ? 1 : 3, nowMs, false);
                CombatJuiceProfile parryProfile = perfect
                    ? CombatJuiceProfile.select(event.school(), CombatJuiceTier.HEAVY)
                    : CombatJuiceProfile.select(event.school(), CombatJuiceTier.LIGHT);
                effectiveProfile = parryProfile;
                freezes.addAll(HitStopController.request(event.attackerUuid(), event.targetUuid(), parryProfile, nowMs));
                shake = CameraShakeController.trigger(parryProfile, event.directionX(), event.directionZ(), nowMs);
            }
            case DODGE -> lastGhost = ParryDodgeJuicePlanner.dodge(event.targetUuid(), profile.qiColorArgb(), nowMs);
            case KILL -> {
                CombatJuiceProfile killProfile = CombatJuiceProfile.select(event.school(), CombatJuiceTier.CRITICAL);
                effectiveProfile = killProfile;
                KillJuiceController.trigger(event, killProfile, nowMs);
            }
            case WOUND -> {
            }
        }

        Overlay overlaySnapshot = activeOverlay(nowMs);
        LastCommand command = new LastCommand(event, effectiveProfile, List.copyOf(freezes), shake, tint, overlaySnapshot, lastParry, lastGhost);
        lastCommand = command;
        return command;
    }

    public static LastCommand lastCommand() {
        return lastCommand;
    }

    public static ParryDodgeJuicePlanner.ParryPlan lastParry() {
        return lastParry;
    }

    public static ParryDodgeJuicePlanner.DodgeGhost lastGhost() {
        return lastGhost;
    }

    public static Overlay activeOverlay(long nowMs) {
        Overlay overlay = activeOverlay;
        if (overlay == null || !overlay.activeAt(nowMs)) {
            activeOverlay = Overlay.none();
            return Overlay.none();
        }
        return overlay;
    }

    public static void tick(long nowMs) {
        HitStopController.tick(nowMs);
        EntityTintController.tick(nowMs);
        activeOverlay(nowMs);
    }

    public static void resetForTests() {
        lastCommand = LastCommand.empty();
        activeOverlay = Overlay.none();
        lastParry = null;
        lastGhost = null;
        HitStopController.resetForTests();
        CameraShakeController.resetForTests();
        EntityTintController.resetForTests();
        KillJuiceController.resetForTests();
    }

    public record LastCommand(
        CombatJuiceEvent event,
        CombatJuiceProfile profile,
        List<HitStopController.Freeze> freezes,
        CameraShakeController.Shake shake,
        EntityTintController.Tint tint,
        Overlay overlay,
        ParryDodgeJuicePlanner.ParryPlan parry,
        ParryDodgeJuicePlanner.DodgeGhost ghost
    ) {
        public LastCommand {
            freezes = freezes == null ? List.of() : List.copyOf(freezes);
            overlay = overlay == null ? Overlay.none() : overlay;
        }

        public static LastCommand empty() {
            return new LastCommand(null, null, List.of(), CameraShakeController.Shake.none(), EntityTintController.Tint.none(), Overlay.none(), null, null);
        }
    }

    public record Overlay(int argb, int durationTicks, long startedAtMs, boolean vignette) {
        public static Overlay none() {
            return new Overlay(0, 0, 0L, false);
        }

        public static Overlay screenTint(int argb, int durationTicks, long nowMs, boolean vignette) {
            return new Overlay(argb, Math.max(0, durationTicks), Math.max(0L, nowMs), vignette);
        }

        public long durationMillis() {
            return Math.max(0, durationTicks) * 50L;
        }

        public boolean activeAt(long nowMs) {
            return durationMillis() > 0L && nowMs - startedAtMs < durationMillis();
        }

        public int colorAt(long nowMs) {
            long duration = durationMillis();
            if (duration <= 0L) {
                return 0;
            }
            long elapsed = Math.max(0L, nowMs - startedAtMs);
            if (elapsed >= duration) {
                return 0;
            }
            int baseAlpha = (argb >>> 24) & 0xFF;
            int alpha = Math.max(0, Math.min(255, (int) Math.round(baseAlpha * (1.0 - elapsed / (double) duration))));
            return (alpha << 24) | (argb & 0x00FFFFFF);
        }
    }
}
