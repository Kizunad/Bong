package com.bong.client.ui.preview;

import com.bong.client.forge.ForgeWindows;
import com.bong.client.forge.ForgeViewModel;
import com.bong.client.forge.ForgeWindowContent;
import com.bong.client.forge.ForgeWorkbenchComponent;
import com.bong.client.forge.state.ForgeOutcomeStore;
import com.bong.client.forge.state.ForgeStationStore;
import com.bong.client.forge.state.ForgeSessionStore;
import com.bong.client.forge.state.BlueprintScrollStore;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.lifecycle.SessionScopedStoreRegistry;
import com.bong.client.ui.adapter.owo.OwoXmlWindowContentAdapter;
import com.bong.client.ui.intent.UiIntentResult;
import com.bong.client.ui.state.StoreUiStateSource;
import com.bong.client.ui.window.UiWindowManager;
import io.wispforest.owo.ui.container.ScrollContainer;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.text.Text;
import net.minecraft.util.math.BlockPos;
import java.util.List;

/** 原生窗口验收使用生产内容和框架；仅在 preview harness 中替换网络与工位可达性。 */
final class UiForgeWindowPreviewScene implements UiPreviewScene {
    private final BlockPos pos = new BlockPos(2, 64, 0);
    private final UiWindowManager manager = new UiWindowManager(800, 600);
    private ForgeWindows windows;
    private UiWindowManager.WindowState state;
    private OwoXmlWindowContentAdapter adapter;
    private ForgeWindowContent content;
    private Screen previewScreen;
    private int scrollCheckTicks;
    private int scrollStartY;
    private boolean scrolling;

    @Override public void installFixture(UiPreviewConfig config) {
        SessionScopedStoreRegistry.clearAllOnDisconnect();
        scrolling = false;
        scrollCheckTicks = 0;
        ForgeStationStore.replace(new ForgeStationStore.Snapshot(pos, "preview", 2, .86f, "旅人", false));
        BlueprintScrollStore.replace(List.of(new BlueprintScrollStore.Entry("iron_sword_v0", "铁剑", 1, 1,
                "iron_sword", List.of("billet"), List.of(new BlueprintScrollStore.Material("fan_tie", 3))),
            new BlueprintScrollStore.Entry("qing_feng_v0", "青锋剑", 2, 2, "qing_feng_sword",
                List.of("billet", "tempering"), List.of(new BlueprintScrollStore.Material("fan_tie", 3))),
            new BlueprintScrollStore.Entry("ling_feng_v0", "灵锋", 4, 4, "ling_feng_sword",
                List.of("billet", "tempering", "inscription", "consecration"),
                List.of(new BlueprintScrollStore.Material("sui_tie", 3)))), 0);
        InventoryStateStore.replace(InventoryModel.builder()
            .gridItem(InventoryItem.createFull(1, "mineral_fan_tie", "凡铁", 1, 1, .2, "common", "", 12, 1, 0)
                .withMineralId("fan_tie"), 0, 0)
            .gridItem(InventoryItem.createFull(2, "mineral_za_gang", "杂钢", 1, 1, .2, "common", "", 4, 1, 0)
                .withMineralId("za_gang"), 0, 1)
            .build());
        windows = new ForgeWindows(manager, StoreUiStateSource.pullOnOpen(ForgeViewModel::snapshot),
            ignored -> UiIntentResult.accepted(), ignored -> true, System::currentTimeMillis);
    }

    @Override public Screen createScreen() {
        return new Screen(Text.literal("锻造窗口预览")) {
            @Override protected void init() {
                if (content != null) content.close();
                if (adapter != null) adapter.close();
                manager.resizeViewport(width, height);
                state = windows.open(pos, new UiWindowManager.Rect(8, 8,
                    Math.min(640, width - 16), Math.min(390, height - 16)));
                adapter = new OwoXmlWindowContentAdapter(manager, state, () -> {});
                adapter.title("锻造");
                content = new ForgeWindowContent(adapter.content(), windows, state, () -> {});
            }
            @Override public void render(DrawContext context, int x, int y, float delta) {
                context.fill(0, 0, width, height, 0xFF111A23);
                adapter.layout(state.bounds());
                content.layout(adapter.content().width(), adapter.content().height());
                adapter.render(context, x, y, delta);
            }
            @Override public void tick() {
                windows.refresh();
                content.tick();
            }
            @Override public boolean mouseClicked(double x, double y, int button) {
                adapter.mouseDown(x, y, button);
                return true;
            }
            @Override public boolean mouseReleased(double x, double y, int button) {
                windows.endInjection();
                adapter.mouseUp(x, y, button);
                return true;
            }
            @Override public boolean mouseScrolled(double x, double y, double amount) {
                adapter.scroll(x, y, amount);
                return true;
            }
            @Override public boolean mouseDragged(double x, double y, int button, double dx, double dy) {
                adapter.mouseDrag(x, y, button, dx, dy);
                return true;
            }
        };
    }

