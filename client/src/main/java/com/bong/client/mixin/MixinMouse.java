package com.bong.client.mixin;

import com.bong.client.botany.BotanyDragState;
import com.bong.client.botany.HarvestSessionStore;
import com.bong.client.botany.HarvestSessionViewModel;
import com.bong.client.inventory.InventoryEquipRules;
import com.bong.client.inventory.model.EquipSlotType;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.network.ClientRequestSender;
import com.bong.client.ui.ScreenTransitionController;
import com.bong.client.ui.TransitionInputPolicy;
import net.minecraft.client.Mouse;
import net.minecraft.client.MinecraftClient;
import org.lwjgl.glfw.GLFW;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(Mouse.class)
public class MixinMouse {

    /**
     * plan-shield-block-v1 P1 — 右键持举盾追踪。
     * true = 上一 tick 右键处于按住且被举盾拦截，防止 RELEASE 事件被意外漏掉。
     */
    private boolean bong$shieldRightHeld = false;

    @Inject(
        method = "updateMouse",
        at = @At("HEAD"),
        cancellable = true
    )
    private void bong$guardNullPlayerOnUpdate(CallbackInfo ci) {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client == null || client.player == null) {
            ci.cancel();
        }
    }

    @Inject(
        method = "onMouseButton(JIII)V",
        at = @At("HEAD"),
        cancellable = true
    )
    private void bong$captureHarvestPanelDrag(long window, int button, int action, int mods, CallbackInfo ci) {
        if (TransitionInputPolicy.shouldBlockMouse(ScreenTransitionController.inputLocked(), action)) {
            ci.cancel();
            return;
        }

        // ── plan-shield-block-v1 P1 §8.1 #5 — 右键举盾仲裁 ──────────────────
        // 规则（优先级从高到低）：
        //   1. 已有 UI 屏幕打开 → 不拦截，让 vanilla 处理（UI 关闭事件等）
        //      兜底：若此时仍持举盾(bong$shieldRightHeld==true)，强制发 LowerShield
        //      防止「持举时打开背包再松键」导致服务端永远卡在 ShieldBlocking。
        //   2. off_hand 有盾牌 → PRESS 举盾 + cancel；RELEASE 放盾 + cancel
        //   3. 否则 → pass through（vanilla 右键使用/放置）
        // 注意：consumable 判断不在客户端做，让 server 的 ItemCategory 负责防止双重消费。
        if (button == GLFW.GLFW_MOUSE_BUTTON_RIGHT) {
            if (action == GLFW.GLFW_PRESS || action == GLFW.GLFW_RELEASE) {
                MinecraftClient client = MinecraftClient.getInstance();
                // UI 打开时强制兜底放盾：持举中打开背包再松键，RELEASE 会落入此分支；
                // 强制发 LowerShield + 重置 flag，防止服务端格挡状态永远残留。
                if (client != null && client.currentScreen != null && bong$shieldRightHeld) {
                    bong$shieldRightHeld = false;
                    ClientRequestSender.sendLowerShield();
                }
                // 若有 UI screen 打开则不拦截
                if (client != null && client.currentScreen == null) {
                    InventoryModel invSnapshot = InventoryStateStore.snapshot();
                    InventoryItem offHand = invSnapshot.equipped().get(EquipSlotType.OFF_HAND);
                    if (InventoryEquipRules.isShieldPublic(offHand)) {
                        if (action == GLFW.GLFW_PRESS && !bong$shieldRightHeld) {
                            bong$shieldRightHeld = true;
                            ClientRequestSender.sendRaiseShield();
                            ci.cancel();
                            return;
                        } else if (action == GLFW.GLFW_RELEASE && bong$shieldRightHeld) {
                            bong$shieldRightHeld = false;
                            ClientRequestSender.sendLowerShield();
                            ci.cancel();
                            return;
                        }
                    } else if (bong$shieldRightHeld) {
                        // 防御性：盾牌被卸下时强制结束举盾状态
                        bong$shieldRightHeld = false;
                        ClientRequestSender.sendLowerShield();
                    }
                }
            }
        }
        // ── 右键举盾仲裁结束 ────────────────────────────────────────────────────

        if (button != GLFW.GLFW_MOUSE_BUTTON_LEFT) {
            return;
        }
        MinecraftClient client = MinecraftClient.getInstance();
        if (client == null || client.currentScreen != null) {
            return;
        }
        HarvestSessionViewModel session = HarvestSessionStore.snapshot();
        if (!session.interactive()) {
            return;
        }
        double mx = client.mouse.getX() * client.getWindow().getScaledWidth()
            / (double) client.getWindow().getWidth();
        double my = client.mouse.getY() * client.getWindow().getScaledHeight()
            / (double) client.getWindow().getHeight();
        int translatedAction = action == GLFW.GLFW_PRESS ? 1 : action == GLFW.GLFW_RELEASE ? 0 : -1;
        if (translatedAction < 0) {
            return;
        }
        if (BotanyDragState.onLeftButton(translatedAction, mx, my)) {
            ci.cancel();
        }
    }
}
