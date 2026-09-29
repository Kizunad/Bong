package com.bong.client.ui.preview;

import com.bong.client.alchemy.AlchemyFurnaceComponent;
import com.bong.client.alchemy.AlchemyIntent;
import com.bong.client.alchemy.AlchemyIncenseTimer;
import com.bong.client.alchemy.AlchemyNotesContent;
import com.bong.client.alchemy.AlchemyUiStateSource;
import com.bong.client.alchemy.AlchemyWindowContent;
import com.bong.client.alchemy.AlchemyWindows;
import com.bong.client.alchemy.AlchemyWorkspaceLayout;
import com.bong.client.alchemy.state.AlchemyAttemptHistoryStore;
import com.bong.client.alchemy.state.AlchemyFurnaceStore;
import com.bong.client.alchemy.state.AlchemySessionStore;
import com.bong.client.alchemy.state.RecipeScrollStore;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.lifecycle.SessionScopedStoreRegistry;
import com.bong.client.ui.adapter.owo.OwoXmlWindowContentAdapter;
import com.bong.client.ui.intent.UiIntentResult;
import com.bong.client.ui.window.UiWindowManager;
import io.wispforest.owo.ui.component.ButtonComponent;
import io.wispforest.owo.ui.container.ScrollContainer;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.util.ScreenshotRecorder;
import net.minecraft.text.Text;
import net.minecraft.util.math.BlockPos;
import org.lwjgl.glfw.GLFW;

import java.util.ArrayList;
import java.util.List;
import java.nio.file.Files;
import java.nio.file.Path;
import java.io.IOException;
import java.io.UncheckedIOException;

/** 显式预览夹具：使用生产工位、独立丹方窗口及真实 owo 输入，不发送联网请求。 */
final class UiAlchemyWindowPreviewScene implements UiPreviewScene {
    private static final BlockPos POSITION = new BlockPos(1, 64, 0);
    private final UiWindowManager manager = new UiWindowManager(800, 600);
    private final List<AlchemyIntent> requests = new ArrayList<>();
    private AlchemyWindows windows;
    private UiWindowManager.WindowState state;
    private UiWindowManager.WindowState notesState;
    private OwoXmlWindowContentAdapter adapter;
    private OwoXmlWindowContentAdapter notesAdapter;
    private AlchemyWindowContent content;
    private AlchemyNotesContent notes;
    private boolean screenshotHover;
    private double hoverX = .525;
    private double hoverY = .55;
    private UiWindowManager.WindowState focused;
    private OwoXmlWindowContentAdapter captured;
    private boolean draggingMaterial;
    private boolean tinyWindow;
    private Path outputDirectory;
    private Path animationFrames;
    private int frameTick;

    @Override public boolean clientReady(MinecraftClient client) {
        return client.world != null && client.player != null && client.getNetworkHandler() != null
            && client.currentScreen == null
            && com.bong.client.ui.ScreenTransitionController.activeTransition() == null
            && client.getNetworkHandler().getCommandDispatcher().getRoot().getChild("ping") != null;
    }

    @Override public void installFixture(UiPreviewConfig config) {
        outputDirectory = Path.of(config.outputDir());
        animationFrames = null;
        tinyWindow = false;
        manager.reset();
        requests.clear();
        SessionScopedStoreRegistry.clearAllOnDisconnect();
        AlchemyFurnaceStore.replace(new AlchemyFurnaceStore.Snapshot(POSITION, 2, 88, 100, "旅人", false));
        RecipeScrollStore.replace(new RecipeScrollStore.Snapshot(List.of(new RecipeScrollStore.RecipeEntry(
            "ling_xi_wan_v1", "灵息丸", "取灵草三株，净去枯叶。\n\n一阶药炉即可炼制。药材齐备再入炉，先以三成温火养之。\n\n守火三成，容差一成半。\n\n徐徐注入真元五份，候八十刻，约四秒。\n\n药成可收，丸中仍留余毒，不宜接连吞服。",
            "佚名", "残卷", 8)), 0));
        windows = new AlchemyWindows(manager, AlchemyUiStateSource.production(), intent -> {
            requests.add(intent);
            return UiIntentResult.accepted();
        }, Runnable::run, ignored -> true);
    }

