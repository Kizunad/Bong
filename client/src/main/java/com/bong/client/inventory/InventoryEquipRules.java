package com.bong.client.inventory;

import com.bong.client.armor.ArmorTintRegistry;
import com.bong.client.combat.ArmorProfileStore;
import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.SlotContents;
import net.minecraft.entity.EquipmentSlot;

import java.util.Map;
import java.util.Set;
import java.util.function.Predicate;

// plan-shield-block-v1 P1 — 改为 public 以允许 MixinMouse 调用 isShieldPublic（同包内也可直接用 isShield）。
public final class InventoryEquipRules {
    private enum WeaponKind {
        SWORD,
        SABER,
        STAFF,
        FIST,
        SPEAR,
        DAGGER,
        BOW
    }

    private static final Map<String, WeaponKind> WEAPON_KIND_BY_ITEM_ID = Map.ofEntries(
        Map.entry("iron_sword", WeaponKind.SWORD),
        Map.entry("rusted_blade", WeaponKind.SWORD),
        Map.entry("spirit_sword", WeaponKind.SWORD),
        Map.entry("flying_sword_feixuan", WeaponKind.SWORD),
        Map.entry("bronze_saber", WeaponKind.SABER),
        Map.entry("wooden_staff", WeaponKind.STAFF),
        Map.entry("bone_dagger", WeaponKind.DAGGER),
        Map.entry("bone_spike", WeaponKind.DAGGER),
        Map.entry("poison_needle", WeaponKind.DAGGER),
        Map.entry("zhenyuan_mine", WeaponKind.DAGGER),
        Map.entry("hand_wrap", WeaponKind.FIST)
    );

    private static final Set<String> HOE_TEMPLATE_IDS = Set.of(
        "hoe_iron",
        "hoe_lingtie",
        "hoe_xuantie"
    );

    private static final Set<String> TOOL_TEMPLATE_IDS = Set.of(
        "cai_yao_dao",
        "bao_chu",
        "cao_lian",
        "dun_qi_jia",
        "gua_dao",
        "gu_hai_qian",
        "bing_jia_shou_tao",
        // 采矿 / 伐木工具（server assets/items tools.toml + workbench_materials.toml，均 category="tool"）。
        // 此前漏收录 → isTool()=false → 拖不进手槽。server 按 ItemCategory::Tool 放行，阻断纯在客户端。
        "stone_pickaxe",
        "pickaxe_bone",
        "pickaxe_iron",
        "pickaxe_copper",
        "stone_axe",
        "axe_bone",
        "axe_iron",
        "axe_copper"
    );

    private static final Set<String> TREASURE_TEMPLATE_IDS = Set.of(
        "starter_talisman",
        "broken_artifact"
    );

    private static final Set<String> FALSE_SKIN_TEMPLATE_IDS = Set.of(
        "tuike_false_skin_silk",
        "tuike_rotten_wood_armor"
    );

    // plan-shield-block-v1 P0 — 凡人级物理防御盾牌，装备于 off_hand 槽。
    // 与 server ItemCategory::Shield 对齐；不走 WeaponKind 路由（盾无 weapon_spec）。
    private static final Set<String> SHIELD_TEMPLATE_IDS = Set.of(
        "wooden_shield",
        "bone_shield"
    );

    private InventoryEquipRules() {
    }

    // plan-layered-equip-v1 P4（决议 #17）：身体槽 worn 栈容量，镜像 server worn_cap（mod.rs:615）。
    // head/feet=2、chest/legs=3。手槽 held-only（不计 worn cap）。
    static int wornCap(EquipSlotType slot) {
        if (slot == null) return 0;
        return switch (slot) {
            case HEAD, FEET -> 2;
            case CHEST, LEGS -> 3;
            default -> 0; // 手槽 = held-only
        };
    }

