package com.bong.client.weapon;

import com.bong.client.combat.EquippedShield;
import com.bong.client.combat.EquippedShieldStore;
import com.bong.client.combat.EquippedWeapon;
import com.bong.client.combat.SkillBarEntry;
import com.bong.client.combat.SkillBarStore;
import com.bong.client.combat.WeaponEquippedStore;
import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;

import net.minecraft.item.ItemStack;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.Optional;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * F8：{@link HeldItemStackResolver} 饱和单测。
 *
 * <p>此 resolver 是从 {@code MixinHeldItemRenderer}（FPV）与 {@code MixinPlayerEntityHeldItem}
 * （TPV）抽出的共享 fallback 链，两处 mixin 此前各自重复实现导致过两次漏同步 bug
 * （盾 off_hand 缺失 / 锄头 TPV 缺失）。本测试锁住 fallback 优先级契约，保证任一 mixin
 * 切换到调用 resolver 后行为不变，且回归时立刻撞红。
 *
 * <p><b>为什么没有断言具体 resolve 出的 vanilla {@code Item}（如 {@code Items.IRON_SWORD}）：</b>
 * 已实测验证——{@code WeaponVanillaIconMap}/{@code ShieldVanillaIconMap}/
 * {@code BlockVanillaIconMap}/{@code HoeVanillaIconMap} 命中已注册模板时最终都会求值
 * {@code net.minecraft.item.Items} 静态字段，而这需要 MC Bootstrap——client 单测环境没有
 * （{@code ShieldVanillaIconMapTest} / {@code BlockVanillaIconMapTest} 的类注释已明确记录
 * 这条边界："happy path 由 e2e/真游戏覆盖"）。本测试遵循同一条既有约定：只测<b>不触发
 * 注册表求值</b>的分支（空/未注册/优先级门控），"resolve 出正确的 vanilla item" 由人工
 * {@code ./gradlew runClient} 目视验收（武器/盾/方块/锄头渲染此前均已是可工作功能，非本次
 * 新增，不在本次回归范围内）。
 *
 * <p>技巧：对"应被跳过"的分支用<b>已注册的真实 template_id 做诱饵</b>
 * （如 {@code torch_item}/{@code hoe_iron}/{@code wooden_shield}）——如果 resolver 的优先级
 * 逻辑有 bug 真的下探/误判到了那一层，诱饵会命中注册表触发 {@code Items} 求值直接抛异常，
 * 测试因此撞红；如果逻辑正确地跳过了该层，诱饵从未被触碰，测试安全通过并断言最终结果。
 * 这让"证明优先级正确"和"规避 MC Bootstrap 限制"能同时满足。
 */
class HeldItemStackResolverTest {

    @BeforeEach
    void setUp() {
        resetAll();
    }

    @AfterEach
    void tearDown() {
        resetAll();
    }

    private static void resetAll() {
        WeaponEquippedStore.resetForTests();
        EquippedShieldStore.resetForTests();
        SkillBarStore.resetForTests();
        InventoryStateStore.resetForTests();
    }

    // ---- resolveMainHand：全空 -----------------------------------------

    @Test
    void resolveMainHand_noStateAtAll_returnsEmpty() {
        assertTrue(HeldItemStackResolver.resolveMainHand().isEmpty(),
            "四路 fallback 全空时应返回 empty（调用方保留 vanilla 空手），实际非空");
    }

    // ---- resolveMainHand：weapon tier 优先级语义（关键回归） -----------------

