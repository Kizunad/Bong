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

    public static final Set<String> TOOL_TEMPLATE_IDS = Set.of(
        "axe_bone",
        "pickaxe_bone",
        "axe_iron",
        "pickaxe_iron"
    );

    /**
     * plan-shield-block-v1 后续修复（#3 手持盾无盾模型）：盾牌也走本注册表的 fake-stack +
     * SML 劫持渲染链。盾牌语义非武器（state 走 {@link com.bong.client.combat.EquippedShieldStore}），
     * 但「手持物 3D 模型」与武器/工具同一条渲染管线，故宿主 item / OBJ 映射统一收口此处，
     * 避免再开第二个 SML scope 来源导致接线孤岛。
     */
    public static final Set<String> SHIELD_TEMPLATE_IDS = Set.of(
        "wooden_shield",
        "bone_shield"
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
        entries.put("axe_bone", new Entry(
            "axe_bone",
            () -> Items.WOODEN_AXE,
            "item/wooden_axe",
            "bong:models/item/axe_bone/axe_bone.obj"
        ));
        entries.put("pickaxe_bone", new Entry(
            "pickaxe_bone",
            () -> Items.WOODEN_PICKAXE,
            "item/wooden_pickaxe",
            "bong:models/item/pickaxe_bone/pickaxe_bone.obj"
        ));
        entries.put("axe_iron", new Entry(
            "axe_iron",
            () -> Items.IRON_AXE,
            "item/iron_axe",
            "bong:models/item/axe_iron/axe_iron.obj"
        ));
        entries.put("pickaxe_iron", new Entry(
            "pickaxe_iron",
            () -> Items.IRON_PICKAXE,
            "item/iron_pickaxe",
            "bong:models/item/pickaxe_iron/pickaxe_iron.obj"
        ));
        // 工具手持模型：镐/斧/锄直接白嫖 vanilla 模型（bongObjModelPath=null → 宿主 item 即模型，
        // 不走 SML 劫持）。与 bone_sword→STONE_SWORD 同模式。server 现对 category=tool 下发
        // weapon_kind="tool" view（weapon_equipped_emit.rs），客户端默认写 WeaponEquippedStore 渲染。
        // 材质映射：骨=木 / 铜≈石 / 铁=铁 / 灵铁≈钻 / 玄铁≈下界合金（凡铁刨锄取石锄）。
        entries.put("stone_pickaxe", new Entry(
            "stone_pickaxe", () -> Items.STONE_PICKAXE, "item/stone_pickaxe", null));
        entries.put("stone_axe", new Entry(
            "stone_axe", () -> Items.STONE_AXE, "item/stone_axe", null));
        entries.put("pickaxe_copper", new Entry(
            "pickaxe_copper", () -> Items.STONE_PICKAXE, "item/stone_pickaxe", null));
        entries.put("axe_copper", new Entry(
            "axe_copper", () -> Items.STONE_AXE, "item/stone_axe", null));
        entries.put("hoe_iron", new Entry(
            "hoe_iron", () -> Items.IRON_HOE, "item/iron_hoe", null));
        entries.put("hoe_lingtie", new Entry(
            "hoe_lingtie", () -> Items.DIAMOND_HOE, "item/diamond_hoe", null));
        entries.put("hoe_xuantie", new Entry(
            "hoe_xuantie", () -> Items.NETHERITE_HOE, "item/netherite_hoe", null));
        entries.put("bao_chu", new Entry(
            "bao_chu", () -> Items.STONE_HOE, "item/stone_hoe", null));
        // 采集异形凡器（plan-tools-v1，category=tool）：server 已对 category=Tool 下发 weapon_kind="tool"
        // view，但此前缺注册表条目 → 手持空手。全部 bongObjModelPath=null 白嫖 vanilla 宿主止血，专属
        // OBJ 留后续。宿主按外形挑选，且**刻意避开已被 SML 劫持的 vanilla 路径**（item/iron_sword=
        // iron_sword.obj、item/bone=bone_dagger.obj 等），否则会渲染成被劫持的他者 OBJ 而非原版 item。
        entries.put("cai_yao_dao", new Entry(
            "cai_yao_dao", () -> Items.SHEARS, "item/shears", null));            // 采药刀：薄刃/园艺剪
        entries.put("cao_lian", new Entry(
            "cao_lian", () -> Items.WOODEN_HOE, "item/wooden_hoe", null));        // 草镰：长柄弯刃
        entries.put("dun_qi_jia", new Entry(
            "dun_qi_jia", () -> Items.FLINT_AND_STEEL, "item/flint_and_steel", null)); // 钝气夹：金属手夹
        entries.put("gua_dao", new Entry(
            "gua_dao", () -> Items.STONE_SWORD, "item/stone_sword", null));       // 刮刀：细刃（原版石剑）
        entries.put("gu_hai_qian", new Entry(
            "gu_hai_qian", () -> Items.FLINT_AND_STEEL, "item/flint_and_steel", null)); // 骨骸钳：重钳（同夹形）
        // 冰甲手套：手部凡器，借用 hand_wrap.obj（item/leather 已被劫持）——同为手戴护具，造型最近。
        entries.put("bing_jia_shou_tao", new Entry(
            "bing_jia_shou_tao", () -> Items.LEATHER, "item/leather", null));
        // 暗器（materials.toml，category=weapon + [item.weapon] dagger）：weapon_spec 分支已下发 view，
        // 此前缺注册表 → 空手。bone_spike 在新手默认 loadout（default.toml:58），开局即触发，优先级最高。
        // bone_spike 借用 bone_dagger.obj（item/bone 已被劫持）——骨制暗器复用骨刃外形合理。
        entries.put("bone_spike", new Entry(
            "bone_spike", () -> Items.BONE, "item/bone", null));                  // 骨刺：借 bone_dagger.obj
        entries.put("poison_needle", new Entry(
            "poison_needle", () -> Items.STICK, "item/stick", null));            // 毒针：细杆（原版木棍）
        entries.put("zhenyuan_mine", new Entry(
            "zhenyuan_mine", () -> Items.FIRE_CHARGE, "item/fire_charge", null)); // 真元诡雷：爆裂物
        // 锻造剑（forge.toml，category=weapon）：weapon_spec 已下发 view，此前缺注册表 → 空手。
        // 次品/粗坯统一白嫖原版石剑（造型更糙，契合 flawed）；正品青锋/灵锋借用 iron_sword.obj 基础剑型。
        // 刻意不借用 feixuan/spirit/rusted 等命名灵剑 OBJ，以免与那几把特例剑撞脸；专属 OBJ 留后续。
        entries.put("iron_sword_flawed", new Entry(
            "iron_sword_flawed", () -> Items.STONE_SWORD, "item/stone_sword", null));
        entries.put("qing_feng_sword", new Entry(
            "qing_feng_sword", () -> Items.IRON_SWORD, "item/iron_sword", null)); // 借 iron_sword.obj
        entries.put("qing_feng_sword_flawed", new Entry(
            "qing_feng_sword_flawed", () -> Items.STONE_SWORD, "item/stone_sword", null));
        entries.put("ling_feng_sword", new Entry(
            "ling_feng_sword", () -> Items.IRON_SWORD, "item/iron_sword", null)); // 借 iron_sword.obj
        entries.put("ling_feng_sword_flawed", new Entry(
            "ling_feng_sword_flawed", () -> Items.STONE_SWORD, "item/stone_sword", null));
        // 石刃（workbench_materials.toml，category=weapon dagger，手搓起点可制作）：原版石剑止血。
        // 不用 item/flint 宿主——minecraft/models/item/flint.json 是指向 crystal_shard_dagger 的孤儿
        // override（SML 未注册该 scope → missing model），改用它需删 override + 同步资源包 sha1，留后续清理。
        entries.put("stone_knife", new Entry(
            "stone_knife", () -> Items.STONE_SWORD, "item/stone_sword", null));
        // 盾牌（#3）：宿主选用 Bong 自有 item 体系里不出现的稀有 vanilla item，
        // 与武器同理（vanilla inventory 恒空，宿主 item 不会真实渲染，无碰撞风险）。
        // 木盾 → NAUTILUS_SHELL，骨盾 → PHANTOM_MEMBRANE。
        entries.put("wooden_shield", new Entry(
            "wooden_shield",
            () -> Items.NAUTILUS_SHELL,
            "item/nautilus_shell",
            "bong:models/item/wooden_shield/wooden_shield.obj"
        ));
        entries.put("bone_shield", new Entry(
            "bone_shield",
            () -> Items.PHANTOM_MEMBRANE,
            "item/phantom_membrane",
            "bong:models/item/bone_shield/bone_shield.obj"
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