    /**
     * plan-layered-equip-v1 P4（决议 #3/#12/#16/#17）：分层校验某件能否落入目标槽。
     *
     * <p>手槽（held）：武器/工具/盾/法宝按类型放行 + 双手武器（spear/staff）锁对侧手 + held 互斥（已占拒）。
     * 身体槽（worn 栈）：护甲（按部位）/ 伪皮 / 背包件，worn 未满才放行（满 → 拒，决议 #3 不顶替）。
     * **仅校验「能否 push 栈顶」**——「能否拖动下层」由 drop 站点按 LIFO 仅栈顶可动单独拦（决议 #12）。
     *
     * @param sourceSlot 拖拽来源槽（同槽内重排 / 从己槽移出时放宽占用约束），可 null
     */
    static boolean canEquip(
        InventoryItem item,
        EquipSlotType targetSlot,
        EquipSlotType sourceSlot,
        Map<EquipSlotType, SlotContents> equipped
    ) {
        if (item == null || item.isEmpty() || targetSlot == null) return false;

        WeaponKind weaponKind = weaponKindOf(item.itemId());
        boolean hoe = isHoe(item.itemId());
        boolean tool = isTool(item.itemId());
        boolean twoHandWeapon = weaponKind == WeaponKind.SPEAR || weaponKind == WeaponKind.STAFF;

        return switch (targetSlot) {
            case MAIN_HAND -> (weaponKind != null || hoe || tool)
                && handHeldFree(equipped, targetSlot, sourceSlot)
                // 双手武器入主手要求副手及全部多臂槽均空闲（双手锁对侧手，取代旧 TWO_HAND 专槽）。
                && (!twoHandWeapon || (handHeldFree(equipped, EquipSlotType.OFF_HAND, sourceSlot)
                    && handHeldFree(equipped, EquipSlotType.EXTRA_HAND_0, sourceSlot)
                    && handHeldFree(equipped, EquipSlotType.EXTRA_HAND_1, sourceSlot)));
            // 工具/锄头双手可用：off_hand 放行 tool/hoe（与 server mod.rs OffHand 同步）。
            // 副手不收双手武器（spear/staff 只走主手 held + 锁副手）。
            case OFF_HAND, EXTRA_HAND_0, EXTRA_HAND_1 -> ((weaponKind == WeaponKind.DAGGER || weaponKind == WeaponKind.FIST)
                || isTreasure(item)
                || isShield(item)
                || tool
                || hoe)
                && handHeldFree(equipped, targetSlot, sourceSlot)
                // 对侧主手持双手武器时，副手被锁，不可装入。
                && !mainHandHoldsTwoHand(equipped);
            // 身体槽 worn 栈：护甲（按部位）/ 伪皮 / 背包件，worn 未满才放行（决议 #17）。
            case HEAD, CHEST, LEGS, FEET ->
                (isArmorForSlot(item, targetSlot) || isFalseSkin(item) || isContainer(item))
                && wornHasRoom(equipped, targetSlot, sourceSlot);
        };
    }

    /** 该手槽 held 是否空闲（已占且非自身来源 → false，决议 #3 不顶替）。 */
    private static boolean handHeldFree(
        Map<EquipSlotType, SlotContents> equipped, EquipSlotType slot, EquipSlotType sourceSlot
    ) {
        if (slot == sourceSlot) return true;
        SlotContents contents = equipped == null ? null : equipped.get(slot);
        return contents == null || contents.held() == null;
    }

    /** 主手是否持双手武器（spear/staff）→ 锁副手/多臂。 */
    private static boolean mainHandHoldsTwoHand(Map<EquipSlotType, SlotContents> equipped) {
        SlotContents main = equipped == null ? null : equipped.get(EquipSlotType.MAIN_HAND);
        if (main == null || main.held() == null) return false;
        WeaponKind k = weaponKindOf(main.held().itemId());
        return k == WeaponKind.SPEAR || k == WeaponKind.STAFF;
    }

    /** 身体槽 worn 栈是否未满（同槽来源放宽——拖回自己来源算合法）。 */
    private static boolean wornHasRoom(
        Map<EquipSlotType, SlotContents> equipped, EquipSlotType slot, EquipSlotType sourceSlot
    ) {
        if (slot == sourceSlot) return true;
        SlotContents contents = equipped == null ? null : equipped.get(slot);
        int count = contents == null ? 0 : contents.wornCount();
        return count < wornCap(slot);
    }

    static EquipSlotType preferredWeaponQuickEquipSlot(
        InventoryItem item,
        Map<EquipSlotType, SlotContents> equipped,
        Predicate<EquipSlotType> usable
    ) {
        // 决议 #17：TWO_HAND 专槽取消，双手武器走 MAIN_HAND held。手槽优先级：主手 → 副手 → 多臂。
        EquipSlotType[] order = {
            EquipSlotType.MAIN_HAND,
            EquipSlotType.OFF_HAND,
            EquipSlotType.EXTRA_HAND_0,
            EquipSlotType.EXTRA_HAND_1
        };
        for (EquipSlotType slot : order) {
            if (isOccupied(equipped, slot)) continue;
            if (!usable.test(slot)) continue;
            if (canEquip(item, slot, null, equipped)) return slot;
        }
        return null;
    }

    static boolean canPlaceIntoHotbar(InventoryItem item) {
        // plan-shield-block-v1 P0 MAJOR #1 — 盾牌同样不能进 hotbar，必须留在 off_hand 槽。
        return isSingleCell(item) && !isWeapon(item) && !isTool(item) && !isTreasure(item) && !isArmor(item) && !isShield(item);
    }

    // plan-shield-block-v1 §P4 — 盾不能放入 QuickUse 槽（与 canPlaceIntoHotbar 排盾对齐）。
    // QuickUse 是 F 键快速使用槽，盾必须通过 off_hand 装备路径，不应能拖入此槽。
    static boolean canPlaceIntoQuickUse(InventoryItem item) {
        return isSingleCell(item) && !isShield(item);
    }

    static boolean isHoe(InventoryItem item) {
        return item != null && isHoe(item.itemId());
    }

    static boolean isWeapon(InventoryItem item) {
        return item != null && weaponKindOf(item.itemId()) != null;
    }

    static boolean isTreasure(InventoryItem item) {
        return item != null && isTreasure(item.itemId());
    }

