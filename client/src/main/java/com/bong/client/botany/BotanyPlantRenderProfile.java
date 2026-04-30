package com.bong.client.botany;

public record BotanyPlantRenderProfile(
    String plantId,
    String baseMeshRef,
    int tintRgb,
    Integer tintRgbSecondary,
    ModelOverlay overlay
) {
    public BotanyPlantRenderProfile {
        plantId = normalize(plantId);
        baseMeshRef = normalize(baseMeshRef);
        tintRgb = tintRgb & 0xFFFFFF;
        if (tintRgbSecondary != null) {
            tintRgbSecondary = tintRgbSecondary & 0xFFFFFF;
        }
        overlay = overlay == null ? ModelOverlay.NONE : overlay;
    }

    public int tintAt(long timeOfDay) {
        if (overlay != ModelOverlay.DUAL_PHASE || tintRgbSecondary == null) {
            return tintRgb;
        }
        long dayTime = Math.floorMod(timeOfDay, 24000L);
        return dayTime >= 12000L ? tintRgbSecondary : tintRgb;
    }

    public static BotanyPlantRenderProfile fallback(String plantId) {
        return new BotanyPlantRenderProfile(plantId, "grass", 0x88AA55, null, ModelOverlay.NONE);
    }

    private static String normalize(String value) {
        return value == null ? "" : value.trim();
    }

    public enum ModelOverlay {
        NONE,
        EMISSIVE,
        DUAL_PHASE;

        public static ModelOverlay fromWireName(String raw) {
            if (raw == null) {
                return NONE;
            }
            return switch (raw.trim().toLowerCase()) {
                case "emissive" -> EMISSIVE;
                case "dual_phase" -> DUAL_PHASE;
                default -> NONE;
            };
        }
    }
}
