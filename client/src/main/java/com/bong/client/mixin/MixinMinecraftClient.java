package com.bong.client.mixin;

import com.bong.client.inventory.InspectScreen;
import com.bong.client.inventory.InspectScreenBootstrap;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.gui.screen.ingame.InventoryScreen;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * plan-weapon-v1 §4.4 方案 A：拦截 {@code MinecraftClient.setScreen(Screen)}。
 * 当尝试打开 vanilla {@code InventoryScreen}（按 E、创造物品栏键绑定等）时，
 * 改路由到 Bong 的 {@code InspectScreen}（装备 tab）。
 *
 * <p>不拦截容器方块（箱子 / 炼丹炉）—— 那些 Screen 是 {@code HandledScreen} 的子类
 * （如 {@code GenericContainerScreen}），不是 {@code InventoryScreen}。
 *
 * <p>同时避免无限递归：若已经是 {@code InspectScreen}（我们自己 setScreen），放行。
 */
@Mixin(MinecraftClient.class)
public class MixinMinecraftClient {

    @Inject(method = "setScreen", at = @At("HEAD"), cancellable = true)
    private void bong$reroutePlayerInventoryScreen(Screen screen, CallbackInfo ci) {
        if (screen instanceof InventoryScreen) {
            InspectScreenBootstrap.openInspectScreen((MinecraftClient) (Object) this);
            ci.cancel();
        }
        // 其他 Screen（容器、MC 菜单、Bong 自己的 InspectScreen）原样放行。
        // InspectScreen 经过 openInspectScreen 也会走回 setScreen,但 Bong 的
        // Screen 不是 InventoryScreen 所以不会再进这个分支。
    }
}
