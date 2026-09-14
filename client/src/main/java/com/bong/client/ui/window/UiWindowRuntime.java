package com.bong.client.ui.window;

import com.bong.client.inspect.ItemInspectContent;
import com.bong.client.inspect.ItemInspectWindows;
import com.bong.client.inventory.InspectScreen;
import com.bong.client.inventory.InventoryContainerWindows;
import com.bong.client.inventory.InventoryContainerContent;
import com.bong.client.inventory.InventoryLoadoutWindows;
import com.bong.client.inventory.component.EquipSlotComponent;
import io.wispforest.owo.ui.container.ScrollContainer;
import com.bong.client.inventory.component.BackpackGridPanel;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.model.InventoryModel;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.hud.LootContainerStateStore;
import com.bong.client.hud.HudRenderCommand;
import com.bong.client.hud.HudWidgetWindows;
import com.bong.client.hud.svg.HudRenderBackend;
import com.bong.client.ui.adapter.owo.OwoXmlWindowContentAdapter;
import com.bong.client.ui.adapter.owo.WindowMotion;
import com.bong.client.ui.adapter.owo.WorkspaceBackgrounds;
import com.bong.client.ui.adapter.owo.WorkspaceControls;
import com.bong.client.ui.state.StoreUiStateSource;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.resource.ResourceManagerHelper;
import net.fabricmc.fabric.api.resource.SimpleSynchronousResourceReloadListener;
import net.minecraft.resource.ResourceType;
import net.minecraft.resource.ResourceManager;
import net.minecraft.util.Identifier;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Set;
import java.util.List;
import java.nio.file.Path;
import java.io.IOException;
import com.bong.client.BongClient;
import org.lwjgl.glfw.GLFW;

/** UI 组合入口；连接代数仅用于本地窗口，不分配或改写 R2/R6 session token。 */
public final class UiWindowRuntime {
    private static final UiWindowManager MANAGER = new UiWindowManager(320, 240);
    private static final ItemInspectWindows ITEMS = new ItemInspectWindows(
        MANAGER, StoreUiStateSource.pullOnOpen(InventoryStateStore::snapshot),
        StoreUiStateSource.pullOnOpen(() -> LootContainerStateStore.current() instanceof LootContainerStateStore.OpenSession loot
            ? loot.placedItems().stream().map(entry -> entry.item()).toList() : List.of()));
    private static final InventoryContainerWindows CONTAINERS = new InventoryContainerWindows(
        MANAGER, StoreUiStateSource.pullOnOpen(InventoryStateStore::snapshot));
    private static final Map<UiWindowManager.WindowKey, View> VIEWS = new LinkedHashMap<>();
    private static boolean initialized;
    private static Object connection;
    private static Object world;
    private static UiWindowManager.WindowKey focusedKey;
    private static int capturedButton;
    private static WindowLayoutPreferenceStore preferences;
    private static WorkspaceBackgrounds backgrounds;
    private static WorkspaceControls controls;
    private static boolean controlsCaptured;
    private static WindowLayoutPreferenceStore savedPreviewPreferences;
    private static InventoryLoadoutWindows loadout;
    private static HudWidgetWindows hud;
    private static HudRenderBackend hudBackend = HudRenderBackend.NOOP;

    private UiWindowRuntime() {}

    public static void initialize() {
        if (initialized) return;
        loadPreferences();
        ResourceManagerHelper.get(ResourceType.CLIENT_RESOURCES).registerReloadListener(new SimpleSynchronousResourceReloadListener() {
            @Override public Identifier getFabricId() { return new Identifier("bong", "workspace-backgrounds"); }
            @Override public void reload(ResourceManager resources) { backgrounds.reload(); }
        });
        ClientTickEvents.END_CLIENT_TICK.register(client -> {
            synchronizeContext(client);
            if (!(client.currentScreen instanceof InspectScreen) || !client.isWindowFocused()) cancelInput();
            ITEMS.refresh();
            CONTAINERS.refresh();
            if (loadout != null) loadout.refresh();
            MANAGER.tick(System.currentTimeMillis());
        });
        initialized = true;
    }

