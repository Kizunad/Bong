package com.bong.client.inventory;

import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.SlotContents;
import org.junit.jupiter.api.Test;

import java.util.EnumMap;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * plan-layered-equip-v1 P4（决议 #3/#12/#17）：InventoryEquipRules.canEquip 分层校验饱和测试。
 * 签名升级为 Map&lt;EquipSlotType,SlotContents&gt;；TWO_HAND 专槽删除（双手武器走 MAIN_HAND held + 锁 OFF_HAND）；
 * 身体槽 worn 栈 cap + 背包件 worn 化 + held 互斥。
 */
class InventoryEquipRulesTest {

    @Test
    void swordCanEquipMainAndExtraHandButNotOffHandOrHotbar() {
        InventoryItem sword = item(1001L, "iron_sword", 1, 2);

        assertTrue(InventoryEquipRules.canEquip(sword, EquipSlotType.MAIN_HAND, null, equipped()));
        assertTrue(InventoryEquipRules.canEquip(sword, EquipSlotType.EXTRA_HAND_0, null, equipped()),
            "期望一般武器 iron_sword 可装入 EXTRA_HAND_0，与 server extra_hand=独立 held 槽契约一致");
        assertTrue(InventoryEquipRules.canEquip(sword, EquipSlotType.EXTRA_HAND_1, null, equipped()),
            "期望一般武器 iron_sword 可装入 EXTRA_HAND_1，与 server extra_hand=独立 held 槽契约一致");
        assertFalse(InventoryEquipRules.canEquip(sword, EquipSlotType.OFF_HAND, null, equipped()));
        assertFalse(InventoryEquipRules.canPlaceIntoHotbar(item(1002L, "bone_dagger", 1, 1)));
    }

    @Test
    void daggerCanFallbackIntoOffHandWhenMainHandOccupied() {
        InventoryItem dagger = item(1001L, "bone_dagger", 1, 1);
        EnumMap<EquipSlotType, SlotContents> equipped = equipped();
        equipped.put(EquipSlotType.MAIN_HAND, SlotContents.ofHeld(item(2002L, "iron_sword", 1, 2)));

        assertEquals(
            EquipSlotType.OFF_HAND,
            InventoryEquipRules.preferredWeaponQuickEquipSlot(dagger, equipped, slot -> true)
        );
    }

    @Test
    void quickEquipDoesNotRouteWeaponsIntoArmorSlots() {
        InventoryItem sword = item(1001L, "iron_sword", 1, 2);

        assertEquals(
            EquipSlotType.MAIN_HAND,
            InventoryEquipRules.preferredWeaponQuickEquipSlot(sword, equipped(), slot -> true)
        );
    }

    // ── 双手武器（spear/staff）：走 MAIN_HAND held + 锁 OFF_HAND（取代旧 TWO_HAND 专槽，决议 #17） ──

    @Test
    void twoHandWeaponEntersMainHandWhenOffHandFree() {
        InventoryItem staff = item(1001L, "wooden_staff", 1, 3);

        assertTrue(InventoryEquipRules.canEquip(staff, EquipSlotType.MAIN_HAND, null, equipped()),
            "期望双手杖在副手空闲时可入主手 held，实际被拒——检查 twoHandWeapon 路由");
    }

    @Test
    void twoHandWeaponRejectedFromMainHandWhenOffHandOccupied() {
        InventoryItem staff = item(1001L, "wooden_staff", 1, 3);
        EnumMap<EquipSlotType, SlotContents> equipped = equipped();
        equipped.put(EquipSlotType.OFF_HAND, SlotContents.ofHeld(item(2002L, "bone_dagger", 1, 1)));

        assertFalse(InventoryEquipRules.canEquip(staff, EquipSlotType.MAIN_HAND, null, equipped),
            "期望双手杖在副手已占时入主手被拒（双手需锁对侧手），实际放行——检查 OFF_HAND held 检查");
    }