    private void openNotes() {
        var viewport = MinecraftClient.getInstance().getWindow();
        if (AlchemyWorkspaceLayout.canReadBesideFurnace(viewport.getScaledWidth())) {
            manager.settleAt(state.key(), AlchemyWorkspaceLayout.furnace(viewport.getScaledWidth(), viewport.getScaledHeight(), true));
        }
        if (notesState != null && notesState.closed() && notesAdapter != null) {
            notesAdapter.close();
            notesAdapter = null;
        }
        var bounds = AlchemyWorkspaceLayout.notes(viewport.getScaledWidth(), viewport.getScaledHeight());
        if (tinyWindow) bounds = new UiWindowManager.Rect(bounds.x(), bounds.y(), 100, 100);
        notesState = manager.openOrFocus(AlchemyNotesContent.DEFINITION,
            manager.key("alchemy-notes", "book"), bounds);
        if (notesAdapter == null) {
            notesAdapter = new OwoXmlWindowContentAdapter(manager, notesState, () -> {});
            notesAdapter.paperFrame();
            notesAdapter.title("丹方");
            notes = new AlchemyNotesContent(notesAdapter.content(), windows);
        }
        focus(notesState);
    }

    private static boolean visible(UiWindowManager.WindowState window) {
        return window != null && !window.closed() && !window.minimized();
    }

    private OwoXmlWindowContentAdapter adapterFor(UiWindowManager.WindowState window) {
        return window == state ? adapter : window == notesState ? notesAdapter : null;
    }

    private void focus(UiWindowManager.WindowState window) {
        if (focused != window) {
            var previous = adapterFor(focused);
            if (previous != null) previous.cancelInput();
            content.cancelInput();
        }
        focused = window;
        if (window != null) manager.openOrFocus(window.definition(), window.key(), window.bounds());
    }

    private OwoXmlWindowContentAdapter focusedAdapter() {
        if (!visible(focused)) focus(null);
        return adapterFor(focused);
    }