    public static UiWindowManager manager() { return MANAGER; }
    public static HudWidgetWindows hud() { loadPreferences(); return hud; }
    public static void hudBackend(HudRenderBackend backend) { hudBackend = java.util.Objects.requireNonNull(backend); }

    public static void captureHudCommands(List<HudRenderCommand> commands) {
        var client = MinecraftClient.getInstance();
        loadPreferences();
        synchronizeContext(client);
        hud.capture(commands, client.getWindow().getScaledWidth(), client.getWindow().getScaledHeight(),
            client.textRenderer::getWidth);
    }

    public static List<HudRenderCommand> layoutHudCommands(List<HudRenderCommand> commands) {
        return hud().layout(commands);
    }

    public static void openHud(HudWidgetWindows.Widget widget) {
        cancelInput();
        focusedKey = hud().open(widget).key();
        savePreferences();
    }

    public static void showHud(HudWidgetWindows.Widget widget, boolean visible) {
        hud().visible(widget, visible);
        savePreferences();
    }
    public static InventoryContainerWindows containers() { return CONTAINERS; }
    public static InventoryLoadoutWindows loadout() {
        if (loadout == null) loadout = new InventoryLoadoutWindows();
        loadout.refresh();
        return loadout;
    }

    private static void loadPreferences() {
        if (preferences != null) return;
        Path directory = MinecraftClient.getInstance().runDirectory.toPath().resolve("config/bong");
        preferences = new WindowLayoutPreferenceStore(directory.resolve("window-layout.json"));
        try { preferences.load(); }
        catch (IOException failure) { BongClient.LOGGER.warn("无法读取窗口布局，使用默认布局", failure); }
        backgrounds = new WorkspaceBackgrounds(directory.resolve("workspace-backgrounds"));
        backgrounds.select(preferences.background());
        createControls();
    }

    private static void createControls() {
        hud = new HudWidgetWindows(MANAGER, preferences);
        controls = new WorkspaceControls(MANAGER, backgrounds, preferences, UiWindowRuntime::savePreferences,
            hud, UiWindowRuntime::openHud, UiWindowRuntime::showHud, UiWindowRuntime::resetLayout,
            UiWindowRuntime::remember);
    }

    private static void savePreferences() {
        if (savedPreviewPreferences != null) return;
        try { preferences.save(); }
        catch (IOException failure) { BongClient.LOGGER.warn("无法保存窗口布局", failure); }
    }

    private static void remember(UiWindowManager.WindowState state) {
        if (state.minimized() && state.key().equals(focusedKey)) cancelInput();
        if (HudWidgetWindows.widget(state.key()) != null) hud.remember(state);
        else preferences.remember(state);
        savePreferences();
    }

    /** 仅由显式预览 harness 调用，场景不能污染玩家的磁盘偏好。 */
    public static void beginPreview() {
        loadPreferences();
        controls.close();
        savedPreviewPreferences = preferences;
        preferences = new WindowLayoutPreferenceStore(MinecraftClient.getInstance().runDirectory.toPath());
        preferences.motion(false);
        CONTAINERS.reset();
        loadout = null;
        backgrounds.select("cosmos");
        createControls();
    }

    public static void endPreview() {
        if (savedPreviewPreferences == null) return;
        controls.close();
        preferences = savedPreviewPreferences;
        savedPreviewPreferences = null;
        backgrounds.select(preferences.background());
        createControls();
    }

    private static void synchronizeContext(MinecraftClient client) {
        if (connection != client.getNetworkHandler() || world != client.world) {
            connection = client.getNetworkHandler();
            world = client.world;
            cancelInput();
            MANAGER.reset();
            CONTAINERS.reset();
            loadout = null;
            if (hud != null) hud.resetSession();
        }
        MANAGER.resizeViewport(Math.max(1, client.getWindow().getScaledWidth()),
            Math.max(1, client.getWindow().getScaledHeight() - 28));
    }