    @Test
    void twoHandMainHandLocksOnlyOffHand() {
        // 主手持双手武器 → 只锁副手；extra_hand 独立不锁。
        EnumMap<EquipSlotType, SlotContents> equipped = equipped();
        equipped.put(EquipSlotType.MAIN_HAND, SlotContents.ofHeld(item(1001L, "wooden_staff", 1, 3)));

        assertFalse(InventoryEquipRules.canEquip(item(7001L, "wooden_shield", 1, 2),
                EquipSlotType.OFF_HAND, null, equipped),
            "期望主手持双手武器时副手被锁、盾不可装入，实际放行——检查 mainHandHoldsTwoHand");
        assertTrue(InventoryEquipRules.canEquip(item(2002L, "iron_sword", 1, 2),
                EquipSlotType.EXTRA_HAND_0, null, equipped),
            "期望主手持双手武器时 EXTRA_HAND_0 仍可装一般武器，实际被错误双手锁拦截");
        assertTrue(InventoryEquipRules.canEquip(item(2003L, "wooden_staff", 1, 3),
                EquipSlotType.EXTRA_HAND_1, null, equipped),
            "期望主手持双手武器时 EXTRA_HAND_1 仍可装 weapon/tool/hoe，实际被错误双手锁拦截");
    }

    @Test
    void heldHandRejectsSecondItemWhenOccupied() {
        // held 互斥（决议 #3 不顶替）：主手已持械，再拖武器入主手被拒。
        EnumMap<EquipSlotType, SlotContents> equipped = equipped();
        equipped.put(EquipSlotType.MAIN_HAND, SlotContents.ofHeld(item(1001L, "iron_sword", 1, 2)));

        assertFalse(InventoryEquipRules.canEquip(item(1003L, "bronze_saber", 1, 2),
                EquipSlotType.MAIN_HAND, null, equipped),
            "期望主手已持械时再装武器被拒（held 互斥不顶替），实际放行");
    }

    @Test
    void sameSourceHandAllowsReorder() {
        // 同槽来源放宽：从己槽拖出再放回算合法（sourceSlot == targetSlot）。
        InventoryItem sword = item(1001L, "iron_sword", 1, 2);
        EnumMap<EquipSlotType, SlotContents> equipped = equipped();
        equipped.put(EquipSlotType.MAIN_HAND, SlotContents.ofHeld(sword));

        assertTrue(InventoryEquipRules.canEquip(sword, EquipSlotType.MAIN_HAND,
                EquipSlotType.MAIN_HAND, equipped),
            "期望从主手拖出再放回主手合法（同槽放宽），实际被拒");
    }

    @Test
    void extraHandAcceptsDaggerWhenFree() {
        InventoryItem dagger = item(2002L, "bone_dagger", 1, 1);
        assertTrue(InventoryEquipRules.canEquip(dagger, EquipSlotType.EXTRA_HAND_0, null, equipped()),
            "期望匕首可入空闲多臂槽，实际被拒");
        assertTrue(InventoryEquipRules.canEquip(dagger, EquipSlotType.EXTRA_HAND_1, null, equipped()),
            "期望匕首可入第二多臂槽，实际被拒");
    }

    @Test
    void extraHandAcceptsGeneralWeaponsAndTwoHandWeaponsWhenFree() {
        assertTrue(InventoryEquipRules.canEquip(item(2101L, "iron_sword", 1, 2),
                EquipSlotType.EXTRA_HAND_0, null, equipped()),
            "期望 EXTRA_HAND_0 接受一般剑类武器；回归到 OFF_HAND 规则会错误拒绝");
        assertTrue(InventoryEquipRules.canEquip(item(2102L, "bronze_saber", 1, 2),
                EquipSlotType.EXTRA_HAND_1, null, equipped()),
            "期望 EXTRA_HAND_1 接受刀类武器；回归到 OFF_HAND 规则会错误拒绝");
        assertTrue(InventoryEquipRules.canEquip(item(2103L, "wooden_staff", 1, 3),
                EquipSlotType.EXTRA_HAND_0, null, equipped()),
            "期望 extra_hand 接受双手杖本身；server 对 extra_hand 不套主副手互锁");
        assertTrue(InventoryEquipRules.canEquip(item(2104L, "short_bow", 2, 2),
                EquipSlotType.EXTRA_HAND_1, null, equipped()),
            "期望 bow 类 weapon 也可入 extra_hand；weaponKind!=null 即应放行");
    }

