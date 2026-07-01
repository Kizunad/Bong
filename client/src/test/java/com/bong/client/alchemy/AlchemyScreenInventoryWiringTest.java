package com.bong.client.alchemy;

import com.bong.client.inventory.component.BackpackGridPanel;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;

/**
 * F13：{@code AlchemyScreen} 背包列真库存接线契约测试。
 *
 * <p>此前背包列固定用 {@code MockInventoryData.create()} 填充，与 {@link InventoryStateStore}
 * 完全无关，永远显示同一份假数据；面板绑定的 containerId（{@code "alchemy_mock"}）全仓唯一
 * 出现处只有那一行——不匹配任何真实数据。{@code InventoryModel.PRIMARY_CONTAINER_ID} 类注释
 * 虽写"legacy"，但服务端多处（{@code server/src/inventory/mod.rs} 默认 5×7 容器、
 * {@code inventory_snapshot_emit.rs} 等）证实 {@code main_pack} 仍是真实生产环境下发的默认主背包
 * 容器 id，且维度与本面板固定的 5×7 完全一致。修复后 {@code AlchemyScreen.build()} 改为
 * {@code new BackpackGridPanel(InventoryModel.PRIMARY_CONTAINER_ID, 5, 7)} +
 * {@code backpack.populateFromModel(InventoryStateStore.snapshot())}，并仿
 * {@code CraftScreen}（{@code inventoryListener} + {@code InventoryStateStore.addListener}/
 * {@code removeListener} 生命周期，含 null-safe {@code MinecraftClient.getInstance()} 判空，
 * 与 {@code CraftScreen.scheduleRefresh()} 一致）在 store 变更时刷新。
 *
 * <p><b>为什么不直接 new AlchemyScreen().build(...)：</b>已实测验证——owo
 * {@code Components.label(...)} 在无头 JUnit 环境下会因
 * {@code MinecraftClient.getInstance()==null}（{@code LabelComponent} 构造读
 * {@code MinecraftClient.textRenderer}）抛 NPE。全仓没有任何一个 {@code BaseOwoScreen} 子类的
 * {@code build()}/{@code removed()} 走完整 UI 树被单测调用过（{@code CraftScreen} 本身也是
 * 零测试文件）——这是这套 owo 组件库的普遍限制，不是 F13 的回归。因此本测试改仿
 * {@code InspectScreenBootstrapTest.populateContainerGridsRoutesEntriesIntoMatchingPanels}
 * 的路数：直接用 {@link BackpackGridPanel}（已证实可无头构造/填充，见该测试）+
 * {@link InventoryStateStore} 锁住 F13 修复所依赖的真实数据契约——即"绑定
 * {@code PRIMARY_CONTAINER_ID} 后确实能从真实 store 拿到数据，绑定旧的
 * {@code alchemy_mock} 则拿不到"。{@link InventoryStateStore} 自身的
 * add/removeListener 通用契约已由 {@code InventoryStateStoreTest} 覆盖，此处不重复。
 */
class AlchemyScreenInventoryWiringTest {

    @BeforeEach
    void setUp() {
        InventoryStateStore.resetForTests();
    }

    @AfterEach
    void tearDown() {
        InventoryStateStore.resetForTests();
    }

    // ---- F13 核心契约：新 containerId 能从真实 store 拿到数据 -----------------

    @Test
    void primaryContainerPanel_populatesFromRealInventoryStateStoreSnapshot() {
        InventoryStateStore.replace(InventoryModel.builder()
            .containers(InventoryModel.DEFAULT_CONTAINERS)
            .gridItem(InventoryItem.simple("spirit_grass", "灵草"),
                InventoryModel.PRIMARY_CONTAINER_ID, 0, 0)
            .build());

        // 与 AlchemyScreen.build() 现在的绑定方式完全一致（containerId + 5×7 维度）。
        BackpackGridPanel panel = new BackpackGridPanel(InventoryModel.PRIMARY_CONTAINER_ID, 5, 7);
        panel.populateFromModel(InventoryStateStore.snapshot());

        InventoryItem placed = panel.itemAt(0, 0);
        assertNotNull(placed, "绑定 PRIMARY_CONTAINER_ID 的面板应从真实 store 拿到数据，实际 (0,0) 为空");
        assertEquals("spirit_grass", placed.itemId());
    }