    @Override public Screen createScreen() {
        return new Screen(Text.literal("炼丹工位预览")) {
            @Override protected void init() {
                var oldState = state;
                var oldAdapter = adapter;
                var oldContent = content;
                UiWindowManager.WindowState nextState = null;
                OwoXmlWindowContentAdapter nextAdapter = null;
                AlchemyWindowContent nextContent = null;
                try {
                    manager.resizeViewport(width, height);
                    nextState = windows.open(POSITION, AlchemyWorkspaceLayout.furnace(width, height, false));
                    nextAdapter = new OwoXmlWindowContentAdapter(manager, nextState, () -> {});
                    nextAdapter.title("炼丹");
                    var candidateAdapter = nextAdapter;
                    nextContent = new AlchemyWindowContent(candidateAdapter.content(), windows, nextState,
                        UiAlchemyWindowPreviewScene.this::openNotes, () -> {
                            openNotes();
                            notes.showHistory();
                            notesAdapter.title("炉记");
                        });
                    state = nextState;
                    adapter = nextAdapter;
                    content = nextContent;
                    focus(state);
                    if (oldContent != null) oldContent.close();
                    if (oldAdapter != null) oldAdapter.close();
                } catch (Throwable failure) {
                    if (nextContent != null) {
                        try {
                            nextContent.close();
                        } catch (Throwable cleanupFailure) {
                            if (cleanupFailure != failure) failure.addSuppressed(cleanupFailure);
                        }
                    }
                    if (nextAdapter != null) {
                        try {
                            nextAdapter.close();
                        } catch (Throwable cleanupFailure) {
                            if (cleanupFailure != failure) failure.addSuppressed(cleanupFailure);
                        }
                    }
                    state = oldState;
                    adapter = oldAdapter;
                    content = oldContent;
                    if (oldState == null && nextState != null) {
                        try {
                            manager.close(nextState.key());
                        } catch (Throwable cleanupFailure) {
                            if (cleanupFailure != failure) failure.addSuppressed(cleanupFailure);
                        }
                    }
                    throwUnchecked(failure);
                }
            }

            @Override public void render(DrawContext context, int x, int y, float delta) {
                context.fill(0, 0, width, height, 0xFF111A1B);
                focusedAdapter();
                if (visible(state)) adapter.layout(state.bounds());
                var furnace = furnace();
                int mx = screenshotHover ? artX(hoverX) : x;
                int my = screenshotHover ? artY(hoverY) : y;
                var top = manager.hitTest(mx, my);
                int depth = 0;
                for (var window : manager.snapshot()) {
                    if (!visible(window)) continue;
                    var target = adapterFor(window);
                    target.layout(window.bounds());
                    if (window == notesState) notes.layout(target.content().width(), target.content().height());
                    if (window == state) content.draggingMaterial(draggingMaterial);
                    context.getMatrices().push();
                    try {
                        context.getMatrices().translate(0, 0, depth);
                        target.render(context, top == window ? mx : -1, top == window ? my : -1, delta);
                        context.draw();
                    } finally {
                        context.getMatrices().pop();
                    }
                    depth += 400;
                }
            }

            @Override public void tick() {
                windows.refresh();
                content.tick();
                if (notes != null) notes.tick();
            }

            @Override public boolean mouseClicked(double x, double y, int button) {
                var targetState = manager.hitTest(x, y);
                focus(targetState);
                var target = adapterFor(targetState);
                if (target == null) return false;
                captured = target;
                if (button == 0 && target.headerAt(x, y)) manager.beginDrag(targetState.key(), x, y);
                else target.mouseDown(x, y, button);
                return true;
            }
            @Override public boolean mouseDragged(double x, double y, int button, double dx, double dy) {
                if (!manager.dragTo(x, y) && captured != null) captured.mouseDrag(x, y, button, dx, dy);
                return true;
            }
            @Override public boolean mouseReleased(double x, double y, int button) {
                manager.endDrag();
                if (captured != null) captured.mouseUp(x, y, button);
                captured = null;
                return true;
            }
            @Override public boolean mouseScrolled(double x, double y, double amount) {
                var target = adapterFor(manager.hitTest(x, y));
                if (target == null) return false;
                target.scroll(x, y, amount);
                return true;
            }
            @Override public boolean charTyped(char character, int modifiers) {
                var target = focusedAdapter();
                return target != null && target.charTyped(character, modifiers);
            }
            @Override public boolean keyPressed(int key, int scan, int mods) {
                var target = focusedAdapter();
                if (target == null) return super.keyPressed(key, scan, mods);
                if (target == adapter && !target.textFocused() && content.keyPressed(key, mods)) return true;
                return target.keyPressed(key, scan, mods) || super.keyPressed(key, scan, mods);
            }
            @Override public boolean keyReleased(int key, int scan, int mods) {
                content.keyReleased(key);
                return super.keyReleased(key, scan, mods);
            }
        };
    }