    @Test
    void toolCanEquipMainHandButNotHotbarOrArmor() {
        InventoryItem tool = item(5005L, "dun_qi_jia", 1, 1);

        assertTrue(InventoryEquipRules.canEquip(tool, EquipSlotType.MAIN_HAND, null, equipped()));
        assertTrue(InventoryEquipRules.isTool(tool));
        assertFalse(InventoryEquipRules.canEquip(tool, EquipSlotType.CHEST, null, equipped()));
        assertFalse(InventoryEquipRules.canPlaceIntoHotbar(tool));
    }

    @Test
    void quickEquipRoutesToolToMainHand() {
        InventoryItem tool = item(5005L, "dun_qi_jia", 1, 1);

        assertEquals(
            EquipSlotType.MAIN_HAND,
            InventoryEquipRules.preferredWeaponQuickEquipSlot(tool, equipped(), slot -> true)
        );
    }

    @Test
    void stonePickaxeCanEquipAllHeldHands() {
        InventoryItem pickaxe = item(6006L, "stone_pickaxe", 1, 2);

        assertTrue(InventoryEquipRules.isTool(pickaxe), "石镐应被识别为工具(白名单已补全)");
        assertTrue(InventoryEquipRules.canEquip(pickaxe, EquipSlotType.MAIN_HAND, null, equipped()),
            "石镐应能装主手");
        assertTrue(InventoryEquipRules.canEquip(pickaxe, EquipSlotType.OFF_HAND, null, equipped()),
            "石镐应能装副手(工具双手可用)");
        assertTrue(InventoryEquipRules.canEquip(pickaxe, EquipSlotType.EXTRA_HAND_0, null, equipped()),
            "石镐应能装 EXTRA_HAND_0");
        assertTrue(InventoryEquipRules.canEquip(pickaxe, EquipSlotType.EXTRA_HAND_1, null, equipped()),
            "石镐应能装 EXTRA_HAND_1");
        assertFalse(InventoryEquipRules.canPlaceIntoHotbar(pickaxe), "工具不进 hotbar");
    }

    // ── plan-zhenfa-trap-client-equip-gate-v1 P0 — zhenfa 阵法工具白名单补录回归 ──
    // warning_trap/blast_trap/slow_trap/array_flag（server assets/items/zhenfa.toml，均 category="tool"）
    // 此前漏收录进 TOOL_TEMPLATE_IDS，导致 isTool()=false → canEquip(MAIN_HAND) 被拒、装不进手槽。
    // 每条锁住 isTool + 主手放行 + hotbar 拒绝，与 server ItemCategory::Tool forbidden_in_hotbar 契约对齐。

    @Test
    void warningTrapCanEquipMainHandNotHotbar() {
        InventoryItem trap = item(9101L, "warning_trap", 1, 1);

        assertTrue(InventoryEquipRules.isTool(trap),
            "期望 warning_trap 被识别为工具（zhenfa.toml category=tool），实际未识别——检查 TOOL_TEMPLATE_IDS 是否漏收");
        assertTrue(InventoryEquipRules.canEquip(trap, EquipSlotType.MAIN_HAND, null, equipped()),
            "期望 warning_trap 可装主手，实际被拒——isTool=false 会导致 canEquip(MAIN_HAND) 分支不放行");
        assertFalse(InventoryEquipRules.canPlaceIntoHotbar(trap),
            "期望 warning_trap 不能放入 hotbar（与 server ItemCategory::Tool forbidden_in_hotbar 契约对齐），实际放行");
    }

    @Test
    void blastTrapCanEquipMainHandNotHotbar() {
        InventoryItem trap = item(9102L, "blast_trap", 1, 1);

        assertTrue(InventoryEquipRules.isTool(trap),
            "期望 blast_trap 被识别为工具（zhenfa.toml category=tool），实际未识别——检查 TOOL_TEMPLATE_IDS 是否漏收");
        assertTrue(InventoryEquipRules.canEquip(trap, EquipSlotType.MAIN_HAND, null, equipped()),
            "期望 blast_trap 可装主手，实际被拒——isTool=false 会导致 canEquip(MAIN_HAND) 分支不放行");
        assertFalse(InventoryEquipRules.canPlaceIntoHotbar(trap),
            "期望 blast_trap 不能放入 hotbar（与 server ItemCategory::Tool forbidden_in_hotbar 契约对齐），实际放行");
    }

