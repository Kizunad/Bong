package com.bong.client.ui.preview;

import com.bong.client.alchemy.AlchemyFurnaceComponent;
import com.bong.client.alchemy.AlchemyFurnaceInteractIntentHandler;
import com.bong.client.alchemy.AlchemyScreenBootstrap;
import com.bong.client.alchemy.AlchemyWindows;
import com.bong.client.alchemy.state.AlchemyFurnaceStore;
import com.bong.client.alchemy.state.AlchemySessionStore;
import com.bong.client.entity.BongEntityModelKind;
import com.bong.client.entity.BongModeledEntity;
import com.bong.client.input.InteractKeyRouter;
import com.bong.client.inventory.InspectScreen;
import com.bong.client.inventory.InventoryContainerWindows;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.ui.ScreenTransitionController;
import com.bong.client.ui.window.UiWindowManager;
import com.bong.client.ui.window.UiWindowRuntime;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.util.Hand;
import net.minecraft.util.hit.EntityHitResult;

/** 在本地 OP 测试服放置真炉，经生产交互路由开窗并等待服务端炉讯。 */
final class UiAlchemyRuntimePreviewScene implements UiPreviewScene {
    private boolean requestedScene;
    private BongModeledEntity furnace;
    private InventoryModel beforeInventory;
    private AlchemyFurnaceStore.Snapshot beforeFurnace;
    private AlchemySessionStore.Snapshot beforeSession;
    private UiWindowManager.WindowState window;

    @Override
    public boolean clientReady(MinecraftClient client) {
        if (client.world == null || client.player == null || client.getNetworkHandler() == null
            || client.currentScreen != null || ScreenTransitionController.activeTransition() != null
            || client.getNetworkHandler().getCommandDispatcher().getRoot().getChild("ping") == null) {
            return false;
        }
        if (!requestedScene) {
            require(client.getNetworkHandler().getCommandDispatcher().getRoot().getChild("scene") != null,
                "正式炼丹入口验收需要本地 OP 账号的 /scene 权限");
            beforeInventory = InventoryStateStore.snapshot();
            client.getNetworkHandler().sendChatCommand("scene test_alchemy_furnace_1");
            requestedScene = true;
            return false;
        }
        // 前一个夹具会清空背包；必须收到 /scene 发材料后的真实库存，不能拿旧炉实体抢跑。
        if (!InventoryStateStore.isAuthoritativeLoaded() || InventoryStateStore.snapshot() == beforeInventory) {
            return false;
        }
        for (var entity : client.world.getEntities()) {
            if (entity instanceof BongModeledEntity modeled
                && modeled.modelKind() == BongEntityModelKind.ALCHEMY_FURNACE
                && !modeled.isRemoved() && AlchemyScreenBootstrap.available(client, modeled.getBlockPos())) {
                furnace = modeled;
                return true;
            }
        }
        return false;
    }

    @Override
    public void installFixture(UiPreviewConfig config) {
        UiWindowRuntime.beginPreview();
        beforeFurnace = AlchemyFurnaceStore.snapshot();
        beforeSession = AlchemySessionStore.snapshot();
    }

    @Override
    public Screen createScreen() {
        var client = MinecraftClient.getInstance();
        // 真实 mixin 路径必须允许右键观察目标，但不能再打开炼丹窗口。
        client.interactionManager.interactEntity(client.player, furnace, Hand.MAIN_HAND);
        require(client.currentScreen == null && ScreenTransitionController.pendingScreen() == null,
            "右键丹炉仍然打开了窗口");

        var previousTarget = client.crosshairTarget;
        try {
            client.crosshairTarget = new EntityHitResult(furnace);
            var handler = new AlchemyFurnaceInteractIntentHandler();
            var candidate = handler.candidate(client).orElseThrow(
                () -> new IllegalStateException("真实丹炉没有进入统一交互候选"));
            client.crosshairTarget = null;
            require(!handler.dispatch(client, candidate), "准星离开丹炉后仍派发过期交互");
            client.crosshairTarget = new EntityHitResult(furnace);
            require(InteractKeyRouter.global().route(client), "G 键路由未打开真实丹炉");
        } finally {
            client.crosshairTarget = previousTarget;
        }
        var screen = ScreenTransitionController.pendingScreen();
        if (screen == null) screen = client.currentScreen;
        require(screen instanceof InspectScreen, "炼丹未通过生产入口打开窗口宿主");
        window = UiWindowRuntime.manager().snapshot().stream()
            .filter(state -> state.definition().equals(AlchemyWindows.DEFINITION) && !state.closed())
            .findFirst().orElseThrow(() -> new IllegalStateException("正式炼丹窗口未创建"));
        return screen;
    }