    @Test
    void resolveMainHand_unmappedWeapon_doesNotFallThroughToBlockOrHoe() {
        // 关键语义：武器已装备但未注册模型时不应下探到 block/hoe——
        // 即使快捷栏和装备槽都有可用 fallback，也应保持"持有未知武器"（vanilla 空手）。
        // 与原两处 mixin 的分支条件一致：block/hoe 分支只在 weapon==null 时触发。
        //
        // torch_item / hoe_iron 是诱饵：均为真实已注册模板。若 resolver 有 bug 下探到
        // block/hoe tier，会命中注册表触发 Items 求值直接抛异常——测试因此撞红。
        WeaponEquippedStore.putOrClear("main_hand",
            new EquippedWeapon("main_hand", 1L, "totally_unregistered_weapon_xyz", "sword", 100f, 100f, 1));
        SkillBarStore.setSelectedSlot(0);
        SkillBarStore.updateSlot(0, SkillBarEntry.item("torch_item", "火把", 0, 0, ""));
        InventoryStateStore.replace(InventoryModel.builder()
            .equip(EquipSlotType.MAIN_HAND, InventoryItem.simple("hoe_iron", "铁锄"))
            .build());

        Optional<ItemStack> result = assertDoesNotThrow(
            HeldItemStackResolver::resolveMainHand,
            "不应下探到 block/hoe（若下探会命中诱饵注册表条目触发 Items 求值异常）");

        assertTrue(result.isEmpty(),
            "武器已装备但未注册模型时应返回 empty，不应下探到 block/hoe fallback；实际=" + result);
    }

    // ---- resolveMainHand：block tier 优先级语义 -----------------------------

    @Test
    void resolveMainHand_selectedSlotIsSkillKind_notBlock_fallsThroughSafely() {
        // SKILL 类型快捷栏条目不应算作 block fallback——即使其 id 与真实已注册 block
        // 模板碰巧同名（"torch_item"），只要 kind!=ITEM 就不该去查 BlockVanillaIconMap。
        // 若 kind 判断有 bug（漏检查/写反），会误查到 torch_item 触发 Items 求值异常。
        SkillBarStore.setSelectedSlot(3);
        SkillBarStore.updateSlot(3, SkillBarEntry.skill("torch_item", "碰撞诱饵", 0, 0, ""));
        // 装备槽未放锄头：hoe tier 也安全返回 empty（isHoe 对非锄头 id 只查 key，不触发注册表）。

        Optional<ItemStack> result = assertDoesNotThrow(
            HeldItemStackResolver::resolveMainHand,
            "SKILL 类型条目不应被误当 block 处理（会命中诱饵 torch_item 触发异常）");

        assertTrue(result.isEmpty(),
            "SKILL 类型快捷栏条目 + 无锄头装备 应最终返回 empty；实际=" + result);
    }

    @Test
    void resolveMainHand_selectedSlotUnknownBlockTemplate_fallsThroughToHoeTierSafely() {
        // block 模板未注册应继续下探到 hoe tier（而非在 block 失败后卡住）。
        SkillBarStore.setSelectedSlot(0);
        SkillBarStore.updateSlot(0, SkillBarEntry.item("not_a_registered_block_xyz", "？", 0, 0, ""));
        // 装备槽物品同样未注册为锄头，hoe tier 也应安全返回 empty。
        InventoryStateStore.replace(InventoryModel.builder()
            .equip(EquipSlotType.MAIN_HAND, InventoryItem.simple("not_a_hoe_either", "杂物"))
            .build());

        Optional<ItemStack> result = assertDoesNotThrow(
            HeldItemStackResolver::resolveMainHand,
            "block/hoe 均未注册时应安全返回 empty，不应抛异常");

        assertTrue(result.isEmpty(),
            "block 与 hoe 均未命中已知模板时应返回 empty；实际=" + result);
    }

    @Test
    void resolveMainHand_emptySkillBarSlot_fallsThroughToHoeTierSafely() {
        // selectedSlot 指向一个从未 updateSlot 过的槽位（entry==null）——应视为"无 block
        // 选中"，继续下探到 hoe tier。
        SkillBarStore.setSelectedSlot(5);
        InventoryStateStore.replace(InventoryModel.builder()
            .equip(EquipSlotType.MAIN_HAND, InventoryItem.simple("not_a_hoe", "杂物"))
            .build());

        Optional<ItemStack> result = assertDoesNotThrow(HeldItemStackResolver::resolveMainHand);

        assertTrue(result.isEmpty(),
            "空快捷栏槽位应下探到 hoe tier，hoe 未命中时应返回 empty；实际=" + result);
    }

    // ---- resolveMainHand：hoe tier 优先级语义 -------------------------------

