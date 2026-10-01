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
import com.bong.client.scroll.ScrollVanillaIconMap;

import net.minecraft.item.ItemStack;

import java.util.Optional;

/** 统一第一、第三人称的手持渲染；依次解析武器、方块、采药锄和残卷。 */
public final class HeldItemStackResolver {

    private HeldItemStackResolver() {
    }

    /** 已装备武器但缺少模型时保持空手视觉；只有未装备武器才继续兜底。 */
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
        return hoeStack.isPresent() ? hoeStack : selectedScrollStack();
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

    /** 主手兜底：已装备的可阅读残卷。 */
    private static Optional<ItemStack> selectedScrollStack() {
        InventoryItem main = InventoryStateStore.snapshot().equipped().get(EquipSlotType.MAIN_HAND);
        if (main == null || main.isEmpty() || !ScrollVanillaIconMap.isReadableScroll(main.itemId())) {
            return Optional.empty();
        }
        return Optional.ofNullable(ScrollVanillaIconMap.createStackFor(main.itemId()));
    }
}
