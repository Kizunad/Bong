package com.bong.client.weapon;

import net.minecraft.item.Item;
import net.minecraft.item.Items;

import java.util.LinkedHashMap;
import java.util.Collections;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import java.util.function.Supplier;

/**
 * plan-weapon-v1 §9：集中维护 `template_id -> baked model / vanilla宿主 item` 的注册表。
 *
 * <p>当前渲染链仍通过 fake vanilla {@link net.minecraft.item.ItemStack} 进入 SML，但不再把
 * 映射散落在多个类里。所有武器模板的宿主 item、minecraft model 路径、以及 Bong OBJ 资源路径
 * 统一在这里定义，便于后续替换为真正的 template_id -> baked model 查询。
 */
public final class BongWeaponModelRegistry {
    public record Entry(String templateId, Supplier<Item> hostItemSupplier, String vanillaModelPath, String bongObjModelPath) {
        public Item hostItem() {
            return hostItemSupplier.get();
        }
    }

    public static final Set<String> V1_WEAPON_TEMPLATE_IDS = Set.of(
        "iron_sword",
        "bronze_saber",
        "wooden_staff",
        "bone_dagger",
        "hand_wrap",
        "bone_sword",
        "lingmu_sword",
        "spirit_sword",
        "flying_sword_feixuan"
    );

    private static final Map<String, Entry> ENTRIES;
    private static final Set<String> VANILLA_MODEL_PATHS;

    static {
        Map<String, Entry> entries = new LinkedHashMap<>();
        entries.put("iron_sword", new Entry(
            "iron_sword",
            () -> Items.IRON_SWORD,
            "item/iron_sword",
            "bong:models/item/iron_sword/iron_sword.obj"
        ));
        entries.put("rusted_blade", new Entry(
            "rusted_blade",
            () -> Items.NETHERITE_SWORD,
            "item/netherite_sword",
            "bong:models/item/rusted_blade/rusted_blade.obj"
        ));
        entries.put("bronze_saber", new Entry(
            "bronze_saber",
            () -> Items.GOLDEN_SWORD,
            "item/golden_sword",
            "bong:models/item/bronze_saber/bronze_saber.obj"
        ));
        entries.put("bone_dagger", new Entry(
            "bone_dagger",
            () -> Items.BONE,
            "item/bone",
            "bong:models/item/bone_dagger/bone_dagger.obj"
        ));
        entries.put("hand_wrap", new Entry(
            "hand_wrap",
            () -> Items.LEATHER,
            "item/leather",
            "bong:models/item/hand_wrap/hand_wrap.obj"
        ));
        entries.put("bone_sword", new Entry(
            "bone_sword",
            () -> Items.STONE_SWORD,
            "item/stone_sword",
            null
        ));
        entries.put("lingmu_sword", new Entry(
            "lingmu_sword",
            () -> Items.WOODEN_SWORD,
            "item/wooden_sword",
            null
        ));
        entries.put("wooden_staff", new Entry(
            "wooden_staff",
            () -> Items.TOTEM_OF_UNDYING,
            "item/totem_of_undying",
            "bong:models/item/wooden_staff/wooden_staff.obj"
        ));
        entries.put("spirit_sword", new Entry(
            "spirit_sword",
            () -> Items.NETHER_STAR,
            "item/nether_star",
            "bong:models/item/spirit_sword/spirit_sword.obj"
        ));
        entries.put("flying_sword_feixuan", new Entry(
            "flying_sword_feixuan",
            () -> Items.DIAMOND_SWORD,
            "item/diamond_sword",
            "bong:models/item/flying_sword_feixuan/flying_sword_feixuan.obj"
        ));
        ENTRIES = Collections.unmodifiableMap(entries);
        VANILLA_MODEL_PATHS = ENTRIES.values().stream()
            .filter(entry -> entry.bongObjModelPath() != null)
            .map(Entry::vanillaModelPath)
            .collect(java.util.stream.Collectors.toUnmodifiableSet());
    }

    private BongWeaponModelRegistry() {
    }

    public static Optional<Entry> get(String templateId) {
        return Optional.ofNullable(ENTRIES.get(templateId));
    }

    public static Set<String> vanillaModelPaths() {
        return VANILLA_MODEL_PATHS;
    }
}