    @Test
    void slowTrapCanEquipMainHandNotHotbar() {
        InventoryItem trap = item(9103L, "slow_trap", 1, 1);

        assertTrue(InventoryEquipRules.isTool(trap),
            "期望 slow_trap 被识别为工具（zhenfa.toml category=tool），实际未识别——检查 TOOL_TEMPLATE_IDS 是否漏收");
        assertTrue(InventoryEquipRules.canEquip(trap, EquipSlotType.MAIN_HAND, null, equipped()),
            "期望 slow_trap 可装主手，实际被拒——isTool=false 会导致 canEquip(MAIN_HAND) 分支不放行");
        assertFalse(InventoryEquipRules.canPlaceIntoHotbar(trap),
            "期望 slow_trap 不能放入 hotbar（与 server ItemCategory::Tool forbidden_in_hotbar 契约对齐），实际放行");
    }

    @Test
    void arrayFlagCanEquipMainHandNotHotbar() {
        // array_flag 是 zhenfa.toml 里 grid_w=1/grid_h=2 的非单格物件——isSingleCell 已为 false，
        // 这里仍显式断言 isTool + canEquip + !canPlaceIntoHotbar 三件套，覆盖 §8.1 #2 决议并入的漏项。
        InventoryItem flag = item(9104L, "array_flag", 1, 2);

        assertTrue(InventoryEquipRules.isTool(flag),
            "期望 array_flag 被识别为工具（zhenfa.toml category=tool，§8.1 #2 并入本 plan），实际未识别");
        assertTrue(InventoryEquipRules.canEquip(flag, EquipSlotType.MAIN_HAND, null, equipped()),
            "期望 array_flag 可装主手，实际被拒——isTool=false 会导致 canEquip(MAIN_HAND) 分支不放行");
        assertFalse(InventoryEquipRules.canPlaceIntoHotbar(flag),
            "期望 array_flag 不能放入 hotbar，实际放行");
    }

    @Test
    void zhenfaToolsQuickEquipRouteToMainHand() {
        // plan §P0 验收抓手："preferredWeaponQuickEquipSlot() 能为四种物品选出手槽"。
        // 仿 quickEquipRoutesToolToMainHand(dun_qi_jia) 先例，锁住四个 zhenfa 工具的 quick-equip 选槽。
        record Case(long id, String name, int w, int h) {}
        for (Case c : new Case[] {
            new Case(9201L, "warning_trap", 1, 1),
            new Case(9202L, "blast_trap", 1, 1),
            new Case(9203L, "slow_trap", 1, 1),
            new Case(9204L, "array_flag", 1, 2),
        }) {
            assertEquals(
                EquipSlotType.MAIN_HAND,
                InventoryEquipRules.preferredWeaponQuickEquipSlot(
                    item(c.id(), c.name(), c.w(), c.h()), equipped(), slot -> true),
                "期望 " + c.name() + " quick-equip 选主手槽（isTool 后 canEquip(MAIN_HAND) 放行），"
                    + "实际未选中——检查 TOOL_TEMPLATE_IDS 补录是否让 preferredWeaponQuickEquipSlot 生效");
        }
    }

    @Test
    void toolAndHoeCanEquipOffHandAndExtraHands() {
        InventoryItem tool = item(5005L, "dun_qi_jia", 1, 1);
        InventoryItem hoe = item(7007L, "hoe_iron", 1, 2);

        assertTrue(InventoryEquipRules.canEquip(tool, EquipSlotType.OFF_HAND, null, equipped()),
            "工具应能装副手");
        assertTrue(InventoryEquipRules.canEquip(hoe, EquipSlotType.OFF_HAND, null, equipped()),
            "锄头应能装副手");
        for (EquipSlotType extraHand : List.of(EquipSlotType.EXTRA_HAND_0, EquipSlotType.EXTRA_HAND_1)) {
            assertTrue(InventoryEquipRules.canEquip(tool, extraHand, null, equipped()),
                "工具应能装 " + extraHand);
            assertTrue(InventoryEquipRules.canEquip(hoe, extraHand, null, equipped()),
                "锄头应能装 " + extraHand);
        }
    }

