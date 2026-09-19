package com.bong.client.ui.window;

import com.bong.client.inspect.ItemInspectContent;
import com.bong.client.inspect.ItemInspectWindows;
import com.bong.client.inventory.InspectScreen;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.inventory.state.InventoryStateStore;
import com.bong.client.hud.LootContainerStateStore;
import com.bong.client.ui.adapter.owo.OwoXmlWindowContentAdapter;
import com.bong.client.ui.state.StoreUiStateSource;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.DrawContext;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Set;
import java.util.List;

/** UI 组合入口；连接代数仅用于本地窗口，不分配或改写 R2/R6 session token。 */
public final class UiWindowRuntime {
    private static final UiWindowManager MANAGER = new UiWindowManager(320, 240);
    private static final ItemInspectWindows ITEMS = new ItemInspectWindows(
        MANAGER, StoreUiStateSource.pullOnOpen(InventoryStateStore::snapshot),
        StoreUiStateSource.pullOnOpen(() -> LootContainerStateStore.current() instanceof LootContainerStateStore.OpenSession loot
            ? loot.placedItems().stream().map(entry -> entry.item()).toList() : List.of()));
    private static final Map<UiWindowManager.WindowKey, View> VIEWS = new LinkedHashMap<>();
    private static boolean initialized;
    private static Object connection;
    private static Object world;
    private static UiWindowManager.WindowKey focusedKey;
    private static int capturedButton;

    private UiWindowRuntime() {}

    public static void initialize() {
        if (initialized) return;
        ClientTickEvents.END_CLIENT_TICK.register(client -> {
            synchronizeContext(client);
            if (!(client.currentScreen instanceof InspectScreen) || !client.isWindowFocused()) cancelInput();
            ITEMS.refresh();
            MANAGER.tick(System.currentTimeMillis());
        });
        initialized = true;
    }

    public static UiWindowManager manager() { return MANAGER; }

    private static void synchronizeContext(MinecraftClient client) {
        if (connection != client.getNetworkHandler() || world != client.world) {
            connection = client.getNetworkHandler();
            world = client.world;
            cancelInput();
            MANAGER.reset();
        }
        MANAGER.resizeViewport(Math.max(1, client.getWindow().getScaledWidth()),
            Math.max(1, client.getWindow().getScaledHeight()));
    }

    public static void openItem(long instanceId) {
        MinecraftClient client = MinecraftClient.getInstance();
        synchronizeContext(client);
        int offset = MANAGER.snapshot().size() * 18;
        int width = client.getWindow().getScaledWidth();
        int height = client.getWindow().getScaledHeight();
        var state = ITEMS.open(instanceId, new UiWindowManager.Rect(
            (width - 260) / 2 + offset, (height - 280) / 2 + offset, 260, 280));
        if (state != null) focusedKey = state.key();
    }

    public static void renderWorkspace(DrawContext context, int mouseX, int mouseY, float delta) {
        render(context, mouseX, mouseY, delta, null);
    }

    /** P4a 的无 Screen 呈现入口；固定偏好与正式 HUD 登记由 P4b 接入。 */
    public static void renderHud(DrawContext context, float delta, Set<UiWindowManager.WindowKey> visible) {
        if (MinecraftClient.getInstance().currentScreen == null) {
            render(context, -1, -1, delta, Set.copyOf(visible));
        }
    }

    private static void render(DrawContext context, int mouseX, int mouseY, float delta,
                               Set<UiWindowManager.WindowKey> visible) {
        synchronizeContext(MinecraftClient.getInstance());
        var top = visible == null ? MANAGER.hitTest(mouseX, mouseY) : null;
        int windowDepth = 1000;
        for (var state : MANAGER.snapshot()) {
            if (visible != null && !visible.contains(state.key())) continue;
            InventoryItem item = ITEMS.item(state.key());
            if (item == null) continue;
            View view = view(state, item);
            boolean hovered = state == top;
            context.getMatrices().push();
            try {
                // owo 子组件自带局部深度；下一窗口的背景必须位于前一窗口整棵内容树之上。
                context.getMatrices().translate(0, 0, windowDepth);
                view.adapter.render(context, hovered ? mouseX : -1,
                    hovered ? mouseY : -1, delta);
                context.draw();
            } finally {
                context.getMatrices().pop();
            }
            windowDepth += 100;
        }
    }

    private static View view(UiWindowManager.WindowState state, InventoryItem item) {
        View view = VIEWS.get(state.key());
        if (view == null) {
            OwoXmlWindowContentAdapter adapter = new OwoXmlWindowContentAdapter(MANAGER, state);
            view = new View(adapter);
            VIEWS.put(state.key(), view);
            state.scope().addCleanup(() -> {
                VIEWS.remove(state.key());
                adapter.close();
                if (state.key().equals(focusedKey)) focusedKey = null;
            });
        }
        view.adapter.layout(state.bounds());
        if (!item.equals(view.item)) {
            ItemInspectContent.bind(view.adapter.content(), item);
            view.adapter.title(item.displayName());
            view.item = item;
        }
        return view;
    }

    public static boolean mouseDown(double x, double y, int button) {
        if (MANAGER.capturedKey() != null) return true;
        var state = MANAGER.hitTest(x, y);
        if (state == null) {
            cancelInput();
            return false;
        }
        var view = view(state, ITEMS.item(state.key()));
        focusedKey = state.key();
        capturedButton = button;
        if (button == 0 && view.adapter.headerAt(x, y)) MANAGER.beginDrag(x, y);
        else {
            MANAGER.capture(x, y);
            view.adapter.mouseDown(x, y, button);
        }
        return true;
    }

    public static boolean mouseDrag(double x, double y, int button, double dx, double dy) {
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
        var captured = MANAGER.capturedKey();
        if (captured == null) return MANAGER.hitTest(x, y) != null;
        if (button != capturedButton) return true;
        try {
            var view = VIEWS.get(captured);
            if (view != null) view.adapter.mouseUp(x, y, button);
        } finally {
            MANAGER.cancelCapture();
        }
        return true;
    }

    public static boolean scroll(double x, double y, double amount) {
        var state = MANAGER.hitTest(x, y);
        if (state == null) return false;
        view(state, ITEMS.item(state.key())).adapter.scroll(x, y, amount);
        return true;
    }

    public static boolean keyPressed(int key, int scan, int mods) {
        var view = VIEWS.get(focusedKey);
        return view != null && view.adapter.keyPressed(key, scan, mods);
    }

    public static boolean hasKeyboardFocus() { return VIEWS.containsKey(focusedKey); }

    public static boolean charTyped(char chr, int mods) {
        var view = VIEWS.get(focusedKey);
        return view != null && view.adapter.charTyped(chr, mods);
    }

    public static void cancelInput() {
        var view = VIEWS.get(MANAGER.capturedKey());
        MANAGER.cancelCapture();
        focusedKey = null;
        if (view != null) view.adapter.mouseUp(-1, -1, capturedButton);
    }

    private static final class View {
        private final OwoXmlWindowContentAdapter adapter;
        private InventoryItem item;
        private View(OwoXmlWindowContentAdapter adapter) { this.adapter = adapter; }
    }
}