    public static void openItem(long instanceId) {
        MinecraftClient client = MinecraftClient.getInstance();
        synchronizeContext(client);
        loadPreferences();
        var key = MANAGER.key(ItemInspectWindows.DEFINITION.windowType(), Long.toString(instanceId));
        boolean existing = MANAGER.contains(key);
        var preference = preferences.window(key.windowType());
        int offset = MANAGER.snapshot().size() * 18;
        int width = client.getWindow().getScaledWidth();
        int height = client.getWindow().getScaledHeight();
        var bounds = preference == null ? new UiWindowManager.Rect(
            (width - 260) / 2 + offset, (height - 280) / 2 + offset, 260, 280) : preference.bounds();
        var state = ITEMS.open(instanceId, bounds);
        if (state != null && !existing && preference != null) {
            MANAGER.pin(state.key(), preference.pinned());
            // 显式双击是打开命令，总是展开；最小化偏好用于未来个人窗口的自动恢复。
        }
        if (state != null) focusedKey = state.key();
    }

    public static void openContainer(String id) {
        var client = MinecraftClient.getInstance();
        synchronizeContext(client);
        loadPreferences();
        CONTAINERS.refresh();
        var def = CONTAINERS.definition(id);
        if (def == null) return;
        var key = MANAGER.key(InventoryContainerWindows.DEFINITION.windowType(), id);
        boolean existing = MANAGER.contains(key);
        var preference = preferences.window(key.windowType());
        int offset = (int) MANAGER.snapshot().stream()
            .filter(state -> state.definition().equals(InventoryContainerWindows.DEFINITION)).count() * 20;
        int width = Math.max(180, Math.min(360, def.cols() * 28 + 28));
        int height = Math.max(140, Math.min(340, def.rows() * 28 + 88));
        var bounds = preference == null ? new UiWindowManager.Rect(
            client.getWindow().getScaledWidth() - width - 12 - offset, 30 + offset, width, height) : preference.bounds();
        var state = CONTAINERS.open(id, bounds);
        if (!existing && preference != null) MANAGER.pin(state.key(), preference.pinned());
        focusedKey = state.key();
    }

    public static void openInventory(List<InventoryModel.ContainerDef> definitions) {
        CONTAINERS.refresh();
        for (var def : definitions) {
            var key = MANAGER.key(InventoryContainerWindows.DEFINITION.windowType(), def.id());
            if (!MANAGER.contains(key)) openContainer(def.id());
        }
    }

    public static void openLoadout(UiWindowDefinition definition) {
        var client = MinecraftClient.getInstance();
        synchronizeContext(client);
        loadPreferences();
        var key = MANAGER.key(definition.windowType(), "player");
        boolean existing = MANAGER.contains(key);
        var preference = preferences.window(key.windowType());
        var initial = definition.equals(InventoryLoadoutWindows.EQUIPMENT)
            ? new UiWindowManager.Rect(12, 30, 190, 256)
            : new UiWindowManager.Rect(212, 30, 180, 194);
        var state = MANAGER.openOrFocus(definition, key, preference == null ? initial : preference.bounds());
        if (!existing && preference != null) MANAGER.pin(key, preference.pinned());
        focusedKey = state.key();
    }

    public static void renderWorkspace(DrawContext context, int mouseX, int mouseY, float delta) {
        loadPreferences();
        captureHudCommands(com.bong.client.BongHud.workspaceCommands());
        var window = MinecraftClient.getInstance().getWindow();
        controls.layout(window.getScaledWidth(), window.getScaledHeight(),
            UiWindowRuntime::windowTitle);
        render(context, mouseX, mouseY, delta, null);
        context.getMatrices().push();
        try {
            context.getMatrices().translate(0, 0, overlayDepth());
            controls.render(context, mouseX, mouseY, delta);
            context.draw();
        } finally { context.getMatrices().pop(); }
    }

