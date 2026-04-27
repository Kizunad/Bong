package com.bong.client.mixin;

import net.minecraft.entity.player.PlayerEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * plan-tsy-loot-v1 §5 — 取消 vanilla 玩家死亡掉落。
 *
 * <p>Bong 所有掉落（主世界 §十二 50% / TSY 秘境分流 / 上古遗物 spawn）由 server 端
 * {@code DroppedLootRegistry} 统一管理 + 通过 {@code dropped_loot_sync_emit} 推送给
 * client 渲染。Vanilla 的 {@link PlayerEntity#dropInventory()} 会同时把 inventory 内
 * 物品 spawn 成 ItemEntity 并清空 player inventory，这跟我们的 server-authoritative
 * 流程冲突 —— 取消即可。
 *
 * <p>Server 端的 keepInventory gamerule 是 double insurance：见
 * {@code server/src/main.rs} world init。两者都启用确保任何代码路径都不会绕过。
 *
 * <p>注意：本 mixin **只**拦截 {@link PlayerEntity#dropInventory()}，不影响：
 * <ul>
 *   <li>玩家主动 Q 键扔物品（走 {@code dropSelectedItem} → server 端 inventory 改动）
 *   <li>Mob 的 vanilla loot drop（走 {@code MobEntity#dropLoot}，不同方法）
 *   <li>玩家正常拾取地上物品（pickup 路径不受影响）
 * </ul>
 */
@Mixin(PlayerEntity.class)
public abstract class MixinPlayerEntityDrop {

    @Inject(method = "dropInventory", at = @At("HEAD"), cancellable = true)
    private void bong$cancelVanillaDeathDrop(CallbackInfo ci) {
        ci.cancel();
    }
}