    @Test
    void resolveMainHand_equippedMainHandItemIsNotHoe_returnsEmpty() {
        // 装备槽 itemId 是普通物品（非 Bong 锄头模板，即使碰巧也是一个真实武器模板 id），
        // 且没有 Weapon 组件/快捷栏选中——全部 tier 都不命中。
        // "iron_sword" 是真实武器模板但不是锄头模板：isHoe() 对它应返回 false（纯 key 查找，
        // 不触发 Items 求值），不会被误判成锄头合成 fake stack。
        InventoryStateStore.replace(InventoryModel.builder()
            .equip(EquipSlotType.MAIN_HAND, InventoryItem.simple("iron_sword", "铁剑（未走武器组件）"))
            .build());

        Optional<ItemStack> result = assertDoesNotThrow(HeldItemStackResolver::resolveMainHand,
            "非锄头 id 的 isHoe() 判定不应触发 Items 求值异常");

        assertTrue(result.isEmpty(),
            "装备槽物品不是 Bong 锄头模板时不应误命中 hoe fallback；实际=" + result);
    }

    // ---- resolveMainHand：scroll tier（plan-scroll-reading-v1 P2，第五级）------

    @Test
    void resolveMainHand_hoeUnregistered_fallsThroughToScrollTierSafely() {
        // block 与 hoe 均未命中已知模板时，链条应继续下探到 scroll tier 而非提前返回——
        // "not_a_hoe_either" 不是残卷模板，scroll tier 同样安全返回 empty（守卫短路，
        // 不触发 Items 求值）。这条测试锁住"hoe → scroll"这一级的下探关系。
        SkillBarStore.setSelectedSlot(0);
        SkillBarStore.updateSlot(0, SkillBarEntry.item("not_a_registered_block_xyz", "？", 0, 0, ""));
        InventoryStateStore.replace(InventoryModel.builder()
            .equip(EquipSlotType.MAIN_HAND, InventoryItem.simple("not_a_hoe_either", "杂物"))
            .build());

        Optional<ItemStack> result = assertDoesNotThrow(
            HeldItemStackResolver::resolveMainHand,
            "block/hoe/scroll 均未注册时应安全返回 empty，不应抛异常");

        assertTrue(result.isEmpty(),
            "block、hoe、scroll 均未命中已知模板时应返回 empty；实际=" + result);
    }

    @Test
    void resolveMainHand_equippedMainHandItemIsNotReadableScroll_returnsEmpty() {
        // 装备槽物品不是 Bong 锄头也不是可阅读残卷（即使碰巧是真实武器模板 id）——
        // isReadableScroll() 对它应返回 false（纯 key 查找），不会被误判合成 fake paper stack。
        InventoryStateStore.replace(InventoryModel.builder()
            .equip(EquipSlotType.MAIN_HAND, InventoryItem.simple("iron_sword", "铁剑（非残卷）"))
            .build());

        Optional<ItemStack> result = assertDoesNotThrow(HeldItemStackResolver::resolveMainHand,
            "非残卷 id 的 isReadableScroll() 判定不应触发 Items 求值异常");

        assertTrue(result.isEmpty(),
            "装备槽物品不是可阅读残卷模板时不应误命中 scroll fallback；实际=" + result);
    }

    @Test
    void resolveMainHand_unmappedWeapon_doesNotFallThroughToScrollTier() {
        // "scroll_meridian_primer" 是诱饵：真实已注册可阅读残卷模板。武器已装备但未注册
        // 模型时不应下探到 scroll tier（同 block/hoe 门控语义）——若下探会命中诱饵触发
        // Items 求值异常。
        WeaponEquippedStore.putOrClear("main_hand",
            new EquippedWeapon("main_hand", 1L, "totally_unregistered_weapon_xyz", "sword", 100f, 100f, 1));
        InventoryStateStore.replace(InventoryModel.builder()
            .equip(EquipSlotType.MAIN_HAND, InventoryItem.simple("scroll_meridian_primer", "《经脉浅述·残卷》"))
            .build());

        Optional<ItemStack> result = assertDoesNotThrow(
            HeldItemStackResolver::resolveMainHand,
            "不应下探到 scroll tier（若下探会命中诱饵 scroll_meridian_primer 触发 Items 求值异常）");

        assertTrue(result.isEmpty(),
            "武器已装备但未注册模型时应返回 empty，不应下探到 scroll fallback；实际=" + result);
    }

    // ---- resolveOffHand：全空 --------------------------------------------