    public static void renderBackground(DrawContext context, int width, int height) {
        loadPreferences();
        backgrounds.render(context, width, height);
    }

    public static int overlayDepth() { return 1400 + MANAGER.snapshot().size() * 400; }

    /** HUD 复用同一个 adapter，禁止接收输入或盖住系统界面。 */
    public static void renderHud(DrawContext context, float delta) {
        var client = MinecraftClient.getInstance();
        if (client.currentScreen != null || client.player == null || client.options.hudHidden) return;
        renderHud(context, delta, MANAGER.snapshot().stream().filter(state -> state.pinned() && !state.minimized()
                && HudWidgetWindows.widget(state.key()) == null)
            .map(UiWindowManager.WindowState::key).collect(java.util.stream.Collectors.toSet()));
    }

    public static void renderHud(DrawContext context, float delta, Set<UiWindowManager.WindowKey> visible) {
        if (MinecraftClient.getInstance().currentScreen == null) {
            render(context, -1, -1, delta, Set.copyOf(visible));
        }
    }

    private static void render(DrawContext context, int mouseX, int mouseY, float delta,
                               Set<UiWindowManager.WindowKey> visible) {
        synchronizeContext(MinecraftClient.getInstance());
        var top = visible == null && !controls.hit(mouseX, mouseY) ? windowAt(mouseX, mouseY) : null;
        long now = System.nanoTime();
        int windowDepth = 1000;
        for (var state : MANAGER.snapshot()) {
            if (visible != null && !visible.contains(state.key())) continue;
            if (visible != null && HudWidgetWindows.widget(state.key()) != null) continue;
            View view = view(state);
            view.adapter.workspace(visible == null);
            var target = state.minimized() ? controls.anchor(state.key()) : state.bounds();
            view.motion.target(target, now, preferences.motion() && !state.key().equals(MANAGER.capturedKey()));
            var displayed = view.motion.sample(now);
            if (state.minimized() && view.motion.settled(now)) continue;
            view.displayed = displayed;
            boolean shrinking = state.minimized() || displayed.width() < state.definition().minimumWidth()
                || displayed.height() < state.definition().minimumHeight();
            var layout = shrinking ? state.bounds() : displayed;
            view.adapter.layout(layout);
            boolean hovered = state == top;
            context.getMatrices().push();
            try {
                // owo 子组件自带局部深度；下一窗口的背景必须位于前一窗口整棵内容树之上。
                context.getMatrices().translate(0, 0, windowDepth);
                if (shrinking) {
                    context.getMatrices().translate(displayed.x(), displayed.y(), 0);
                    context.getMatrices().scale((float) displayed.width() / layout.width(),
                        (float) displayed.height() / layout.height(), 1);
                    context.getMatrices().translate(-layout.x(), -layout.y(), 0);
                }
                view.adapter.render(context, hovered ? mouseX : -1,
                    hovered ? mouseY : -1, delta);
                context.draw();
            } finally {
                context.getMatrices().pop();
            }
            windowDepth += 400;
        }
    }

    private static String windowTitle(UiWindowManager.WindowKey key) {
        var widget = HudWidgetWindows.widget(key);
        if (widget != null) return widget.title();
        if (key.windowType().equals(InventoryLoadoutWindows.EQUIPMENT.windowType())) return "装备";
        if (key.windowType().equals(InventoryLoadoutWindows.SHORTCUTS.windowType())) return "快捷槽";
        if (key.windowType().equals(InventoryContainerWindows.DEFINITION.windowType())) {
            var def = CONTAINERS.definition(key.identity());
            return def == null ? "" : def.name();
        }
        var item = ITEMS.item(key);
        return item == null ? "" : item.displayName();
    }

