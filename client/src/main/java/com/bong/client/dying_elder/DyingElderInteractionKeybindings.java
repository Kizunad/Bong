package com.bong.client.dying_elder;

import com.bong.client.BongClient;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.network.ClientRequestSender;
import com.bong.client.input.BongKeybindRegistry;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.option.KeyBinding;
import net.minecraft.client.util.InputUtil;

/**
 * plan-dying-elder-v1 P3 — 垂死大能遭遇交互键绑定。
 *
 * <p>三个键绑定仅在 {@link DyingElderEncounterStore#isActive()} 为 true 时生效：
 * <ul>
 *   <li>给丹：在背包搜索第一颗 {@code hui_yuan_pill}，
 *       构造并发送 {@code GiveDanToElderReq}（{@code give_dan_to_elder} C2S）。</li>
 *   <li>拒绝：日志警告（server 无 refuse 协议，仅占位；玩家移开即关闭遭遇）。</li>
 *   <li>拖延：日志警告（server 无 delay 协议，仅占位）。</li>
 * </ul>
 *
 * <p><b>守恒红线</b>：此类只发 C2S 请求，绝不直接修改玩家真元、血量或任何 gameplay 数值。
 * 真元流动由 server qi_physics::ledger 负责处理。
 */
public final class DyingElderInteractionKeybindings {

    private static final String CATEGORY = "category.bong-client.dying_elder";

    private static final String GIVE_DAN_OWNER = "dying_elder.give_dan";
    private static final String REFUSE_OWNER = "dying_elder.refuse";
    private static final String DELAY_OWNER = "dying_elder.delay";

    // 键绑定 key 翻译路径
    private static final String KEY_GIVE_DAN = "key.bong-client.dying_elder_give_dan";
    private static final String KEY_REFUSE    = "key.bong-client.dying_elder_refuse";
    private static final String KEY_DELAY     = "key.bong-client.dying_elder_delay";

    /** 回元丹的 item template id（与 server handle_give_dan_to_elder 校验的 "huiyuan_pill" 对齐）。 */
    static final String HUI_YUAN_PILL_ITEM_ID = "huiyuan_pill";

    /** HUD 在实际绑定为空或尚未完成注册时使用的明确文案。 */
    public static final String UNBOUND_KEY_LABEL = "未绑定";

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
        // 统一 G 路由仍由 InteractionKeybindings 唯一持有，避免新增物理默认冲突。
        giveDanKey = BongKeybindRegistry.global().register(
            new BongKeybindRegistry.BindingSpec(
                new BongKeybindRegistry.BindingOwner(GIVE_DAN_OWNER),
                KEY_GIVE_DAN,
                InputUtil.Type.KEYSYM,
                InputUtil.UNKNOWN_KEY.getCode(),
                CATEGORY
            )
        );
        refuseKey = BongKeybindRegistry.global().register(
            new BongKeybindRegistry.BindingSpec(
                new BongKeybindRegistry.BindingOwner(REFUSE_OWNER),
                KEY_REFUSE,
                InputUtil.Type.KEYSYM,
                InputUtil.UNKNOWN_KEY.getCode(),
                CATEGORY
            )
        );
        delayKey = BongKeybindRegistry.global().register(
            new BongKeybindRegistry.BindingSpec(
                new BongKeybindRegistry.BindingOwner(DELAY_OWNER),
                KEY_DELAY,
                InputUtil.Type.KEYSYM,
                InputUtil.UNKNOWN_KEY.getCode(),
                CATEGORY
            )
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
            BongClient.LOGGER.info("[DyingElder] 拒绝键按下：server 无 refuse 协议（占位），玩家离开遭遇区即结束遭遇。");
        }
        if (consumeWasPressed(delayKey)) {
            BongClient.LOGGER.info("[DyingElder] 拖延键按下：server 无 delay 协议（占位），当前版本无延迟效果。");
        }
    }

    /**
     * 返回 HUD 当前应显示的三个有效绑定标签。
     *
     * <p>读取的是 Minecraft 当前 bound key，而不是默认键；玩家在设置中重绑定后，下一帧 HUD
     * 即可反映新标签。未注册或显式 UNKNOWN 都 fail-closed 为“未绑定”。
     */
    public static BindingLabels effectiveBindingLabels() {
        return new BindingLabels(
            effectiveBindingLabel(giveDanKey),
            effectiveBindingLabel(refuseKey),
            effectiveBindingLabel(delayKey)
        );
    }

    static String effectiveBindingLabel(KeyBinding key) {
        if (key == null || key.isUnbound()) {
            return UNBOUND_KEY_LABEL;
        }
        String label = key.getBoundKeyLocalizedText().getString();
        return label == null || label.isBlank() ? UNBOUND_KEY_LABEL : label;
    }

    /** HUD 纯函数使用的动作顺序固定为给丹、拒绝、拖延。 */
    public record BindingLabels(String giveDan, String refuse, String delay) {
        public BindingLabels {
            giveDan = normalizeLabel(giveDan);
            refuse = normalizeLabel(refuse);
            delay = normalizeLabel(delay);
        }

        private static String normalizeLabel(String label) {
            return label == null || label.isBlank() ? UNBOUND_KEY_LABEL : label;
        }
    }

    /**
     * 给丹核心逻辑：
     * 1. 从 {@link DyingElderEncounterStore} 取大能 entity id；
     * 2. 从 {@link InventoryStateStore} 扫描背包找第一颗 {@code hui_yuan_pill}；
     * 3. 构造并发送 {@code give_dan_to_elder} C2S。
     *
     * <p>背包无丹或遭遇已关闭时静默放弃（server 无论如何会校验，防重复只需 client 端提示）。
     */
    static void handleGiveDan() {
        // getElderEntityId() 返回 MC protocol entity_id（Valence 从 1 起分配）。
        // <= 0 表示未收到 appeared 事件 / elder 尚未同步（0 是 sentinel，Valence 不分配此值）。
        int elderEntityId = DyingElderEncounterStore.getElderEntityId();
        if (elderEntityId <= 0) {
            BongClient.LOGGER.warn("[DyingElder] 给丹失败：elderEntityId={} 无效（大能未出现或 MC protocol id 未同步）", elderEntityId);
            return;
        }

        long pillInstanceId = findHuiYuanPillInstanceId();
        if (pillInstanceId < 0L) {
            BongClient.LOGGER.info("[DyingElder] 给丹失败：背包中无回元丹（hui_yuan_pill）。");
            return;
        }

        BongClient.LOGGER.info(
            "[DyingElder] 发送 give_dan_to_elder: pill_instance_id={} elder_entity_id={}",
            pillInstanceId, elderEntityId
        );
        ClientRequestSender.sendGiveDanToElder(pillInstanceId, elderEntityId);
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