    @Test
    void stoneKnifeCanEquipMainAndOffHandViaKnifeFallback() {
        InventoryItem knife = item(8008L, "stone_knife", 1, 1);

        assertTrue(InventoryEquipRules.isWeapon(knife),
            "期望 isWeapon(stone_knife)=true（经 \"knife\" fallback 判为 DAGGER），实际 false");
        assertTrue(InventoryEquipRules.canEquip(knife, EquipSlotType.MAIN_HAND, null, equipped()),
            "期望 stone_knife 可装主手，实际被拒");
        assertTrue(InventoryEquipRules.canEquip(knife, EquipSlotType.OFF_HAND, null, equipped()),
            "期望 stone_knife（DAGGER）可装副手，实际被拒");
        assertFalse(InventoryEquipRules.canPlaceIntoHotbar(knife),
            "期望 stone_knife（武器）不进 hotbar，实际放行");
    }

    @Test
    void consumablesStayHotbarCompatible() {
        assertTrue(InventoryEquipRules.canPlaceIntoHotbar(item(3003L, "guyuan_pill", 1, 1)));
    }

    @Test
    void treasureCanEquipOffHandButNotHotbar() {
        InventoryItem treasure = item(4004L, "starter_talisman", 1, 1);

        assertTrue(InventoryEquipRules.canEquip(treasure, EquipSlotType.OFF_HAND, null, equipped()));
        assertFalse(InventoryEquipRules.canEquip(treasure, EquipSlotType.EXTRA_HAND_0, null, equipped()),
            "starter_talisman 是 OFF_HAND 特权，不应继承到 extra_hand");
        assertFalse(InventoryEquipRules.canPlaceIntoHotbar(treasure));
    }

    @Test
    void shieldAndTreasureCannotEquipExtraHand() {
        InventoryItem shield = item(4005L, "wooden_shield", 1, 2);
        InventoryItem treasure = item(4006L, "starter_talisman", 1, 1);

        assertTrue(InventoryEquipRules.canEquip(shield, EquipSlotType.OFF_HAND, null, equipped()),
            "盾仍应可装 OFF_HAND");
        assertFalse(InventoryEquipRules.canEquip(shield, EquipSlotType.EXTRA_HAND_0, null, equipped()),
            "盾牌只属于 OFF_HAND 特权，extra_hand 不应放行");
        assertFalse(InventoryEquipRules.canEquip(treasure, EquipSlotType.EXTRA_HAND_1, null, equipped()),
            "法宝只属于 OFF_HAND 特权，extra_hand 不应放行");
    }

    // ── 身体槽 worn 栈：护甲（按部位）+ 背包件（容器）+ worn cap（决议 #16/#17） ──

    @Test
    void mundaneArmorEquipsOnlyMatchingArmorSlot() {
        InventoryItem chestplate = item(6006L, "armor_bone_chestplate", 2, 2);

        assertTrue(InventoryEquipRules.canEquip(chestplate, EquipSlotType.CHEST, null, equipped()));
        assertFalse(InventoryEquipRules.canEquip(chestplate, EquipSlotType.HEAD, null, equipped()));
        assertTrue(InventoryEquipRules.isArmor(chestplate));
    }

    @Test
    void containerBackpackEquipsOntoBodySlotWornLayer() {
        // 决议 #17：背包件（容器）拖入身体槽算合法 worn 件，worn 未满放行。
        InventoryItem pouch = item(9001L, "worn_grass_pouch", 1, 1);

        assertTrue(InventoryEquipRules.isContainer(pouch), "期望 worn_grass_pouch 识别为容器");
        assertTrue(InventoryEquipRules.canEquip(pouch, EquipSlotType.CHEST, null, equipped()),
            "期望背包件可入空 CHEST worn 栈，实际被拒——检查 isContainer worn 化");
    }