    private static View view(UiWindowManager.WindowState state) {
        View view = VIEWS.get(state.key());
        if (view == null) {
            OwoXmlWindowContentAdapter adapter = new OwoXmlWindowContentAdapter(MANAGER, state, () -> remember(state));
            var bounds = state.bounds();
            var initial = new UiWindowManager.Rect(bounds.x() + 2, bounds.y() + 2,
                Math.max(1, bounds.width() - 4), Math.max(1, bounds.height() - 4));
            view = new View(adapter, initial);
            var widget = HudWidgetWindows.widget(state.key());
            if (widget != null) {
                adapter.title(widget.title());
                adapter.closeAction(() -> { hud.close(widget); savePreferences(); });
                adapter.content().surface((context, component) -> {
                    var panelBounds = new UiWindowManager.Rect(component.x(), component.y(),
                        Math.max(1, component.width()), Math.max(1, component.height()));
                    context.enableScissor(panelBounds.x(), panelBounds.y(),
                        panelBounds.x() + panelBounds.width(), panelBounds.y() + panelBounds.height());
                    try {
                        var commands = hud.preview(widget, panelBounds);
                        if (commands.isEmpty()) {
                            var renderer = MinecraftClient.getInstance().textRenderer;
                            context.drawText(renderer, "未触发", panelBounds.x() + 8, panelBounds.y() + 12, 0xFF8C9794, false);
                        } else com.bong.client.BongHud.renderPanelCommands(context, commands, hudBackend);
                    } finally { context.disableScissor(); }
                });
            } else if (state.definition().equals(InventoryContainerWindows.DEFINITION)) {
                view.container = new InventoryContainerContent(adapter.content());
            } else if (state.definition().equals(InventoryLoadoutWindows.EQUIPMENT)
                || state.definition().equals(InventoryLoadoutWindows.SHORTCUTS)) {
                view.loadout = loadout();
                view.loadout.attach(state.definition(), adapter.content());
                view.adapter.title(windowTitle(state.key()));
            }
            VIEWS.put(state.key(), view);
            View ownedView = view;
            state.scope().addCleanup(() -> {
                VIEWS.remove(state.key());
                if (ownedView.loadout != null) ownedView.loadout.detach(state.definition(), adapter.content());
                adapter.close();
                if (state.key().equals(focusedKey)) focusedKey = null;
            });
        }
        InventoryItem item = ITEMS.item(state.key());
        if (view.container != null) {
            view.container.bind(CONTAINERS, state.key().identity());
            view.adapter.title(windowTitle(state.key()));
        } else if (item != null && !item.equals(view.item)) {
            ItemInspectContent.bind(view.adapter.content(), item);
            view.adapter.title(item.displayName());
            view.item = item;
        }
        return view;
    }

    public static boolean mouseDown(double x, double y, int button) {
        if (MANAGER.capturedKey() != null || controlsCaptured) return true;
        if (controls != null && controls.mouseDown(x, y, button)) {
            cancelInput();
            controlsCaptured = true;
            return true;
        }
        var state = windowAt(x, y);
        if (state == null) {
            cancelInput();
            return false;
        }
        var view = view(state);
        if (!view.motion.settled(System.nanoTime())) MANAGER.settleAt(state.key(), view.displayed);
        view.adapter.layout(state.bounds());
        view.displayed = state.bounds();
        view.motion.target(state.bounds(), System.nanoTime(), false);
        var previous = VIEWS.get(focusedKey);
        if (previous != null && previous != view) previous.adapter.cancelInput();
        focusedKey = state.key();
        capturedButton = button;
        if (button == 0 && view.adapter.headerAt(x, y)) MANAGER.beginDrag(state.key(), x, y);
        else {
            MANAGER.capture(state.key());
            view.adapter.mouseDown(x, y, button);
        }
        return true;
    }

