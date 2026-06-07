package com.bong.client.dying_elder;

import com.bong.client.BongClient;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.network.ClientRequestSender;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;

/**
 * plan-dying-elder-v1 P3 — 垂死大能遭遇交互键绑定。
 *
 * <p>三个键绑定（G/H/J）仅在 {@link DyingElderEncounterStore#isActive()} 为 true 时生效：
 * <ul>
 *   <li>G — 给丹：在背包搜索第一颗 {@code hui_yuan_pill}，
 *       构造并发送 {@code GiveDanToElderReq}（{@code give_dan_to_elder} C2S）。</li>
 *   <li>H — 拒绝：日志警告（server 无 refuse 协议，仅占位；玩家移开即关闭遭遇）。</li>
 *   <li>J — 拖延：日志警告（server 无 delay 协议，仅占位）。</li>
 * </ul>
 *
 * <p><b>守恒红线</b>：此类只发 C2S 请求，绝不直接修改玩家真元、血量或任何 gameplay 数值。
 * 真元流动由 server qi_physics::ledger 负责处理。
 */
public final class DyingElderInteractionKeybindings {

    private static final String CATEGORY = "category.bong-client.dying_elder";

    // 键绑定 key 翻译路径
    private static final String KEY_GIVE_DAN = "key.bong-client.dying_elder_give_dan";
    private static final String KEY_REFUSE    = "key.bong-client.dying_elder_refuse";
    private static final String KEY_DELAY     = "key.bong-client.dying_elder_delay";

    /** 回元丹的 item template id（与 server ItemTemplateId::HuiYuanPill 对齐）。 */
    static final String HUI_YUAN_PILL_ITEM_ID = "hui_yuan_pill";

    private static KeyBinding giveDanKey;
    private static KeyBinding refuseKey;
    private static KeyBinding delayKey;
    private static boolean registered;

    private DyingElderInteractionKeybindings() {}

    /** 注册键绑定 + tick listener。由 {@link com.bong.client.BongClient#onInitializeClient()} 调用。 */
    public static void register() {
        if (registered) {
            return;
        }
        // 默认不绑定固定键（UNKNOWN_KEY = -1 = 未绑定），由玩家在键位设置中自行配置。
        // HUD 显示 "给丹[G]" 是提示标签，不强制绑定到 GLFW_KEY_G，
        // 避免与 InteractionKeybindings 的 G 键默认绑定冲突
        // （NoDuplicateDefaultGKeybindingTest 强制执行此约束）。
        giveDanKey = KeyBindingHelper.registerKeyBinding(
            new KeyBinding(KEY_GIVE_DAN, InputUtil.Type.KEYSYM, InputUtil.UNKNOWN_KEY.getCode(), CATEGORY)
        );
        refuseKey = KeyBindingHelper.registerKeyBinding(
            new KeyBinding(KEY_REFUSE, InputUtil.Type.KEYSYM, InputUtil.UNKNOWN_KEY.getCode(), CATEGORY)
        );
        delayKey = KeyBindingHelper.registerKeyBinding(
            new KeyBinding(KEY_DELAY, InputUtil.Type.KEYSYM, InputUtil.UNKNOWN_KEY.getCode(), CATEGORY)
        );
        ClientTickEvents.END_CLIENT_TICK.register(DyingElderInteractionKeybindings::onEndClientTick);
        registered = true;
        BongClient.LOGGER.info("[DyingElder] Interaction keybindings registered (give_dan/refuse/delay, defaults unbound — configure in keybinding settings).");
    }

    static void onEndClientTick(MinecraftClient client) {
        // 仅在游戏内（无 screen 覆盖）且有活跃遭遇时处理
        if (client == null || client.player == null || client.currentScreen != null) {
            consumeAllKeys(); // drain 队列防止下一帧积压
            return;
        }
        if (!DyingElderEncounterStore.isActive()) {
            consumeAllKeys();
            return;
        }

        if (consumeWasPressed(giveDanKey)) {
            handleGiveDan();
        }
        if (consumeWasPressed(refuseKey)) {
            BongClient.LOGGER.info("[DyingElder] 拒绝键（H）按下：server 无 refuse 协议（占位），玩家离开遭遇区即结束遭遇。");
        }
        if (consumeWasPressed(delayKey)) {
            BongClient.LOGGER.info("[DyingElder] 拖延键（J）按下：server 无 delay 协议（占位），当前版本无延迟效果。");
        }
    }

    /**
     * 给丹核心逻辑：
     * 1. 从 {@link DyingElderEncounterStore} 取大能 entity idx；
     * 2. 从 {@link InventoryStateStore} 扫描背包找第一颗 {@code hui_yuan_pill}；
     * 3. 构造并发送 {@code give_dan_to_elder} C2S。
     *
     * <p>背包无丹或遭遇已关闭时静默放弃（server 无论如何会校验，防重复只需 client 端提示）。
     */
    static void handleGiveDan() {
        int elderEntityIdx = DyingElderEncounterStore.getElderEntityIdx();
        if (elderEntityIdx <= 0) {
            BongClient.LOGGER.warn("[DyingElder] 给丹失败：elderEntityIdx={} 无效（大能未出现或 id 未同步）", elderEntityIdx);
            return;
        }

        long pillInstanceId = findHuiYuanPillInstanceId();
        if (pillInstanceId < 0L) {
            BongClient.LOGGER.info("[DyingElder] 给丹失败：背包中无回元丹（hui_yuan_pill）。");
            return;
        }

        BongClient.LOGGER.info(
            "[DyingElder] 发送 give_dan_to_elder: pill_instance_id={} elder_entity_id={}",
            pillInstanceId, elderEntityIdx
        );
        ClientRequestSender.sendGiveDanToElder(pillInstanceId, elderEntityIdx);
    }

    /**
     * 在 InventoryStateStore 的 pack + hotbar 中搜索第一颗 {@code hui_yuan_pill}。
     *
     * @return instance_id（≥ 0）；若未找到返回 {@code -1L}
     */
    static long findHuiYuanPillInstanceId() {
        InventoryModel inv = InventoryStateStore.snapshot();
        if (inv == null) {
            return -1L;
        }
        // 先搜格子容器（背包/布袋）
        for (InventoryModel.GridEntry entry : inv.gridItems()) {
            if (entry != null && entry.item() != null
                && HUI_YUAN_PILL_ITEM_ID.equals(entry.item().itemId())) {
                return entry.item().instanceId();
            }
        }
        // 再搜 hotbar
        for (com.bong.client.inventory.model.InventoryItem item : inv.hotbar()) {
            if (item != null && HUI_YUAN_PILL_ITEM_ID.equals(item.itemId())) {
                return item.instanceId();
            }
        }
        return -1L;
    }

    private static boolean consumeWasPressed(KeyBinding key) {
        boolean pressed = false;
        while (key != null && key.wasPressed()) {
            pressed = true;
        }
        return pressed;
    }

    /** 消耗全部未处理按键事件（防止从非战斗 screen 切回时积压触发）。 */
    private static void consumeAllKeys() {
        consumeWasPressed(giveDanKey);
        consumeWasPressed(refuseKey);
        consumeWasPressed(delayKey);
    }

    // ── 仅供测试 ─────────────────────────────────────────────────────────────

    /** 仅供测试：重置注册状态（使 register() 可重入）。 */
    static void resetForTest() {
        registered = false;
        giveDanKey = null;
        refuseKey  = null;
        delayKey   = null;
    }
}