    @Test
    void wornStackRejectsWhenSlotFull() {
        // chest cap=3：worn 满 3 层 → 拒（决议 #3 不顶替）。
        EnumMap<EquipSlotType, SlotContents> equipped = equipped();
        equipped.put(EquipSlotType.CHEST, new SlotContents(java.util.List.of(
            item(1L, "armor_bone_chestplate", 2, 2),
            item(2L, "tuike_false_skin_silk", 2, 2),
            item(3L, "worn_grass_pouch", 1, 1)
        ), null));

        assertFalse(InventoryEquipRules.canEquip(item(4L, "tuike_rotten_wood_armor", 2, 2),
                EquipSlotType.CHEST, null, equipped),
            "期望 chest worn 满 3 层时再装被拒（cap=3 不顶替），实际放行——检查 wornHasRoom");
    }

    @Test
    void wornStackAcceptsWhenBelowCap() {
        // head cap=2：已 1 层 → 还能叠第 2 层（伪皮/护甲同槽叠层）。
        EnumMap<EquipSlotType, SlotContents> equipped = equipped();
        equipped.put(EquipSlotType.HEAD, SlotContents.ofWorn(item(1L, "armor_bone_helmet", 1, 1)));

        assertTrue(InventoryEquipRules.canEquip(item(2L, "tuike_false_skin_silk", 1, 1),
                EquipSlotType.HEAD, null, equipped),
            "期望 head worn 1 层（cap=2）时还能叠第 2 层，实际被拒——检查 wornHasRoom 边界");
    }

    // Bug B（真机回归）— client FALSE_SKIN_TEMPLATE_IDS 必须镜像 server false_skin_kind_for_item
    // (tuike.rs:124)。此前漏 fake_spirit_hide（及 disguise_wrap / camouflage_net）→ client canEquip(CHEST)
    // 在 isFalseSkin=false 处把伪皮拦死，玩家拖不上胸槽（server 已认它为 SpiderSilk）。
    @Test
    void fakeSpiritHideCanEquipChestWornStack() {
        // fake_spirit_hide 2×2，空 CHEST worn 栈应放行（cap=3）。
        assertTrue(InventoryEquipRules.canEquip(item(1L, "fake_spirit_hide", 2, 2),
                EquipSlotType.CHEST, null, equipped()),
            "期望 fake_spirit_hide 可入 CHEST worn 栈（server 认它为伪皮 SpiderSilk），实际被拒"
                + "——检查 client FALSE_SKIN_TEMPLATE_IDS 是否漏 fake_spirit_hide");
    }

    // Bug B（防漂移）— 与 server false_skin_kind_for_item 全名单逐一对齐，任何一个漏收都撞红。
    @Test
    void allServerFalseSkinsCanEquipBodySlot() {
        // server tuike.rs:124 SpiderSilk 组：tuike_false_skin_silk / disguise_wrap / camouflage_net /
        // fake_spirit_hide；RottenWoodArmor 组：tuike_rotten_wood_armor。全部应能装入空身体槽 worn。
        String[] serverFalseSkins = {
            "tuike_false_skin_silk",
            "disguise_wrap",
            "camouflage_net",
            "fake_spirit_hide",
            "tuike_rotten_wood_armor"
        };
        long id = 100L;
        for (String skin : serverFalseSkins) {
            assertTrue(InventoryEquipRules.canEquip(item(id++, skin, 2, 2),
                    EquipSlotType.CHEST, null, equipped()),
                "期望 server 伪皮 `" + skin + "` 在 client 也能装入 CHEST worn 栈，实际被拒"
                    + "——client/server false_skin 名单漂移，检查 FALSE_SKIN_TEMPLATE_IDS");
        }
    }