    static boolean isTool(InventoryItem item) {
        return item != null && isTool(item.itemId());
    }

    static boolean isArmor(InventoryItem item) {
        return item != null && isArmor(item.itemId());
    }

    // plan-shield-block-v1 P0 — 盾牌判断，供 OFF_HAND 路由使用。
    static boolean isShield(InventoryItem item) {
        return item != null && isShield(item.itemId());
    }

    // plan-shield-block-v1 P1 — public 版本，供 MixinMouse 右键仲裁层跨包访问。
    public static boolean isShieldPublic(InventoryItem item) {
        return isShield(item);
    }

    private static boolean isSingleCell(InventoryItem item) {
        return item != null && item.gridWidth() == 1 && item.gridHeight() == 1;
    }

    private static boolean isArmor(String itemId) {
        return ArmorTintRegistry.isMundaneArmor(itemId) || ArmorProfileStore.isArmor(itemId);
    }

    private static boolean isArmorForSlot(InventoryItem item, EquipSlotType targetSlot) {
        if (item == null || item.isEmpty() || item.durability() <= 0.0) return false;
        ArmorTintRegistry.ArmorItemSpec mundane = ArmorTintRegistry.item(item.itemId());
        if (mundane != null) {
            return mundane.slot() == equipmentSlot(targetSlot);
        }
        return ArmorProfileStore.isArmor(item.itemId()) && ArmorProfileStore.equipSlotForItemId(item.itemId()) == targetSlot;
    }

    private static EquipmentSlot equipmentSlot(EquipSlotType slot) {
        return switch (slot) {
            case HEAD -> EquipmentSlot.HEAD;
            case CHEST -> EquipmentSlot.CHEST;
            case LEGS -> EquipmentSlot.LEGS;
            case FEET -> EquipmentSlot.FEET;
            default -> null;
        };
    }

    /** 手槽视角的「已占」= held 非空；身体槽视角 = worn 栈非空（quick-equip 仅用于手槽）。 */
    private static boolean isOccupied(Map<EquipSlotType, SlotContents> equipped, EquipSlotType slot) {
        if (equipped == null) return false;
        SlotContents contents = equipped.get(slot);
        if (contents == null) return false;
        return slot.isHand() ? contents.held() != null : contents.wornCount() > 0;
    }

    // plan-layered-equip-v1 P4（决议 #17）：背包件（容器）按 ContainerSpec.equip_slot 指向身体槽作 worn 层。
    // 客户端无 ItemRegistry，按已知模板名白名单识别（当前仓库仅 worn_grass_pouch / grass_pouch 配 container_spec）。
    private static final Set<String> CONTAINER_TEMPLATE_IDS = Set.of(
        "worn_grass_pouch",
        "grass_pouch"
    );

    static boolean isContainer(InventoryItem item) {
        return item != null && item.itemId() != null && CONTAINER_TEMPLATE_IDS.contains(item.itemId());
    }

    private static boolean isHoe(String itemId) {
        return itemId != null && HOE_TEMPLATE_IDS.contains(itemId);
    }

    private static boolean isTreasure(String itemId) {
        return itemId != null && TREASURE_TEMPLATE_IDS.contains(itemId);
    }

    private static boolean isFalseSkin(InventoryItem item) {
        return item != null && FALSE_SKIN_TEMPLATE_IDS.contains(item.itemId());
    }

    private static boolean isTool(String itemId) {
        return itemId != null && TOOL_TEMPLATE_IDS.contains(itemId);
    }

    // plan-shield-block-v1 P0 — 盾牌 id 判断（与 SHIELD_TEMPLATE_IDS 对齐）。
    private static boolean isShield(String itemId) {
        return itemId != null && SHIELD_TEMPLATE_IDS.contains(itemId);
    }

    private static WeaponKind weaponKindOf(String itemId) {
        if (itemId == null || itemId.isBlank()) return null;
        WeaponKind explicit = WEAPON_KIND_BY_ITEM_ID.get(itemId);
        if (explicit != null) return explicit;

        String normalized = itemId.toLowerCase(java.util.Locale.ROOT);
        if (normalized.contains("staff")) return WeaponKind.STAFF;
        if (normalized.contains("spear")) return WeaponKind.SPEAR;
        if (normalized.contains("dagger") || normalized.contains("needle")
            || normalized.contains("spike") || normalized.contains("knife")) {
            // "knife" → 短刃归 DAGGER：stone_knife（workbench dagger，手搓起点可制作）此前不在
            // WEAPON_KIND_BY_ITEM_ID、fallback 也不含 "knife" → weaponKindOf=null → canEquip(MAIN_HAND)=false
            // → UI 拦截拖入手槽。补 "knife" 子串放行，覆盖未来同类短刃命名。
            return WeaponKind.DAGGER;
        }
        if (normalized.contains("saber")) return WeaponKind.SABER;
        if (normalized.contains("sword") || normalized.contains("blade")) return WeaponKind.SWORD;
        if (normalized.contains("fist") || normalized.contains("wrap")) return WeaponKind.FIST;
        if (normalized.contains("bow")) return WeaponKind.BOW;
        return null;
    }
}
