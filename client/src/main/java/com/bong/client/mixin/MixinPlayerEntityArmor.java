package com.bong.client.mixin;

import com.bong.client.armor.ArmorFeatureRenderer;
import com.bong.client.armor.ArmorModelRegistry;
import com.bong.client.armor.ArmorTintRegistry;
import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.state.InventoryStateStore;
import net.minecraft.client.MinecraftClient;
import net.minecraft.entity.EquipmentSlot;
import net.minecraft.entity.LivingEntity;
import net.minecraft.entity.player.PlayerEntity;
import net.minecraft.item.ItemStack;
import net.minecraft.world.World;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * plan-armor-visual-v1：把 Bong 自定义装备栏里的凡物盔甲映射为染色 leather armor。
 *
 * <p>只对本地玩家、armor slot、原 vanilla stack 为空的情况介入；服务端战斗数值
 * 仍以 `combat::armor` profile 为准。
 */
@Mixin(PlayerEntity.class)
public abstract class MixinPlayerEntityArmor {
    @Inject(method = "getEquippedStack", at = @At("RETURN"), cancellable = true)
    private void bong$overrideBongArmorStack(
        EquipmentSlot slot,
        CallbackInfoReturnable<ItemStack> cir
    ) {
        if (slot == null || !slot.isArmorSlot()) return;
        ItemStack real = cir.getReturnValue();
        if (real != null && !real.isEmpty()) return;

        LivingEntity self = (LivingEntity) (Object) this;
        World world = self.getWorld();
        if (world == null || !world.isClient) return;
        if (MinecraftClient.getInstance().player != self) return;

        EquipSlotType bongSlot = toBongSlot(slot);
        if (bongSlot == null) return;
        InventoryItem equipped = InventoryStateStore.snapshot().equipped().get(bongSlot);
        if (equipped == null || equipped.isEmpty()) return;

        // 已注册 OBJ 模型的甲：仅当 OBJ 渲染**已实装**时才跳过 leather-dye（交给 ArmorFeatureRenderer
        // 画 OBJ）。OBJ 未实装时（OBJ_RENDER_READY=false）继续走下面的 leather-dye 兜底，否则甲全透明
        // （ArmorFeatureRenderer.render 此时是 early-return 空实现）。翻 OBJ_RENDER_READY 即切回 OBJ。
        if (ArmorFeatureRenderer.OBJ_RENDER_READY
            && ArmorModelRegistry.get(equipped.itemId()).isPresent()) {
            return; // leave vanilla stack empty, ArmorFeatureRenderer will render the OBJ
        }

        // Otherwise fall back to leather dye
        ItemStack fake = ArmorTintRegistry.createLeatherArmorStack(
            equipped.itemId(),
            slot,
            equipped.durability()
        );
        if (fake.isEmpty()) return;

        cir.setReturnValue(fake);
    }

    private static EquipSlotType toBongSlot(EquipmentSlot slot) {
        return switch (slot) {
            case HEAD -> EquipSlotType.HEAD;
            case CHEST -> EquipSlotType.CHEST;
            case LEGS -> EquipSlotType.LEGS;
            case FEET -> EquipSlotType.FEET;
            default -> null;
        };
    }
}