    public static boolean mouseDrag(double x, double y, int button, double dx, double dy) {
        if (controlsCaptured) return true;
        var captured = MANAGER.capturedKey();
        if (captured == null) return false;
        if (button != capturedButton) return true;
        if (!MANAGER.dragTo(x, y)) {
            var view = VIEWS.get(captured);
            if (view != null) view.adapter.mouseDrag(x, y, button, dx, dy);
        }
        return true;
    }

    public static boolean mouseUp(double x, double y, int button) {
        if (controlsCaptured) {
            controlsCaptured = false;
            controls.mouseUp(x, y, button);
            return true;
        }
        var captured = MANAGER.capturedKey();
        if (captured == null) return MANAGER.hitTest(x, y) != null;
        if (button != capturedButton) return true;
        try {
            var view = VIEWS.get(captured);
            if (view != null) view.adapter.mouseUp(x, y, button);
            for (var state : MANAGER.snapshot()) if (state.key().equals(captured)) remember(state);
        } finally {
            MANAGER.cancelCapture();
        }
        return true;
    }

    public static boolean scroll(double x, double y, double amount) {
        if (controls != null && controls.hit(x, y)) {
            controls.scroll(x, y, amount);
            return true;
        }
        var state = windowAt(x, y);
        if (state == null) return false;
        view(state).adapter.scroll(x, y, amount);
        return true;
    }

    public static boolean keyPressed(int key, int scan, int mods) {
        if (key == GLFW.GLFW_KEY_ESCAPE && controls != null && controls.escape()) return true;
        var view = VIEWS.get(focusedKey);
        return view != null && view.adapter.keyPressed(key, scan, mods);
    }

    public static boolean hasKeyboardFocus() {
        var view = VIEWS.get(focusedKey);
        return view != null && view.adapter.textFocused();
    }

    public static boolean charTyped(char chr, int mods) {
        var view = VIEWS.get(focusedKey);
        return view != null && view.adapter.charTyped(chr, mods);
    }

    /** 还原呈现布局，保留窗口 identity、业务 scope、物品与制作会话。 */
    public static void resetLayout() {
        loadPreferences();
        cancelInput();
        preferences.resetLayouts();
        hud.resetLayouts();
        int itemOffset = 0, containerOffset = 0;
        var window = MinecraftClient.getInstance().getWindow();
        for (var state : MANAGER.snapshot()) {
            if (HudWidgetWindows.widget(state.key()) != null) continue;
            var definition = state.definition();
            UiWindowManager.Rect bounds;
            if (definition.equals(InventoryLoadoutWindows.EQUIPMENT)) bounds = new UiWindowManager.Rect(12, 30, 190, 256);
            else if (definition.equals(InventoryLoadoutWindows.SHORTCUTS)) bounds = new UiWindowManager.Rect(212, 30, 180, 194);
            else if (definition.equals(InventoryContainerWindows.DEFINITION)) {
                var def = CONTAINERS.definition(state.key().identity());
                if (def == null) continue;
                int width = Math.max(180, Math.min(360, def.cols() * 28 + 28));
                int height = Math.max(140, Math.min(340, def.rows() * 28 + 88));
                bounds = new UiWindowManager.Rect(window.getScaledWidth() - width - 12 - containerOffset,
                    30 + containerOffset, width, height);
                containerOffset += 20;
            } else if (definition.equals(ItemInspectWindows.DEFINITION)) {
                bounds = new UiWindowManager.Rect((window.getScaledWidth() - 260) / 2 + itemOffset,
                    (window.getScaledHeight() - 280) / 2 + itemOffset, 260, 280);
                itemOffset += 18;
            } else continue;
            MANAGER.settleAt(state.key(), bounds);
            MANAGER.pin(state.key(), false);
            MANAGER.restore(state.key());
        }
        savePreferences();
    }

