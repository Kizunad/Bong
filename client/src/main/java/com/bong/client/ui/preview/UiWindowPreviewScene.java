package com.bong.client.ui.preview;

import com.bong.client.inventory.InspectScreen;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.ui.window.UiWindowRuntime;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import java.util.List;

/** 由既有环境变量门禁启用的首窗场景，走生产 XML、输入路由与 manager。 */
final class UiWindowPreviewScene implements UiPreviewScene {
    private final String modelItem;
    private long modelInstanceId;
    private long companionInstanceId;

    UiWindowPreviewScene() { this("pickaxe_iron"); }

    UiWindowPreviewScene(String modelItem) {
        this.modelItem = modelItem;
    }

    @Override
    public void installFixture(UiPreviewConfig config) {
        UiWindowRuntime.manager().reset();
        InventoryItem item = config.items().get(modelItem);
        InventoryItem companion = config.items().get("herb_bundle");
        if (item == null || companion == null) {
            throw new IllegalArgumentException("预览缺少物品数据，请先运行 scripts/export-item-preview.py");
        }
        modelInstanceId = item.instanceId();
        companionInstanceId = companion.instanceId();
        // 预览要容纳真实 TOML 尺寸（例如腿甲 2×3），不能依赖生产默认的 2×3 body_pocket。
        var previewContainer = new InventoryModel.ContainerDef(
            InventoryModel.BODY_POCKET_CONTAINER_ID, "贴身口袋", 3, 4);
        int companionColumn = item.gridWidth();
        if (item.gridWidth() + companion.gridWidth() > previewContainer.cols()
            || Math.max(item.gridHeight(), companion.gridHeight()) > previewContainer.rows()) {
            throw new IllegalStateException("预览容器无法容纳物品尺寸: "
                + item.itemId() + " + " + companion.itemId());
        }
        InventoryStateStore.replace(InventoryModel.builder()
            .containers(List.of(previewContainer))
            .gridItem(item, 0, 0)
            .gridItem(companion, 0, companionColumn).build());
    }

    @Override
    public Screen createScreen() {
        UiWindowRuntime.openItem(modelInstanceId);
        UiWindowRuntime.openItem(companionInstanceId);
        return new InspectScreen(InventoryStateStore.snapshot());
    }

    @Override public String selectedTemplateId(Screen screen) { return "item-inspect"; }
    @Override public boolean isReady(Screen screen) { return ((InspectScreen) screen).windowHostReadyForPreview(); }
    @Override public boolean initializationFailed(Screen screen) { return ((InspectScreen) screen).windowHostFailedForPreview(); }