    @Test
    void wornCapValuesMatchServer() {
        assertEquals(2, InventoryEquipRules.wornCap(EquipSlotType.HEAD), "head worn cap 应=2");
        assertEquals(2, InventoryEquipRules.wornCap(EquipSlotType.FEET), "feet worn cap 应=2");
        assertEquals(3, InventoryEquipRules.wornCap(EquipSlotType.CHEST), "chest worn cap 应=3");
        assertEquals(3, InventoryEquipRules.wornCap(EquipSlotType.LEGS), "legs worn cap 应=3");
        assertEquals(0, InventoryEquipRules.wornCap(EquipSlotType.MAIN_HAND), "手槽 held-only worn cap 应=0");
    }

    @Test
    void brokenArmorCannotEquipAndArmorCannotUseHotbar() {
        InventoryItem brokenBoots = InventoryItem.createFull(
            6007L, "armor_bone_boots", "骨甲靴", 1, 1, 1.0, "common", "", 1, 1.0, 0.0
        );

        assertFalse(InventoryEquipRules.canEquip(brokenBoots, EquipSlotType.FEET, null, equipped()));
        assertFalse(InventoryEquipRules.canPlaceIntoHotbar(brokenBoots));
    }

    // ── plan-shield-block-v1 P0 — 盾牌路由（保留回归） ──

    @Test
    void woodenShieldCanEquipOffHand() {
        InventoryItem shield = item(7001L, "wooden_shield", 1, 2);
        assertTrue(InventoryEquipRules.canEquip(shield, EquipSlotType.OFF_HAND, null, equipped()),
            "期望 wooden_shield 可装入 OFF_HAND，实际被拒");
    }

    @Test
    void boneShieldCanEquipOffHand() {
        InventoryItem shield = item(7002L, "bone_shield", 1, 2);
        assertTrue(InventoryEquipRules.canEquip(shield, EquipSlotType.OFF_HAND, null, equipped()),
            "期望 bone_shield 可装入 OFF_HAND，实际被拒");
    }

    @Test
    void shieldCannotEquipOffHandWhenMainHandHoldsTwoHand() {
        // two_hand 专槽删后：主手持双手武器锁副手 → 盾不可装入（边界，取代旧 TWO_HAND 占用语义）。
        InventoryItem shield = item(7001L, "wooden_shield", 1, 2);
        EnumMap<EquipSlotType, SlotContents> equipped = equipped();
        equipped.put(EquipSlotType.MAIN_HAND, SlotContents.ofHeld(item(9999L, "wooden_staff", 1, 3)));

        assertFalse(InventoryEquipRules.canEquip(shield, EquipSlotType.OFF_HAND, null, equipped),
            "期望主手持双手武器时盾装 OFF_HAND 被拒（副手锁），实际放行");
    }

    @Test
    void swordIsStillRejectedFromOffHand() {
        InventoryItem sword = item(1001L, "iron_sword", 1, 2);
        assertFalse(InventoryEquipRules.canEquip(sword, EquipSlotType.OFF_HAND, null, equipped()),
            "期望 iron_sword 装 OFF_HAND 仍被拒绝，实际放行");
    }

    @Test
    void quickEquipRoutesGeneralWeaponToExtraHandWhenMainHandOccupied() {
        EnumMap<EquipSlotType, SlotContents> equipped = equipped();
        equipped.put(EquipSlotType.MAIN_HAND, SlotContents.ofHeld(item(9101L, "iron_sword", 1, 2)));

        assertEquals(
            EquipSlotType.EXTRA_HAND_0,
            InventoryEquipRules.preferredWeaponQuickEquipSlot(
                item(9102L, "bronze_saber", 1, 2),
                equipped,
                slot -> true
            ),
            "主手已占且一般武器不能进 OFF_HAND 时，quick-equip 应路由到 EXTRA_HAND_0"
        );
    }

    @Test
    void isShieldReturnsTrueForKnownShields() {
        assertTrue(InventoryEquipRules.isShield(item(7001L, "wooden_shield", 1, 2)));
        assertTrue(InventoryEquipRules.isShield(item(7002L, "bone_shield", 1, 2)));
    }

    @Test
    void isShieldReturnsFalseForNonShields() {
        assertFalse(InventoryEquipRules.isShield(item(1001L, "iron_sword", 1, 2)));
        assertFalse(InventoryEquipRules.isShield(item(4004L, "starter_talisman", 1, 1)));
    }

