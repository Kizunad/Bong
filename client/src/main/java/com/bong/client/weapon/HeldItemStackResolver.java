package com.bong.client.weapon;

import com.bong.client.block.BlockVanillaIconMap;
import com.bong.client.combat.EquippedShield;
import com.bong.client.combat.EquippedShieldStore;
import com.bong.client.combat.EquippedWeapon;
import com.bong.client.combat.SkillBarEntry;
import com.bong.client.combat.SkillBarStore;
import com.bong.client.combat.WeaponEquippedStore;
import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.lingtian.HoeVanillaIconMap;
import com.bong.client.scroll.ScrollVanillaIconMap;

import net.minecraft.item.ItemStack;

import java.util.Optional;

/**
 * F8：手持物品 fallback 解析——{@code weapon → shield(仅 off_hand) → block → hoe → scroll}。
 * 主手第五级 {@code scroll}（plan-scroll-reading-v1 P2）追加于 hoe 之后，白嫖
 * vanilla paper，逻辑同 hoe：装备槽 template_id 命中已知可阅读残卷即合成 fake stack。
 *
 * <p>此前 {@code MixinHeldItemRenderer}（FPV，hook {@code HeldItemRenderer.updateHeldItems}）
 * 与 {@code MixinPlayerEntityHeldItem}（TPV，hook {@code LivingEntity.getMainHandStack}/
 * {@code getOffHandStack}）各自独立实现了完全相同的 fallback 链 + 一份逐字相同的私有
 * {@code bong$selectedBlockStack()}。两处历史上已各自漏同步过一次（盾只接了 FPV 的
 * off_hand 兜底、锄头只接了 FPV 缺 TPV），本类把逻辑抽成单一实现，两 mixin 改为调用它。
 *
 * <p>之所以不放进 {@code com.bong.client.mixin} 包：Mixin 包禁止被普通代码直接引用，
 * 从 mixin handler 引用 mixin 包内的类会在运行时抛
 * {@code IllegalClassLoadError}（{@link com.bong.client.interaction.ClientInteractionItemResolver}
 * 的类注释记录过同一个坑，此处照抄先例：resolver 放普通包）。
 *
 * <p>纯重构：不改变任何优先级语义，不涉及 qi / 战斗数值——两个 mixin 都只影响客户端视觉
 * 渲染，vanilla attack / server 战斗逻辑读的是 Bong 自己的组件，不读这里合成的 fake stack。
 */
public final class HeldItemStackResolver {

    private HeldItemStackResolver() {
    }

    /**
     * 主手 fallback 优先级：Bong 武器 → 快捷栏当前选中方块 → 已装备的 Bong 锄头。
     * 三者皆无命中时返回 {@link Optional#empty()}（调用方保留 vanilla 空手渲染）。
     *
     * <p>注意优先级语义（照抄原两处 mixin 的分支条件，非臆造）：武器 tier 按"是否装备了
     * Bong 武器"（{@code weapon != null}）гейт，不是按"fake stack 是否合成成功"gate——
     * 装备了武器但 {@link WeaponVanillaIconMap} 未注册对应模型（{@code fake == null}）时，
     * 视觉上仍应保持"持有武器但暂无模型"（vanilla 空手），不下探去显示方块/锄头。
     * block → hoe 这一段则相反：block 合成失败会继续尝试 hoe（原代码两个 if 均只
     * gate 在 {@code bongMain == null}，天然形成失败即下探的链）。
     */
    public static Optional<ItemStack> resolveMainHand() {
        EquippedWeapon weapon = WeaponEquippedStore.mainHandRenderWeapon();
        if (weapon != null) {
            return weaponFake(weapon);
        }

        Optional<ItemStack> blockStack = selectedBlockStack();
        if (blockStack.isPresent()) {
            return blockStack;
        }

        Optional<ItemStack> hoeStack = selectedHoeStack();
        if (hoeStack.isPresent()) {
            return hoeStack;
        }

        return selectedScrollStack();
    }

    /**
     * 副手 fallback 优先级：Bong 武器 → 已装备的盾（盾非武器组件，单独走
     * {@link EquippedShieldStore}）。武器 tier 的 gate 语义同 {@link #resolveMainHand()}
     * 注释：按"是否装备"而非"是否成功合成 fake stack"判断是否下探到盾。
     */
    public static Optional<ItemStack> resolveOffHand() {
        EquippedWeapon weapon = WeaponEquippedStore.get("off_hand");
        if (weapon != null) {
            return weaponFake(weapon);
        }

        return equippedShieldStack();
    }

    private static Optional<ItemStack> weaponFake(EquippedWeapon weapon) {
        if (weapon == null) {
            return Optional.empty();
        }
        return Optional.ofNullable(WeaponVanillaIconMap.createStackFor(weapon.templateId()));
    }

    private static Optional<ItemStack> equippedShieldStack() {
        EquippedShield shield = EquippedShieldStore.snapshot();
        if (shield == null) {
            return Optional.empty();
        }
        return Optional.ofNullable(ShieldVanillaIconMap.createStackFor(shield.templateId()));
    }

    private static Optional<ItemStack> selectedBlockStack() {
        int selectedSlot = SkillBarStore.selectedSlot();
        SkillBarEntry entry = SkillBarStore.snapshot().slot(selectedSlot);
        if (entry == null || entry.kind() != SkillBarEntry.Kind.ITEM) {
            return Optional.empty();
        }
        return BlockVanillaIconMap.createStackFor(entry.id());
    }

    private static Optional<ItemStack> selectedHoeStack() {
        InventoryItem main = InventoryStateStore.snapshot().equipped().get(EquipSlotType.MAIN_HAND);
        if (main == null || main.isEmpty() || !HoeVanillaIconMap.isHoe(main.itemId())) {
            return Optional.empty();
        }
        return Optional.ofNullable(HoeVanillaIconMap.createStackFor(main.itemId()));
    }

    /**
     * plan-scroll-reading-v1 P2 — 主手第五级 fallback：已装备的可阅读残卷（白嫖 paper）。
     * 逻辑与 {@link #selectedHoeStack()} 同构，只是查 {@link ScrollVanillaIconMap}。
     */
    private static Optional<ItemStack> selectedScrollStack() {
        InventoryItem main = InventoryStateStore.snapshot().equipped().get(EquipSlotType.MAIN_HAND);
        if (main == null || main.isEmpty() || !ScrollVanillaIconMap.isReadableScroll(main.itemId())) {
            return Optional.empty();
        }
        return Optional.ofNullable(ScrollVanillaIconMap.createStackFor(main.itemId()));
    }
}