    @Override
    public String selectedTemplateId(Screen screen) {
        return window.definition().templateId();
    }

    @Override
    public boolean isReady(Screen screen) {
        return ((InspectScreen) screen).windowHostReadyForPreview()
            && AlchemyFurnaceStore.snapshot() != beforeFurnace
            && AlchemySessionStore.snapshot() != beforeSession
            && furnace.getBlockPos().equals(AlchemyFurnaceStore.snapshot().pos());
    }

    @Override
    public boolean initializationFailed(Screen screen) {
        return ((InspectScreen) screen).windowHostFailedForPreview();
    }

    @Override
    public void prepareScreenshot(Screen screen, UiPreviewShot shot) {
        if (shot.name().endsWith("-100")) {
            var manager = UiWindowRuntime.manager();
            for (var other : manager.snapshot()) {
                if (other != window) manager.close(other.key());
            }
            manager.settleAt(window.key(), new UiWindowManager.Rect(12, 24, 100, 100));
        }
        if (!shot.name().contains("op-backpack")) return;
        // 此处读取真实服务端库存，不能通过客户端 fixture 伪造 OP 身份或扩容。
        require(InventoryStateStore.isAuthoritativeLoaded(), "OP 验收尚未收到服务端库存");
        var pocket = InventoryStateStore.snapshot().containers().stream()
            .filter(container -> container.id().equals(InventoryModel.BODY_POCKET_CONTAINER_ID))
            .findFirst().orElseThrow(() -> new IllegalStateException("OP 验收缺少随身容器"));
        require(pocket.rows() == 12 && pocket.cols() == 12 && pocket.name().equals("OP 背包"),
            "真实 OP 账号未获得扩容快照");
        var manager = UiWindowRuntime.manager();
        var key = manager.key(InventoryContainerWindows.DEFINITION.windowType(), pocket.id());
        for (var other : manager.snapshot()) {
            if (other != window && !other.key().equals(key)) manager.close(other.key());
        }
        UiWindowRuntime.openContainer(pocket.id());
        manager.settleAt(window.key(), new UiWindowManager.Rect(12, 24, 540, 390));
        manager.settleAt(key, new UiWindowManager.Rect(shot.expectedLogicalWidth() - 376, 24, 364, 424));
    }

    @Override
    public void validateGeometry(Screen screen, UiPreviewShot shot) {
        var art = UiWindowRuntime.windowContentForPreview(window.key())
            .childById(AlchemyFurnaceComponent.class, "alchemy-furnace-art");
        require(art != null && art.width() > 0 && art.height() > 0,
            "生产模板未挂载炼丹工位，或工位没有可见尺寸");
        require(art.failure() == null, "正式炼丹窗口的模型加载失败");
        if (shot.name().endsWith("-100")) {
            require(window.bounds().width() == 100 && window.bounds().height() == 100,
                "正式工位没有缩小到 100×100");
            var canvas = art.canvasBounds();
            require(canvas.width() >= 60 && canvas.height() >= 40, "正式小工位的炉景被控件挤占");
            double x = canvas.x() + canvas.width() * .5;
            double y = canvas.y() + canvas.height() * .55;
            UiWindowRuntime.mouseDown(x, y, 0);
            try {
                require(window.key().equals(UiWindowRuntime.manager().capturedKey()),
                    "正式小工位的鼠标输入被工作台工具栏遮挡");
            } finally {
                UiWindowRuntime.mouseUp(x, y, 0);
            }
        }
        require(!InteractKeyRouter.global().route(MinecraftClient.getInstance()),
            "窗口打开时 G 键路由仍继续派发世界交互");
    }

    @Override
    public void cleanup() {
        UiWindowRuntime.manager().reset();
        UiWindowRuntime.endPreview();
        requestedScene = false;
        furnace = null;
        beforeInventory = null;
        window = null;
    }

    private static void require(boolean condition, String message) {
        if (!condition) throw new IllegalStateException(message);
    }
}
