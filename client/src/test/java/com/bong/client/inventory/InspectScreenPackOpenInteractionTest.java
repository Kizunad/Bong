package com.bong.client.inventory;

import com.bong.client.inventory.model.InventoryItem;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * fix/tarkov-nest-persistence —— grid / hotbar 双击开包 + 右键开包的纯判定逻辑回归。
 *
 * <p>双击/右键 → openWornContainerPanel 的「打开」决策由两个纯谓词组成：
 * <ul>
 *   <li>{@link InspectScreen#isInstanceDoubleClick}：同一 instance 两连击 ≤ 窗口 → 双击；
 *       不同 instance / 超窗 / 首次（lastInstanceId &lt; 0）均非双击（防误触单击拾取）。</li>
 *   <li>{@link InventoryEquipRules#isContainer}：仅容器件（背包）才拦截开包，非容器件落到
 *       原有 pickup / pill / weapon 菜单路径（不被新拦截吞）。</li>
 * </ul>
 * 这两个谓词共同锁住「双击/右键容器件开包、单击/非容器件不开包」的契约。owo 布局 + mouseClicked
 * 命中-触发（wornContainerPanel 实挂载 / §C4 dispose）headless 无法构造（需 MinecraftClient
 * 初始化布局做 hit-test），与既有 InspectScreenContainerRefreshTest 同口径，由 gradle build +
 * 真机验证兜底。</p>
 */
public class InspectScreenPackOpenInteractionTest {

    private static final String PACK_ID = "worn_grass_pouch"; // CONTAINER_TEMPLATE_IDS 内
    private static final String NON_PACK_ID = "spirit_herb";

    private static InventoryItem item(long instanceId, String itemId) {
        return InventoryItem.createFull(
            instanceId, itemId, itemId, 1, 1, 0.1, "common", "", 1, 1.0, 1.0);
    }

    // ── §C1/§C2 双击计时（instanceId 维度）状态转换 ──

    @Test
    void instanceDoubleClickTriggersWhenSameInstanceWithinWindow() {
        assertTrue(InspectScreen.isInstanceDoubleClick(42L, 1_000L, 42L, 1_100L),
            "同一 instance 间隔 100ms（< 400ms 窗口）应判为双击");
    }

    @Test
    void instanceDoubleClickDoesNotTriggerOutsideWindow() {
        assertFalse(InspectScreen.isInstanceDoubleClick(42L, 1_000L, 42L, 1_500L),
            "同一 instance 但间隔 500ms（> 400ms 窗口）不应判为双击");
    }

    @Test
    void instanceDoubleClickBoundaryAtExactlyWindowStillTriggers() {
        assertTrue(InspectScreen.isInstanceDoubleClick(42L, 1_000L, 42L, 1_400L),
            "恰好 400ms（窗口上界，<=）应判为双击");
        assertFalse(InspectScreen.isInstanceDoubleClick(42L, 1_000L, 42L, 1_401L),
            "401ms（超窗 1ms）不应判为双击");
    }

    @Test
    void instanceDoubleClickDoesNotTriggerForDifferentInstance() {
        assertFalse(InspectScreen.isInstanceDoubleClick(42L, 1_000L, 43L, 1_100L),
            "不同 instance（42→43）即使窗内也不应判为双击，防对不同物品误触开包");
    }

    @Test
    void instanceDoubleClickDoesNotTriggerOnFirstClick() {
        assertFalse(InspectScreen.isInstanceDoubleClick(-1L, 0L, 42L, 50L),
            "首次点击（lastInstanceId<0）不应判为双击 —— 单击仍走 pickup（防误触）");
    }

    // ── §C1/§C2/§C3 开包拦截谓词：仅容器件拦截，非容器件落空（不被吞） ──

    @Test
    void containerItemIsRecognizedForOpenIntercept() {
        assertTrue(InventoryEquipRules.isContainer(item(100L, PACK_ID)),
            "背包件（worn_grass_pouch）应被识别为容器 → 双击/右键拦截开包");
    }

    @Test
    void nonContainerItemFallsThroughOpenIntercept() {
        assertFalse(InventoryEquipRules.isContainer(item(101L, NON_PACK_ID)),
            "非容器件（spirit_herb）不应被识别为容器 → 双击/右键不开包，落到原有 pickup/菜单路径");
    }

    @Test
    void nullItemIsNotContainer() {
        assertFalse(InventoryEquipRules.isContainer(null),
            "空槽（null）不应被当作容器，防 NPE / 误开包");
    }

    // ── 复合：双击容器件 = 开包；单击容器件 / 双击非容器件均不开包 ──

    @Test
    void doubleClickOnContainerOpens_singleClickOrNonContainerDoesNot() {
        InventoryItem pack = item(7L, PACK_ID);
        InventoryItem herb = item(8L, NON_PACK_ID);
        // 双击容器件 → 开包条件成立。
        boolean openOnDoubleContainer =
            InspectScreen.isInstanceDoubleClick(7L, 1_000L, 7L, 1_050L)
                && InventoryEquipRules.isContainer(pack);
        assertTrue(openOnDoubleContainer, "双击同一背包件应满足开包条件");
        // 单击（首次）容器件 → 不开包（仍 pickup）。
        boolean openOnSingleContainer =
            InspectScreen.isInstanceDoubleClick(-1L, 0L, 7L, 1_000L)
                && InventoryEquipRules.isContainer(pack);
        assertFalse(openOnSingleContainer, "首次单击背包件不应开包，应走 pickup");
        // 双击非容器件 → 不开包（走 pill/weapon 菜单或 pickup）。
        boolean openOnDoubleHerb =
            InspectScreen.isInstanceDoubleClick(8L, 1_000L, 8L, 1_050L)
                && InventoryEquipRules.isContainer(herb);
        assertFalse(openOnDoubleHerb, "双击非容器件不应开包");
    }
}