    @Override public void prepareScreenshot(Screen screen, UiPreviewShot shot) {
        previewScreen = screen;
        if (shot.name().contains("billet")) ForgeSessionStore.replace(new ForgeSessionStore.Snapshot(
            7, "iron_sword_v0", "铁剑", true, "billet", 0, 1, "{}"));
        if (shot.name().contains("tempering")) ForgeSessionStore.replace(new ForgeSessionStore.Snapshot(
            7, "qing_feng_v0", "青锋剑", true, "tempering", 1, 1,
            "{\"pattern_remaining\":[\"L\",\"H\",\"L\",\"F\",\"H\"],\"hits\":5,\"misses\":0}"));
        if (shot.name().contains("consecration")) ForgeSessionStore.replace(new ForgeSessionStore.Snapshot(
            8, "ling_feng_v0", "灵锋剑", true, "consecration", 3, 3,
            "{\"qi_injected\":35,\"qi_required\":80,\"color_imprint\":\"sharp\"}"));
        if (shot.name().contains("inscription")) ForgeSessionStore.replace(new ForgeSessionStore.Snapshot(
            8, "ling_feng_v0", "灵锋", true, "inscription", 2, 3,
            "{\"max_slots\":2,\"filled_slots\":1,\"fail_chance\":0.2}"));
        for (String bucket : List.of("perfect", "good", "flawed", "waste", "explode")) {
            if (!shot.name().endsWith(bucket)) continue;
            ForgeSessionStore.replace(new ForgeSessionStore.Snapshot(9, "ling_feng_v0", "灵锋", false, "done", 3, 4, "{}"));
            ForgeOutcomeStore.replace(new ForgeOutcomeStore.Snapshot(9, "ling_feng_v0", bucket,
                bucket.equals("waste") || bucket.equals("explode") ? null : "ling_feng_sword",
                bucket.equals("perfect") ? 1f : bucket.equals("good") ? .8f : .4f, null, "", 4, false));
        }
        windows.refresh();
        content.tick();
        render(screen);
        var model = adapter.content().childById(ForgeWorkbenchComponent.class, "forge-workbench");
        model.camera().settle();
        model.camera().autoRotate(false);
        if (adapter.content().width() >= 470) {
            var rotate = adapter.content().childById(io.wispforest.owo.ui.component.ButtonComponent.class, "forge-rotate");
            screen.mouseClicked(rotate.x() + 8, rotate.y() + 8, 0);
            screen.mouseReleased(rotate.x() + 8, rotate.y() + 8, 0);
            if (!model.camera().autoRotate()) throw new IllegalStateException("旋转开关未启用自动旋转");
            rotate = adapter.content().childById(io.wispforest.owo.ui.component.ButtonComponent.class, "forge-rotate");
            screen.mouseClicked(rotate.x() + 8, rotate.y() + 8, 0);
            screen.mouseReleased(rotate.x() + 8, rotate.y() + 8, 0);
            if (model.camera().autoRotate()) throw new IllegalStateException("旋转开关未停止自动旋转");
        }
        double modelX = model.x() + model.width() / 2.0;
        double modelY = model.y() + Math.min(60, model.height() / 2.0);
        float zoom = model.camera().zoom();
        screen.mouseScrolled(modelX, modelY, 1);
        if (model.camera().zoom() <= zoom) throw new IllegalStateException("模型滚轮未放大");
        float yaw = model.camera().yaw();
        screen.mouseClicked(modelX, modelY, 0);
        screen.mouseDragged(modelX + 20, modelY + 4, 0, 20, 4);
        screen.mouseReleased(modelX + 20, modelY + 4, 0);
        if (model.camera().yaw() == yaw) throw new IllegalStateException("停止旋转后拖拽未改变模型方向");
        model.camera().reset();
        var scroll = adapter.content().childById(ScrollContainer.class, "forge-scroll");
        scrollStartY = scroll.child().y();
        // 模型区的滚轮用于缩放；从滚动条区域验证内容滚动，避免两种输入语义混淆。
        screen.mouseScrolled(scroll.x() + scroll.width() - 1, scroll.y() + 10, -20);
        scrolling = true;
    }

    @Override public void tick() {
        if (!scrolling || ++scrollCheckTicks < 5) return;
        var scroll = adapter.content().childById(ScrollContainer.class, "forge-scroll");
        if (scroll.child().height() > scroll.height() && scroll.child().y() >= scrollStartY) {
            throw new IllegalStateException("滚轮输入后经过五个 tick，锻造内容仍未移动");
        }
        previewScreen.mouseScrolled(scroll.x() + scroll.width() - 1, scroll.y() + 10, 100);
        scrolling = false;
    }

    private static void render(Screen screen) {
        var client = MinecraftClient.getInstance();
        var draw = new DrawContext(client, client.getBufferBuilders().getEntityVertexConsumers());
        screen.render(draw, -1, -1, 0);
        draw.draw();
    }

    @Override public void validateGeometry(Screen screen, UiPreviewShot shot) {
        var bounds = state.bounds();
        if (bounds.x() < 0 || bounds.y() < 0 || bounds.x() + bounds.width() > screen.width
            || bounds.y() + bounds.height() > screen.height) throw new IllegalStateException("锻造窗口越界");
        var scroll = adapter.content().childById(ScrollContainer.class, "forge-scroll");
        if (scroll.width() <= 0 || scroll.height() <= 0 || scroll.child().height() <= 0) {
            throw new IllegalStateException("锻造内容区域为空");
        }
        var model = adapter.content().childById(ForgeWorkbenchComponent.class, "forge-workbench");
        if (model == null || model.width() < 1 || model.height() < 1 || model.failure() != null) {
            throw new IllegalStateException("锻造装备模型没有成功显示：" + (model == null ? "组件缺失" : model.failure()));
        }
    }
    @Override public String selectedTemplateId(Screen screen) { return "forge-window"; }
    @Override public boolean isReady(Screen screen) { return adapter != null; }
    @Override public boolean initializationFailed(Screen screen) { return false; }
    @Override public void cleanup() {
        if (content != null) content.close();
        if (adapter != null) adapter.close();
        adapter = null;
        manager.reset();
        SessionScopedStoreRegistry.clearAllOnDisconnect();
    }
}