    @Override public void prepareScreenshot(Screen screen, UiPreviewShot shot) {
        // 100×100 指业务面板，原生窗口使用可用尺寸，避免 Windows 窗框的最小宽度干扰验收。
        tinyWindow = shot.name().contains("-100");
        if (shot.name().startsWith("result-") || shot.name().equals("alchemy-hover-brewing") || shot.name().contains("feed")) {
            animationFrames = outputDirectory.resolve("frames-" + shot.name());
            frameTick = 0;
        }
        screenshotHover = false;
        hoverX = .525;
        hoverY = .55;
        if (shot.name().contains("hover-fire")) {
            hoverX = .54;
            hoverY = .79;
        } else if (shot.name().contains("hover-incense")) {
            hoverX = .18;
            hoverY = .90;
        }
        if (shot.name().contains("hover") || shot.name().contains("feed")) {
            AlchemyFurnaceStore.replace(new AlchemyFurnaceStore.Snapshot(POSITION, 2, 88, 100, "旅人", true));
            AlchemySessionStore.replace(new AlchemySessionStore.Snapshot("ling_xi_wan_v1", true, 32, 80,
                .3f, .3f, .15f, 5, 5, "炼制中", List.of(
                new AlchemySessionStore.StageHint(0, 0, "灵草 × 3", true, false,
                    List.of(new AlchemySessionStore.IngredientHint("spirit_grass", 3, 3)))), List.of(),
                new AlchemyIncenseTimer.Snapshot(AlchemyIncenseTimer.State.BURNING,
                    58_000, 60, "incense_plain", 1, 1, "#B8B4A7")));
        }
        windows.refresh();
        content.tick();
        render(screen, -1, -1);
        verifyResize(screen);
        if (tinyWindow) {
            manager.resize(state.key(), "100", "100");
            render(screen, -1, -1);
        }
        if (shot.name().startsWith("result-")) prepareResult(shot.name().substring(7));
        if (shot.name().contains("feed")) verifyFeed(screen);
        if (shot.name().contains("controls")) verifyKeyboard(screen);
        if (shot.name().contains("journal")) {
            for (String bucket : List.of("good", "waste", "explode", "perfect")) {
                AlchemyAttemptHistoryStore.append(new AlchemyAttemptHistoryStore.Entry(
                    bucket, "ling_xi_wan_v1", "ling_xi_wan", "", "", false));
            }
            windows.refresh();
            openNotes();
            notes.showHistory();
            notesAdapter.title("炉记");
            render(screen, -1, -1);
            verifyNotesReading(screen);
        } else if (shot.name().contains("notes")) {
            screen.mouseClicked(artX(.16), artY(.44), 0);
            if (notesAdapter == null) throw new IllegalStateException("左桌未打开独立丹方窗口");
            render(screen, -1, -1);
            verifyNotesLifecycle(screen);
        } else if (shot.name().contains("error")) {
            windows.acceptMessage("这一味尚缺两份，炉口容不下更多了。");
        } else if (shot.name().contains("hover")) {
            screenshotHover = true;
        }
    }

    @Override public void tick() {
        if (animationFrames == null || ++frameTick > 40 || frameTick % 2 != 0) return;
        try {
            Files.createDirectories(animationFrames);
            try (var frame = ScreenshotRecorder.takeScreenshot(MinecraftClient.getInstance().getFramebuffer())) {
                frame.writeTo(animationFrames.resolve(String.format("frame-%02d.png", frameTick)));
            }
        } catch (IOException failure) {
            throw new UncheckedIOException("无法保存炼丹动画帧", failure);
        }
    }

    private void prepareResult(String bucket) {
        boolean early = bucket.equals("early");
        AlchemyFurnaceStore.replace(new AlchemyFurnaceStore.Snapshot(POSITION, 2, 88, 100, "旅人", true));
        AlchemySessionStore.replace(new AlchemySessionStore.Snapshot("ling_xi_wan_v1", early, early ? 32 : 80, 80,
            .3f, .3f, .15f, 5, 5, early ? "炼制中" : "待收取", List.of(), List.of()));
        windows.refresh();
        if (windows.settle().kind() != UiIntentResult.Kind.LOCAL_ACCEPTED) {
            throw new IllegalStateException("结果动画预览未通过生产收取入口");
        }
        AlchemyAttemptHistoryStore.append(new AlchemyAttemptHistoryStore.Entry(
            early ? "waste" : bucket, "ling_xi_wan_v1", "", "", "", false));
        AlchemyFurnaceStore.replace(new AlchemyFurnaceStore.Snapshot(POSITION, 2, 88, 100, "旅人", false));
        AlchemySessionStore.replace(new AlchemySessionStore.Snapshot("ling_xi_wan_v1", false, early ? 32 : 80, 80,
            .3f, .3f, .15f, 5, 5, "已收取", List.of(), List.of()));
        windows.refresh();
        content.tick();
        if (windows.confirmedResult() == null) throw new IllegalStateException("权威结果夹具未触发结算动画");
    }