    public static void cancelInput() {
        var view = VIEWS.get(focusedKey);
        MANAGER.cancelCapture();
        controlsCaptured = false;
        focusedKey = null;
        if (view != null) view.adapter.cancelInput();
    }

    public static boolean hit(double x, double y) {
        return controls != null && controls.hit(x, y) || windowAt(x, y) != null;
    }

    public static BackpackGridPanel containerGridAt(double x, double y) {
        if (controls != null && controls.hit(x, y)) return null;
        var state = windowAt(x, y);
        if (state == null || !state.definition().equals(InventoryContainerWindows.DEFINITION)) return null;
        var view = VIEWS.get(state.key());
        return view != null && view.container.gridAt(x, y)
            ? CONTAINERS.grid(state.key().identity()) : null;
    }

    public static void focusContainerAt(double x, double y) {
        var state = windowAt(x, y);
        if (state == null || containerGridAt(x, y) == null) return;
        cancelInput();
        MANAGER.openOrFocus(state.definition(), state.key(), state.bounds());
    }

    private static boolean loadoutAt(UiWindowDefinition definition, double x, double y) {
        if (controls != null && controls.hit(x, y)) return false;
        var state = windowAt(x, y);
        if (state == null || !state.definition().equals(definition)) return false;
        var view = VIEWS.get(state.key());
        if (view == null) return false;
        var scroll = view.adapter.content().childById(ScrollContainer.class, "loadout-scroll");
        return x >= scroll.x() && x < scroll.x() + scroll.width() - scroll.scrollbarThiccness()
            && y >= scroll.y() && y < scroll.y() + scroll.height();
    }

    public static EquipSlotComponent equipmentAt(double x, double y) {
        return loadoutAt(InventoryLoadoutWindows.EQUIPMENT, x, y) ? loadout().equipment().slotAtScreen(x, y) : null;
    }

    public static int hotbarAt(double x, double y) {
        return loadoutAt(InventoryLoadoutWindows.SHORTCUTS, x, y) ? loadout().hotbarAt(x, y) : -1;
    }

    public static int quickUseAt(double x, double y) {
        return loadoutAt(InventoryLoadoutWindows.SHORTCUTS, x, y) ? loadout().quickUseAt(x, y) : -1;
    }

    public static boolean loadoutSlotAt(double x, double y) {
        return equipmentAt(x, y) != null || hotbarAt(x, y) >= 0 || quickUseAt(x, y) >= 0;
    }

    public static void focusLoadoutAt(double x, double y) {
        var state = windowAt(x, y);
        if (state == null || !loadoutSlotAt(x, y)) return;
        cancelInput();
        MANAGER.focus(state.key());
    }

    private static UiWindowManager.WindowState windowAt(double x, double y) {
        var states = MANAGER.snapshot();
        for (int index = states.size() - 1; index >= 0; index--) {
            var state = states.get(index);
            var view = VIEWS.get(state.key());
            var bounds = view == null ? state.bounds() : view.displayed;
            if (!state.minimized() && bounds.contains(x, y)) return state;
        }
        return null;
    }

    public static void previewMotion(boolean enabled) { preferences.motion(enabled); }

    public static boolean previewBackground(String id) { return backgrounds.select(id); }

    public static UiWindowManager.Rect restoreBoundsForPreview(UiWindowManager.WindowKey key) {
        return controls.anchor(key);
    }

    public static UiWindowManager.Rect workspaceControlBoundsForPreview(String... path) {
        return controls.boundsForPreview(path);
    }

    private static final class View {
        private final OwoXmlWindowContentAdapter adapter;
        private InventoryItem item;
        private InventoryContainerContent container;
        private InventoryLoadoutWindows loadout;
        private final WindowMotion motion;
        private UiWindowManager.Rect displayed;
        private View(OwoXmlWindowContentAdapter adapter, UiWindowManager.Rect initial) {
            this.adapter = adapter;
            motion = new WindowMotion(initial);
            displayed = initial;
        }
    }
}