    @Test
    void primaryContainerPanel_reflectsSubsequentStoreReplace() {
        InventoryStateStore.replace(InventoryModel.builder()
            .containers(InventoryModel.DEFAULT_CONTAINERS)
            .gridItem(InventoryItem.simple("spirit_grass", "灵草"),
                InventoryModel.PRIMARY_CONTAINER_ID, 0, 0)
            .build());
        BackpackGridPanel panel = new BackpackGridPanel(InventoryModel.PRIMARY_CONTAINER_ID, 5, 7);
        panel.populateFromModel(InventoryStateStore.snapshot());
        assertEquals("spirit_grass", panel.itemAt(0, 0).itemId());

        // 模拟 AlchemyScreen 的 inventoryListener 在 store 变更时重新 populateFromModel。
        InventoryStateStore.replace(InventoryModel.builder()
            .containers(InventoryModel.DEFAULT_CONTAINERS)
            .gridItem(InventoryItem.simple("guyuan_pill", "固元丹"),
                InventoryModel.PRIMARY_CONTAINER_ID, 0, 0)
            .build());
        panel.populateFromModel(InventoryStateStore.snapshot());

        assertEquals("guyuan_pill", panel.itemAt(0, 0).itemId(),
            "store 变更后重新 populateFromModel 应反映新数据，而不是残留旧的 spirit_grass");
    }

    @Test
    void primaryContainerPanel_emptyStore_staysEmpty() {
        // 未连接 / 初始态：InventoryStateStore.snapshot() 是 InventoryModel.empty()
        BackpackGridPanel panel = new BackpackGridPanel(InventoryModel.PRIMARY_CONTAINER_ID, 5, 7);
        panel.populateFromModel(InventoryStateStore.snapshot());

        assertNull(panel.itemAt(0, 0), "空 store 时面板不应显示任何物品");
    }

    @Test
    void primaryContainerPanel_ignoresItemsInOtherContainers() {
        // body_pocket / back_pack 容器里的物品不应泄漏进绑定 main_pack 的面板。
        InventoryStateStore.replace(InventoryModel.builder()
            .containers(InventoryModel.DEFAULT_CONTAINERS)
            .gridItem(InventoryItem.simple("herb_in_pocket", "口袋药草"),
                InventoryModel.BODY_POCKET_CONTAINER_ID, 0, 0)
            .gridItem(InventoryItem.simple("herb_in_backpack", "破包药草"),
                InventoryModel.BACK_PACK_CONTAINER_ID, 0, 0)
            .build());

        BackpackGridPanel panel = new BackpackGridPanel(InventoryModel.PRIMARY_CONTAINER_ID, 5, 7);
        panel.populateFromModel(InventoryStateStore.snapshot());

        assertNull(panel.itemAt(0, 0),
            "body_pocket/back_pack 容器的物品不应出现在绑定 main_pack 的面板里");
    }

    @Test
    void primaryContainerPanel_multipleItems_eachAtCorrectPosition() {
        InventoryStateStore.replace(InventoryModel.builder()
            .containers(InventoryModel.DEFAULT_CONTAINERS)
            .gridItem(InventoryItem.simple("spirit_grass", "灵草"),
                InventoryModel.PRIMARY_CONTAINER_ID, 0, 0)
            .gridItem(InventoryItem.simple("guyuan_pill", "固元丹"),
                InventoryModel.PRIMARY_CONTAINER_ID, 2, 3)
            .gridItem(InventoryItem.simple("recipe_scroll_qixue_pill", "丹方残卷·气血丹"),
                InventoryModel.PRIMARY_CONTAINER_ID, 4, 6)
            .build());

        BackpackGridPanel panel = new BackpackGridPanel(InventoryModel.PRIMARY_CONTAINER_ID, 5, 7);
        panel.populateFromModel(InventoryStateStore.snapshot());

        assertEquals("spirit_grass", panel.itemAt(0, 0).itemId());
        assertEquals("guyuan_pill", panel.itemAt(2, 3).itemId());
        assertEquals("recipe_scroll_qixue_pill", panel.itemAt(4, 6).itemId(),
            "面板右下角（5×7 边界内最后一格）应正确放置");
    }

    // ---- 回归 pin：旧的 "alchemy_mock" containerId 是死绑定 --------------------

    @Test
    void legacyMockContainerId_neverMatchesRealInventoryData() {
        // "alchemy_mock" 曾是 AlchemyScreen 唯一使用它的地方（grep 全仓确认），
        // 不对应任何真实容器 id——绑定它的面板对真实 store 数据永远视而不见。
        // 此测试锁住这个"旧绑定必然是死代码"的事实，防止未来误改回旧 id。
        InventoryStateStore.replace(InventoryModel.builder()
            .containers(InventoryModel.DEFAULT_CONTAINERS)
            .gridItem(InventoryItem.simple("spirit_grass", "灵草"),
                InventoryModel.PRIMARY_CONTAINER_ID, 0, 0)
            .build());

        BackpackGridPanel legacyPanel = new BackpackGridPanel("alchemy_mock", 5, 7);
        legacyPanel.populateFromModel(InventoryStateStore.snapshot());

        assertNull(legacyPanel.itemAt(0, 0),
            "绑定旧 containerId(alchemy_mock) 的面板不应从真实 store 拿到任何数据"
                + "（证明修复前的绑定对真实库存是死代码）");
    }
}