    @Override
    public void prepareScreenshot(Screen screen, UiPreviewShot shot) {
        var manager = UiWindowRuntime.manager();
        var context = new DrawContext(MinecraftClient.getInstance(),
            MinecraftClient.getInstance().getBufferBuilders().getEntityVertexConsumers());
        // 真实格子输入回归：关闭全部窗口后悬停不应改变布局，双击不能发送库存请求。
        manager.reset();
        screen.render(context, -1, -1, 0);
        var slot = ((InspectScreen) screen).itemSlotForPreview(modelInstanceId);
        if (slot == null) throw new IllegalStateException("预览物品格子未挂载");
        int slotX = slot.x(), slotY = slot.y();
        for (int frame = 0; frame < 12; frame++) screen.render(context, slotX + 8, slotY + 8, 0);
        if (slot.x() != slotX || slot.y() != slotY) throw new IllegalStateException("悬停物品仍触发布局抖动");
        var requests = new java.util.ArrayList<String>();
        com.bong.client.network.ClientRequestSender.setBackendForTests((channel, bytes) -> requests.add(channel.toString()));
        try {
            for (int click = 0; click < 2; click++) {
                screen.mouseClicked(slotX + 8, slotY + 8, 0);
                screen.mouseReleased(slotX + 8, slotY + 8, 0);
            }
            if (manager.snapshot().size() != 1 || !requests.isEmpty()) {
                throw new IllegalStateException("双击详情未打开，或查看物品产生了库存操作: " + requests);
            }
            screen.render(context, -1, -1, 0);
            var opened = manager.snapshot().get(0);
            screen.mouseClicked(opened.bounds().x() + opened.bounds().width() - 10, opened.bounds().y() + 10, 0);
            screen.mouseReleased(-1, -1, 0);
            if (!manager.snapshot().isEmpty()) throw new IllegalStateException("详情关闭按钮没有响应");
            for (int click = 0; click < 2; click++) {
                screen.mouseClicked(slotX + 8, slotY + 8, 0);
                screen.mouseReleased(slotX + 8, slotY + 8, 0);
            }
            if (manager.snapshot().size() != 1 || !requests.isEmpty()) {
                throw new IllegalStateException("关闭后双击未重新打开详情");
            }
            manager.reset();
            screen.mouseClicked(slotX + 8, slotY + 8, 0);
            screen.mouseDragged(-30, -30, 0, -38 - slotX, -38 - slotY);
            screen.mouseReleased(-30, -30, 0);
            if (((InspectScreen) screen).itemSlotForPreview(modelInstanceId) == null || !requests.isEmpty()) {
                throw new IllegalStateException("取消拖放后物品未回源，或错误发送了库存请求");
            }
        } finally {
            com.bong.client.network.ClientRequestSender.resetBackendForTests();
        }
        manager.reset();
        UiWindowRuntime.openItem(modelInstanceId);
        UiWindowRuntime.openItem(companionInstanceId);
        var first = manager.snapshot().get(0);
        UiWindowRuntime.openItem(Long.parseLong(first.key().identity()));
        if (manager.snapshot().size() != 2 || manager.snapshot().get(1) != first) {
            throw new IllegalStateException("重复打开未复用并置顶既有窗口");
        }
        screen.render(context, -1, -1, 0);
        double x = first.bounds().x() + 8;
        double y = first.bounds().y() + 8;
        screen.mouseClicked(x, y, 0);
        screen.mouseDragged(-40, -40, 0, -40 - x, -40 - y);
        screen.mouseReleased(-40, -40, 0);
        if (first.bounds().x() != 0 || first.bounds().y() != 0 || manager.capturedKey() != null) {
            throw new IllegalStateException("真实输入未驱动窗口拖动、边界约束或释放");
        }
        for (var state : manager.snapshot()) {
            var bounds = state.bounds();
            if (bounds.x() < 0 || bounds.y() < 0 || bounds.x() + bounds.width() > shot.expectedLogicalWidth()
                || bounds.y() + bounds.height() > shot.expectedLogicalHeight()) {
                throw new IllegalStateException("物品窗口越出逻辑 viewport: " + bounds);
            }
        }
        screen.render(context, -1, -1, 0);
        // 关闭必须在非零窗口坐标下命中，覆盖 Screen 到独立 adapter 的坐标转换。
        screen.mouseClicked(8, 8, 0);
        screen.mouseDragged(26, 26, 0, 18, 18);
        screen.mouseReleased(26, 26, 0);
        screen.render(context, -1, -1, 0);
        screen.mouseClicked(first.bounds().x() + first.bounds().width() - 10, first.bounds().y() + 10, 0);
        screen.mouseReleased(-1, -1, 0);
        if (!first.scope().isClosed() || manager.snapshot().size() != 1) {
            throw new IllegalStateException("XML 关闭按钮没有关闭业务窗口");
        }
        UiWindowRuntime.openItem(Long.parseLong(first.key().identity()));
        screen.render(context, -1, -1, 0);
        var reopened = manager.snapshot().get(1).bounds();
        screen.mouseClicked(reopened.x() + 8, reopened.y() + 8, 0);
        screen.mouseDragged(reopened.x() - 10, reopened.y() + 8, 0, -18, 0);
        screen.mouseReleased(reopened.x() - 10, reopened.y() + 8, 0);
        screen.render(context, -1, -1, 0);
        reopened = manager.snapshot().get(1).bounds();
        screen.mouseClicked(reopened.x() + reopened.width() - 35, reopened.y() + 41, 0);
        screen.mouseReleased(-1, -1, 0);
        context.draw();
    }

    @Override public void validateGeometry(Screen screen, UiPreviewShot shot) {}

    @Override
    public void cleanup() {
        UiWindowRuntime.manager().reset();
        InventoryStateStore.replace(InventoryModel.empty());
    }
}