    @Test
    void resolveOffHand_noStateAtAll_returnsEmpty() {
        assertTrue(HeldItemStackResolver.resolveOffHand().isEmpty(),
            "副手 weapon/shield 均无状态时应返回 empty");
    }

    // ---- resolveOffHand：weapon tier 优先级语义（关键回归） -------------------

    @Test
    void resolveOffHand_unmappedWeapon_doesNotFallThroughToShield() {
        // wooden_shield 是诱饵：真实已注册盾模板。若 resolver 有 bug 下探到 shield tier，
        // 会命中注册表触发 Items 求值异常。
        WeaponEquippedStore.putOrClear("off_hand",
            new EquippedWeapon("off_hand", 2L, "totally_unregistered_weapon_xyz", "dagger", 50f, 50f, 1));
        EquippedShieldStore.equip(new EquippedShield(9L, "wooden_shield", 100f, 100f));

        Optional<ItemStack> result = assertDoesNotThrow(
            HeldItemStackResolver::resolveOffHand,
            "不应下探到盾 tier（若下探会命中诱饵 wooden_shield 触发 Items 求值异常）");

        assertTrue(result.isEmpty(),
            "off_hand 武器已装备但未注册模型时不应下探到盾 fallback；实际=" + result);
    }

    // ---- resolveOffHand：shield tier ----------------------------------------

    @Test
    void resolveOffHand_noWeaponNoShield_returnsEmpty() {
        assertTrue(HeldItemStackResolver.resolveOffHand().isEmpty());
    }

    @Test
    void resolveOffHand_noWeapon_unmappedShieldTemplate_returnsEmpty() {
        // 已装备"盾"但 template_id 未在 BongWeaponModelRegistry.SHIELD_TEMPLATE_IDS 注册——
        // ShieldVanillaIconMap 的注册表守卫在查 hostItem 前短路，不触发 Items 求值。
        EquippedShieldStore.equip(new EquippedShield(9L, "not_a_registered_shield_xyz", 100f, 100f));

        Optional<ItemStack> result = assertDoesNotThrow(HeldItemStackResolver::resolveOffHand,
            "未注册盾模板不应触发 Items 求值异常");

        assertTrue(result.isEmpty(),
            "未注册盾模板应返回 empty（守卫短路，不应误合成 fake stack）；实际=" + result);
    }

    // ---- 交叉：main/off 互不干扰 ----------------------------------------------

    @Test
    void resolveOffHand_mainHandWeaponPresent_doesNotLeakIntoOffHand() {
        // main_hand 有武器（真实已注册模板 iron_sword），off_hand 什么都没有——
        // resolveOffHand() 只读 WeaponEquippedStore.get("off_hand")，不应触碰 main_hand
        // 的数据，也就不会触发 iron_sword 的 Items 求值（否则测试会直接崩，而非仅仅断言错误）。
        WeaponEquippedStore.putOrClear("main_hand",
            new EquippedWeapon("main_hand", 1L, "iron_sword", "sword", 100f, 100f, 1));

        Optional<ItemStack> result = assertDoesNotThrow(HeldItemStackResolver::resolveOffHand,
            "resolveOffHand() 不应读取/求值 main_hand 的武器数据");

        assertTrue(result.isEmpty(),
            "main_hand 武器不应泄漏进 off_hand 的 resolve 结果；实际=" + result);
    }

    @Test
    void resolveMainHand_offHandShieldPresent_doesNotLeakIntoMainHand() {
        // 反向交叉：off_hand 有盾（真实已注册模板 wooden_shield），main_hand 全空——
        // resolveMainHand() 不应查 EquippedShieldStore（盾只在 resolveOffHand 里查），
        // 也就不会触发 wooden_shield 的 Items 求值。
        EquippedShieldStore.equip(new EquippedShield(9L, "wooden_shield", 100f, 100f));

        Optional<ItemStack> result = assertDoesNotThrow(HeldItemStackResolver::resolveMainHand,
            "resolveMainHand() 不应读取/求值 off_hand 的盾数据");

        assertTrue(result.isEmpty(),
            "off_hand 盾不应泄漏进 main_hand 的 resolve 结果；实际=" + result);
    }
}
