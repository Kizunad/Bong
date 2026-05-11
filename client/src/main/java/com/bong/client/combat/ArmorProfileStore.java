package com.bong.client.combat;

import com.bong.client.armor.ArmorTintRegistry;
import com.bong.client.inventory.model.EquipSlotType;
import net.minecraft.entity.EquipmentSlot;

import java.util.Locale;
import java.util.Map;
import java.util.Objects;

/**
 * plan-armor-v1 §5 — client 侧护甲 profile 的最小镜像。
 *
 * <p>当前不走 server 下发：护甲 profile blueprint 仅存在于 server assets。
 * HUD/tooltip 需要在不扩 IPC 的前提下渲染矩阵，因此先做一个 client-side
 * lookup（按 template_id/itemId）。后续若补 §3 ArmorDurabilityChanged +
 * profile 下发，可替换成网络驱动 store。
 */
public final class ArmorProfileStore {
    /** Mitigation in [0, 0.85]. Wire tokens align with server WoundKind snake_case. */
    public record ArmorMitigation(
        float cut,
        float blunt,
        float pierce,
        float burn,
        float concussion
    ) {
        public ArmorMitigation {
            cut = clampCap(cut);
            blunt = clampCap(blunt);
            pierce = clampCap(pierce);
            burn = clampCap(burn);
            concussion = clampCap(concussion);
        }

        private static float clampCap(float v) {
            if (Float.isNaN(v)) return 0.0f;
            if (v < 0.0f) return 0.0f;
            // plan-armor-v1 Q7: cap 0.85
            if (v > 0.85f) return 0.85f;
            return v;
        }
    }

    private static final Map<String, ArmorMitigation> BY_ITEM_ID = Map.ofEntries(
        Map.entry("cloth_robe", new ArmorMitigation(0.10f, 0.20f, 0.05f, 0.00f, 0.10f)),
        Map.entry("fake_spirit_hide", new ArmorMitigation(0.25f, 0.30f, 0.20f, 0.10f, 0.15f)),
        Map.entry("spirit_weave_robe", new ArmorMitigation(0.35f, 0.35f, 0.35f, 0.40f, 0.35f)),
        Map.entry("iron_plate_chest", new ArmorMitigation(0.50f, 0.40f, 0.55f, 0.15f, 0.20f))
    );

    private ArmorProfileStore() {
    }

    public static ArmorMitigation mitigationForItemId(String itemId) {
        if (itemId == null) return null;
        ArmorMitigation legacy = BY_ITEM_ID.get(normalizeLegacyItemId(itemId));
        if (legacy != null) return legacy;

        ArmorTintRegistry.ArmorItemSpec mundane = ArmorTintRegistry.item(itemId);
        if (mundane == null) return null;
        float physical = (float) Math.min(0.85, Math.max(0.0, mundane.defenseForSlot() / 10.0));
        float burn = (float) Math.min(0.85, physical * 0.35f);
        return new ArmorMitigation(physical, physical, physical, burn, physical);
    }

    public static boolean isArmor(String itemId) {
        return mitigationForItemId(itemId) != null;
    }

    public static EquipSlotType equipSlotForItemId(String itemId) {
        ArmorTintRegistry.ArmorItemSpec mundane = ArmorTintRegistry.item(itemId);
        if (mundane != null) {
            return fromEquipmentSlot(mundane.slot());
        }
        String legacyId = itemId == null ? "" : normalizeLegacyItemId(itemId);
        return BY_ITEM_ID.containsKey(legacyId) ? EquipSlotType.CHEST : null;
    }

    private static EquipSlotType fromEquipmentSlot(EquipmentSlot slot) {
        return switch (slot) {
            case HEAD -> EquipSlotType.HEAD;
            case CHEST -> EquipSlotType.CHEST;
            case LEGS -> EquipSlotType.LEGS;
            case FEET -> EquipSlotType.FEET;
            default -> null;
        };
    }

    public static String kindLabel(String kindSnakeCase) {
        String k = Objects.requireNonNullElse(kindSnakeCase, "").trim().toLowerCase(Locale.ROOT);
        return switch (k) {
            case "cut" -> "斩";
            case "blunt" -> "钝";
            case "pierce" -> "刺";
            case "burn" -> "灼";
            case "concussion" -> "震";
            default -> "?";
        };
    }

    private static String normalizeLegacyItemId(String itemId) {
        return itemId.trim().toLowerCase(Locale.ROOT);
    }
}