    private void verifyResize(Screen screen) {
        var bounds = state.bounds();
        manager.resize(state.key(), "100", "100");
        render(screen, -1, -1);
        var furnace = furnace();
        var canvas = furnace.canvasBounds();
        if (state.bounds().width() != 100 || state.bounds().height() != 100
            || canvas.width() < 60 || canvas.height() < 40) {
            throw new IllegalStateException("100×100 工位被固定留白或快捷键挤占");
        }
        if (!adapter.headerAt(state.bounds().x() + 8, state.bounds().y() + 10)
            || adapter.headerAt(state.bounds().x() + 90, state.bounds().y() + 10)) {
            throw new IllegalStateException("100×100 标题拖动区与关闭入口重叠");
        }
        manager.resize(state.key(), Integer.toString(bounds.width()), Integer.toString(bounds.height()));
        render(screen, -1, -1);
    }

    private void verifyFeed(Screen screen) {
        var herb = InventoryItem.createFull(31, "spirit_grass", "灵草", 1, 1, .2, "common", "", 2, 1, 0);
        feedSnapshot(0);
        InventoryStateStore.replace(InventoryModel.builder().gridItem(herb, 0, 0).build());
        windows.refresh();
        content.tick();
        var furnace = furnace();
        int cx = artX(.55);
        int cy = artY(.29);
        render(screen, artX(.56), artY(.20));
        // 浮窗布局随部位避让，不再要求它在窄窗中必须挡住炉口。
        draggingMaterial = true;
        render(screen, cx, cy);
        screen.keyPressed(GLFW.GLFW_KEY_F, 0, 0);
        if (!requests.isEmpty()) throw new IllegalStateException("拖料途中触发了注元");
        if (content.drop(furnace.x() + 2, cy, herb)) throw new IllegalStateException("工位边缘错误接收材料");
        if (!content.drop(cx, cy, herb)
            || requests.stream().noneMatch(intent -> intent instanceof AlchemyIntent.FeedSlot feed
                && feed.slot() == 0 && feed.count() == 2)) throw new IllegalStateException("炉位投料阶段或数量错误");
        if (windows.confirmedEffect() != null) throw new IllegalStateException("发包后提前播放成功特效");
        InventoryStateStore.replace(InventoryModel.empty());
        feedSnapshot(2);
        windows.refresh();
        content.tick();
        var effect = windows.confirmedEffect();
        windows.refresh();
        if (effect == null || effect != windows.confirmedEffect()) throw new IllegalStateException("重复快照重放特效");
        draggingMaterial = false;
        render(screen, -1, -1);
    }

    private static void feedSnapshot(int inserted) {
        AlchemySessionStore.replace(new AlchemySessionStore.Snapshot("ling_xi_wan_v1", true, 0, 80,
            .3f, .3f, .15f, 5, 5, "待投首料", List.of(
            new AlchemySessionStore.StageHint(0, 0, "灵草 × 3", inserted >= 3, false,
                List.of(new AlchemySessionStore.IngredientHint("spirit_grass", 3, inserted)))), List.of()));
    }