    @Test
    void shieldCannotPlaceIntoHotbar() {
        assertFalse(InventoryEquipRules.canPlaceIntoHotbar(item(7001L, "wooden_shield", 1, 2)));
        assertFalse(InventoryEquipRules.canPlaceIntoHotbar(item(7002L, "bone_shield", 1, 2)));
        // 隔离断言：1×1 盾使 isSingleCell 不短路，唯一拦截落 !isShield。
        assertFalse(InventoryEquipRules.canPlaceIntoHotbar(item(7003L, "wooden_shield", 1, 1)),
            "期望 1×1 wooden_shield 仍不进 hotbar：唯一拦截必须是 !isShield");
    }

    // ── plan-extra-hand-client-equip-gate-v1：extra_hand 独立不参与主/副手双手锁 ──

    @Test
    void twoHandWeaponAllowedFromMainHandWhenExtraHand0Occupied() {
        // extra_hand_0 已占（held dagger）→ 双手杖入主手仍应放行，因 extra_hand 独立不锁。
        InventoryItem staff = item(1001L, "wooden_staff", 1, 3);
        EnumMap<EquipSlotType, SlotContents> equipped = equipped();
        equipped.put(EquipSlotType.EXTRA_HAND_0, SlotContents.ofHeld(item(2002L, "bone_dagger", 1, 1)));

        assertTrue(InventoryEquipRules.canEquip(staff, EquipSlotType.MAIN_HAND, null, equipped),
            "期望双手杖在 EXTRA_HAND_0 已占时仍可入主手；extra_hand 不参与 main/off 双手互锁");
    }

    @Test
    void twoHandWeaponAllowedFromMainHandWhenExtraHand1Occupied() {
        // extra_hand_1 已占（held dagger）→ 双手杖入主手仍应放行。
        InventoryItem staff = item(1001L, "wooden_staff", 1, 3);
        EnumMap<EquipSlotType, SlotContents> equipped = equipped();
        equipped.put(EquipSlotType.EXTRA_HAND_1, SlotContents.ofHeld(item(2002L, "bone_dagger", 1, 1)));

        assertTrue(InventoryEquipRules.canEquip(staff, EquipSlotType.MAIN_HAND, null, equipped),
            "期望双手杖在 EXTRA_HAND_1 已占时仍可入主手；extra_hand 不参与 main/off 双手互锁");
    }

    @Test
    void twoHandWeaponAllowedWhenAllOffHandSlotsAreFree() {
        // 副手空 → 双手杖可入主手；extra_hand 空不空不影响该契约。
        InventoryItem staff = item(1001L, "wooden_staff", 1, 3);

        assertTrue(InventoryEquipRules.canEquip(staff, EquipSlotType.MAIN_HAND, null, equipped()),
            "期望副手空时双手杖可入主手，实际被拒");
    }

    @Test
    void twoHandWeaponAllowedWhenExtraHand0OccupiedButOffHandFree() {
        // 精确边界：OFF_HAND 空但 EXTRA_HAND_0 有物 → 仍放行。
        InventoryItem staff = item(1001L, "wooden_staff", 1, 3);
        EnumMap<EquipSlotType, SlotContents> equipped = equipped();
        // OFF_HAND 保持空；只占 EXTRA_HAND_0
        equipped.put(EquipSlotType.EXTRA_HAND_0, SlotContents.ofHeld(item(3003L, "iron_sword", 1, 2)));

        assertTrue(InventoryEquipRules.canEquip(staff, EquipSlotType.MAIN_HAND, null, equipped),
            "期望 OFF_HAND 空但 EXTRA_HAND_0 占时双手杖仍可入主手，实际被错误多臂锁拦截");
    }

    private static EnumMap<EquipSlotType, SlotContents> equipped() {
        return new EnumMap<>(EquipSlotType.class);
    }

    private static InventoryItem item(long instanceId, String itemId, int gridWidth, int gridHeight) {
        return InventoryItem.createFull(
            instanceId, itemId, itemId, gridWidth, gridHeight,
            1.0, "common", "", 1, 1.0, 1.0
        );
    }
}
