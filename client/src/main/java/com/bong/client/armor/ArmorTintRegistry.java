package com.bong.client.armor;

import net.minecraft.entity.EquipmentSlot;
import net.minecraft.item.DyeableItem;
import net.minecraft.item.Item;
import net.minecraft.item.ItemStack;
import net.minecraft.item.Items;
import net.minecraft.text.Text;

import java.util.HashMap;
import java.util.Locale;
import java.util.Map;

/**
 * plan-armor-visual-v1：凡物盔甲 material → tint / slot / tooltip 规格。
 *
 * <p>Bong 自定义装备栏是权威；vanilla armor renderer 只需要一个染色 leather
 * stack 作为视觉宿主，数值仍完全走 server armor profile。
 */
public final class ArmorTintRegistry {
    private static final Map<String, ArmorMaterialSpec> MATERIALS = Map.ofEntries(
        Map.entry("bone", new ArmorMaterialSpec("bone", "凡物·骨制", 0xD0C8B8, 3, 80, "armor_bone")),
        Map.entry("hide", new ArmorMaterialSpec("hide", "凡物·兽皮", 0x8B6914, 5, 120, "armor_hide")),
        Map.entry("iron", new ArmorMaterialSpec("iron", "凡物·铁制", 0x555555, 8, 200, "armor_iron")),
        Map.entry("copper", new ArmorMaterialSpec("copper", "凡物·铜制", 0xB87333, 7, 160, "armor_copper")),
        Map.entry("spirit_cloth", new ArmorMaterialSpec("spirit_cloth", "凡物·灵布", 0x88BBCC, 4, 100, "armor_spirit_cloth")),
        Map.entry("scroll_wrap", new ArmorMaterialSpec("scroll_wrap", "凡物·残卷", 0xA08030, 6, 140, "armor_scroll_wrap"))
    );

    private static final Map<String, ArmorItemSpec> ITEMS = buildItems();

    private ArmorTintRegistry() {
    }

    public record ArmorMaterialSpec(
        String materialId,
        String materialLine,
        int rgb,
        int defense,
        int durabilityMax,
        String iconId
    ) {
        public int argb() {
            return 0xFF000000 | rgb;
        }
    }

    public record ArmorItemSpec(
        String itemId,
        ArmorMaterialSpec material,
        EquipmentSlot slot
    ) {
        public double defenseForSlot() {
            double ratio = switch (slot) {
                case CHEST -> 0.40;
                case LEGS -> 0.30;
                case HEAD, FEET -> 0.15;
                default -> 0.0;
            };
            return material.defense() * ratio;
        }
    }

    public static ArmorMaterialSpec material(String materialId) {
        if (materialId == null) return null;
        return MATERIALS.get(normalize(materialId));
    }

    public static ArmorItemSpec item(String itemId) {
        if (itemId == null) return null;
        return ITEMS.get(normalize(itemId));
    }

    public static boolean isMundaneArmor(String itemId) {
        return item(itemId) != null;
    }

    public static Integer tintForItemId(String itemId) {
        ArmorItemSpec item = item(itemId);
        return item == null ? null : item.material().rgb();
    }

    public static int argbForItemIdOrDefault(String itemId, int fallbackArgb) {
        ArmorItemSpec item = item(itemId);
        return item == null ? fallbackArgb : item.material().argb();
    }

    public static String materialLine(String itemId) {
        ArmorItemSpec item = item(itemId);
        return item == null ? "" : item.material().materialLine();
    }

    public static String defenseLine(String itemId) {
        ArmorItemSpec item = item(itemId);
        return item == null ? "" : "防御: +" + trimDefense(item.defenseForSlot());
    }

    public static String repairLine(String itemId) {
        ArmorItemSpec item = item(itemId);
        return item == null ? "" : "修复: 同材质 ×2 hand-craft";
    }

    public static String iconPathForItemId(String itemId) {
        ArmorItemSpec item = item(itemId);
        if (item == null) return null;
        return "bong-client:textures/gui/items/armor/" + item.material().iconId() + ".png";
    }

    public static ItemStack createLeatherArmorStack(String itemId, EquipmentSlot slot, double durability) {
        ArmorItemSpec spec = item(itemId);
        if (spec == null || spec.slot() != slot) return ItemStack.EMPTY;

        Item vanillaItem = switch (slot) {
            case HEAD -> Items.LEATHER_HELMET;
            case CHEST -> Items.LEATHER_CHESTPLATE;
            case LEGS -> Items.LEATHER_LEGGINGS;
            case FEET -> Items.LEATHER_BOOTS;
            default -> null;
        };
        if (vanillaItem == null) return ItemStack.EMPTY;

        ItemStack stack = new ItemStack(vanillaItem);
        if (stack.getItem() instanceof DyeableItem dyeableItem) {
            dyeableItem.setColor(stack, spec.material().rgb());
        }
        stack.setCustomName(Text.literal(spec.material().materialLine()));
        if (stack.getMaxDamage() > 0) {
            double clamped = Math.max(0.0, Math.min(1.0, durability));
            int damage = (int) Math.round((1.0 - clamped) * stack.getMaxDamage());
            stack.setDamage(Math.max(0, Math.min(stack.getMaxDamage(), damage)));
        }
        return stack;
    }

    public static int materialCount() {
        return MATERIALS.size();
    }

    public static int itemCount() {
        return ITEMS.size();
    }

    private static Map<String, ArmorItemSpec> buildItems() {
        Map<String, ArmorItemSpec> out = new HashMap<>();
        for (ArmorMaterialSpec material : MATERIALS.values()) {
            registerAll(out, material);
        }
        return Map.copyOf(out);
    }

    private static void registerAll(Map<String, ArmorItemSpec> out, ArmorMaterialSpec material) {
        register(out, material, "helmet", EquipmentSlot.HEAD);
        register(out, material, "chestplate", EquipmentSlot.CHEST);
        register(out, material, "leggings", EquipmentSlot.LEGS);
        register(out, material, "boots", EquipmentSlot.FEET);
    }

    private static void register(
        Map<String, ArmorItemSpec> out,
        ArmorMaterialSpec material,
        String suffix,
        EquipmentSlot slot
    ) {
        String itemId = "armor_" + material.materialId() + "_" + suffix;
        out.put(itemId, new ArmorItemSpec(itemId, material, slot));
    }

    private static String normalize(String value) {
        return value.trim().toLowerCase(Locale.ROOT);
    }

    private static String trimDefense(double value) {
        if (Math.abs(value - Math.rint(value)) < 1e-6) {
            return Long.toString(Math.round(value));
        }
        return String.format(Locale.ROOT, "%.2f", value);
    }
}