    private void verifyNotesLifecycle(Screen screen) {
        notesAdapter.paperFrame();
        notesAdapter.title("丹方");
        var before = notesState.bounds();
        int edgeX = before.x() + before.width() / 2;
        int edgeY = before.y() + 12;
        int dragX = Math.min(8, screen.width - before.x() - before.width());
        int dragY = -Math.min(8, before.y());
        screen.mouseClicked(edgeX, edgeY, 0);
        screen.mouseDragged(edgeX + dragX, edgeY + dragY, 0, dragX, dragY);
        screen.mouseReleased(edgeX + dragX, edgeY + dragY, 0);
        if (notesState.bounds().x() != before.x() + dragX || notesState.bounds().y() != before.y() + dragY) {
            throw new IllegalStateException("纸边无法拖动丹方窗口");
        }
        render(screen, -1, -1);
        verifyNotesReading(screen);
        manager.minimize(notesState.key());
        render(screen, -1, -1);
        verifyResumedFurnaceInput(screen);
        manager.restore(notesState.key());
        focus(notesState);
        render(screen, -1, -1);
        var bounds = notesState.bounds();
        screen.mouseClicked(bounds.x() + bounds.width() - 16, bounds.y() + 14, 0);
        screen.mouseReleased(bounds.x() + bounds.width() - 16, bounds.y() + 14, 0);
        if (!notesState.closed()) throw new IllegalStateException("合卷按钮被纸边拖动截获");
        render(screen, -1, -1);
        verifyResumedFurnaceInput(screen);
        openNotes();
        render(screen, -1, -1);
        if (!visible(notesState)) throw new IllegalStateException("丹方关闭后无法重新打开");
    }

    private void verifyNotesReading(Screen screen) {
        var original = notesState.bounds();
        manager.resize(notesState.key(), "100", "100");
        render(screen, -1, -1);
        verifyNotesGeometry();
        var scroll = notesAdapter.content().childById(ScrollContainer.class, "alchemy-notes-scroll");
        int before = scroll.child().y();
        screen.mouseScrolled(scroll.x() + 10, scroll.y() + 10, -4);
        for (int frame = 0; frame < 8; frame++) render(screen, -1, -1);
        if (scroll.child().y() >= before) throw new IllegalStateException("最小纸张窗口内的长文无法滚动");
        scroll.scrollTo(0);
        manager.settleAt(notesState.key(), original);
        render(screen, -1, -1);
    }

    private void verifyNotesGeometry() {
        var bounds = notesState.bounds();
        var paper = notesAdapter.content();
        if (paper.x() != bounds.x() || paper.y() <= bounds.y()
            || paper.width() != bounds.width() || paper.y() + paper.height() != bounds.y() + bounds.height()
            || !notesAdapter.headerAt(bounds.x() + 10, paper.y() - 1)
            || notesAdapter.headerAt(bounds.x() + 10, paper.y())) {
            throw new IllegalStateException("纸张背景未限定在独立标题栏下方的内容区");
        }
        var padding = paper.padding().get();
        for (var child : paper.children()) {
            if (child.x() < paper.x() + padding.left() || child.y() < paper.y() + padding.top()
                || child.x() + child.width() > paper.x() + paper.width() - padding.right()
                || child.y() + child.height() > paper.y() + paper.height() - padding.bottom()) {
                throw new IllegalStateException("丹方或炉记控件越过纸张内边距：" + child.id());
            }
        }
    }

    private void verifyResumedFurnaceInput(Screen screen) {
        int x = artX(.55);
        int y = artY(.36);
        render(screen, x, y);
        screen.mouseClicked(x, y, 0);
        screen.mouseReleased(x, y, 0);
        render(screen, x, y);
        if (!screen.keyPressed(GLFW.GLFW_KEY_F, 0, 0)) {
            throw new IllegalStateException("丹方隐藏后仍拦截炉位快捷键");
        }
    }

    private void verifyKeyboard(Screen screen) {
        render(screen, artX(.55), artY(.53));
        int sent = requests.size();
        render(screen, -1, -1);
        render(screen, artX(.82), artY(.50));
        screen.keyPressed(GLFW.GLFW_KEY_F, 0, GLFW.GLFW_MOD_SHIFT);
        if (requests.size() != sent) throw new IllegalStateException("Shift+F 被误识别为注元");
        int cx = artX(.82);
        int cy = artY(.755);
        render(screen, -1, -1);
        render(screen, -1, -1);
        // 第一次确认后切换焦点再回来，旧确认必须撤销。
        screen.keyPressed(GLFW.GLFW_KEY_R, 0, 0);
        screen.keyReleased(GLFW.GLFW_KEY_R, 0, 0);
        focus(null);
        focus(state);
        render(screen, cx, cy);
        screen.keyPressed(GLFW.GLFW_KEY_R, 0, 0);
        screen.keyPressed(GLFW.GLFW_KEY_R, 0, 0);
        if (requests.size() != sent) throw new IllegalStateException("长按 R 越过了提前收取的二次确认");
        screen.keyReleased(GLFW.GLFW_KEY_R, 0, 0);
        screen.keyPressed(GLFW.GLFW_KEY_R, 0, 0);
        screen.keyReleased(GLFW.GLFW_KEY_R, 0, 0);
        if (requests.size() != sent + 1 || !(requests.get(sent) instanceof AlchemyIntent.TakeBack)) {
            throw new IllegalStateException("松开后再次按 R 未提交收取请求");
        }
        render(screen, -1, -1);
    }

    private AlchemyFurnaceComponent furnace() {
        return adapter.content().childById(AlchemyFurnaceComponent.class, "alchemy-furnace-art");
    }

    /** 以设计画布位置发送真实鼠标输入，窗口留白不应改变交互目标。 */
    private int artX(double ratio) {
        var canvas = furnace().canvasBounds();
        return canvas.x() + (int) Math.round(canvas.width() * ratio);
    }

    private int artY(double ratio) {
        var canvas = furnace().canvasBounds();
        return canvas.y() + (int) Math.round(canvas.height() * ratio);
    }

    private static void render(Screen screen, int x, int y) {
        var client = MinecraftClient.getInstance();
        var context = new DrawContext(client, client.getBufferBuilders().getEntityVertexConsumers());
        screen.render(context, x, y, 0);
        context.draw();
    }

    @Override public void validateGeometry(Screen screen, UiPreviewShot shot) {
        if (visible(notesState)) verifyNotesGeometry();
        if (tinyWindow) {
            if (state.bounds().width() != 100 || state.bounds().height() != 100
                || visible(notesState) && (notesState.bounds().width() != 100 || notesState.bounds().height() != 100)) {
                throw new IllegalStateException("截图中的炼丹或纸页面板不是实际 100×100");
            }
        }
        if (!windows.available()) throw new IllegalStateException("预览缺少有效炉讯");
        var furnace = furnace();
        if (furnace.failure() != null) throw new IllegalStateException(furnace.failure());
        if (furnace.x() < state.bounds().x() || furnace.y() < state.bounds().y()
            || furnace.x() + furnace.width() > state.bounds().x() + state.bounds().width()
            || furnace.y() + furnace.height() > state.bounds().y() + state.bounds().height()) {
            throw new IllegalStateException("工位越过窗口边界");
        }
        if (adapter.content().childById(ButtonComponent.class, "alchemy-ignite") != null) {
            throw new IllegalStateException("工位残留常驻操作按钮");
        }
    }

    @Override public String selectedTemplateId(Screen screen) { return "alchemy-window"; }
    @Override public boolean isReady(Screen screen) { return adapter != null; }
    @Override public boolean initializationFailed(Screen screen) { return false; }
    @Override public void cleanup() {
        animationFrames = null;
        Throwable primary = null;
        try {
            manager.reset();
        } catch (Throwable failure) {
            primary = failure;
        }
        for (var cleanup : List.of(
            (AutoCloseable) () -> { if (content != null) content.close(); },
            (AutoCloseable) () -> { if (adapter != null) adapter.close(); },
            (AutoCloseable) () -> { if (notesAdapter != null) notesAdapter.close(); },
            (AutoCloseable) SessionScopedStoreRegistry::clearAllOnDisconnect
        )) {
            try {
                cleanup.close();
            } catch (Throwable failure) {
                if (primary == null) primary = failure;
                else if (primary != failure) primary.addSuppressed(failure);
            }
        }
        adapter = null;
        notesAdapter = null;
        content = null;
        if (primary != null) UiAlchemyWindowPreviewScene.<RuntimeException>throwUnchecked(primary);
    }

    @SuppressWarnings("unchecked")
    private static <T extends Throwable> void throwUnchecked(Throwable failure) throws T {
        throw (T) failure;
    }
}
