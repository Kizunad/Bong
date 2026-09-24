package com.bong.client.ui.window;

import com.bong.client.ui.contract.DefaultUiScreenScope;
import com.bong.client.ui.contract.UiScreenScope;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;

/** 窗口 identity、z-order、输入捕获和 viewport 约束的唯一实现。 */
public final class UiWindowManager {
    private final Map<WindowKey, WindowState> windows = new LinkedHashMap<>();
    private WindowKey capturedKey;
    private double captureOffsetX;
    private double captureOffsetY;
    private boolean dragging;
    private int viewportWidth;
    private int viewportHeight;
    private long generation;
    private boolean clearing;

    public UiWindowManager(int viewportWidth, int viewportHeight) {
        resizeViewport(viewportWidth, viewportHeight);
    }

    public synchronized WindowState openOrFocus(UiWindowDefinition definition, WindowKey key, Rect initialBounds) {
        Objects.requireNonNull(definition, "definition must not be null");
        Objects.requireNonNull(key, "key must not be null");
        if (clearing || key.connectionGeneration() != generation) {
            throw new IllegalStateException("window belongs to an inactive generation");
        }
        if (!definition.windowType().equals(key.windowType())) {
            throw new IllegalArgumentException("window key type does not match definition");
        }
        WindowState existing = windows.get(key);
        if (existing != null) {
            existing.minimized = false;
            focus(key);
            return existing;
        }
        WindowState created = new WindowState(
            key,
            definition,
            Objects.requireNonNull(initialBounds, "initialBounds must not be null"),
            new DefaultUiScreenScope()
        );
        created.scope().onOpen();
        created.bounds(clamp(initialBounds, definition));
        windows.put(key, created);
        focus(key);
        return created;
    }

    public synchronized boolean contains(WindowKey key) {
        return windows.containsKey(Objects.requireNonNull(key, "key must not be null"));
    }

    public synchronized boolean focus(WindowKey key) {
        WindowState state = windows.remove(Objects.requireNonNull(key, "key must not be null"));
        if (state == null) {
            return false;
        }
        windows.put(key, state);
        return true;
    }

    public synchronized WindowState hitTest(double x, double y) {
        List<WindowState> states = new ArrayList<>(windows.values());
        for (int index = states.size() - 1; index >= 0; index--) {
            WindowState state = states.get(index);
            if (!state.closed() && !state.minimized() && state.bounds().contains(x, y)) {
                return state;
            }
        }
        return null;
    }

    public synchronized boolean beginDrag(double x, double y) {
        WindowState state = capture(x, y);
        return startDrag(state, x, y);
    }

    public synchronized boolean beginDrag(WindowKey key, double x, double y) {
        return startDrag(capture(key), x, y);
    }

    private boolean startDrag(WindowState state, double x, double y) {
        if (state == null || !editable(state)) {
            return false;
        }
        dragging = true;
        captureOffsetX = x - state.bounds().x();
        captureOffsetY = y - state.bounds().y();
        return true;
    }

    public synchronized boolean dragTo(double x, double y) {
        if (capturedKey == null || !dragging) {
            return false;
        }
        WindowState state = windows.get(capturedKey);
        if (state == null || state.closed()) {
            cancelCapture();
            return false;
        }
        Rect requested = new Rect(
            (int) Math.round(x - captureOffsetX),
            (int) Math.round(y - captureOffsetY),
            state.desiredBounds.width(), state.desiredBounds.height());
        Rect effective = clamp(requested, state.definition());
        state.desiredBounds = new Rect(effective.x(), effective.y(), requested.width(), requested.height());
        state.bounds(effective);
        return true;
    }

    public synchronized boolean endDrag() {
        if (capturedKey == null) {
            return false;
        }
        cancelCapture();
        return true;
    }

    public synchronized void cancelCapture() {
        capturedKey = null;
        dragging = false;
    }

    public synchronized WindowState capture(double x, double y) {
        WindowState state = hitTest(x, y);
        return capture(state == null ? null : state.key());
    }

    public synchronized WindowState capture(WindowKey key) {
        cancelCapture();
        WindowState state = windows.get(key);
        if (state != null && state.minimized()) return null;
        if (state != null) {
            focus(state.key());
            capturedKey = state.key();
        }
        return state;
    }

    public synchronized boolean minimize(WindowKey key) {
        WindowState state = windows.get(key);
        if (!editable(state)) return false;
        state.minimized = true;
        if (key.equals(capturedKey)) cancelCapture();
        return true;
    }

    public synchronized boolean restore(WindowKey key) {
        WindowState state = windows.get(key);
        if (state == null) return false;
        state.minimized = false;
        return focus(key);
    }

    public synchronized boolean pin(WindowKey key, boolean pinned) {
        WindowState state = windows.get(key);
        if (!editable(state)) return false;
        if (state.definition.supports(UiWindowDefinition.Capability.STATION)) return false;
        state.pinned = pinned;
        return true;
    }

    /** 用户在转场中抓住窗口时，从当帧可见外框接续交互。 */
    public synchronized void settleAt(WindowKey key, Rect displayed) {
        WindowState state = windows.get(key);
        if (!editable(state)) return;
        Rect effective = clamp(displayed, state.definition);
        state.bounds(effective);
        state.desiredBounds = effective;
    }

    public synchronized boolean resize(WindowKey key, String width, String height) {
        try {
            int w = Integer.parseInt(width.strip());
            int h = Integer.parseInt(height.strip());
            if (w <= 0 || h <= 0) return false;
            WindowState state = windows.get(key);
            if (!editable(state)) return false;
            state.desiredBounds = new Rect(state.bounds.x(), state.bounds.y(),
                Math.max(state.definition.minimumWidth(), w), Math.max(state.definition.minimumHeight(), h));
            state.bounds(clamp(state.desiredBounds, state.definition));
            return true;
        } catch (NumberFormatException | NullPointerException invalid) {
            return false;
        }
    }

    private static boolean editable(WindowState state) {
        return state != null && !state.definition.supports(UiWindowDefinition.Capability.SYSTEM)
            && (state.definition.supports(UiWindowDefinition.Capability.WINDOW)
                || state.definition.supports(UiWindowDefinition.Capability.STATION)
                || state.definition.supports(UiWindowDefinition.Capability.OFFER));
    }

    public synchronized boolean close(WindowKey key) {
        WindowState state = windows.remove(Objects.requireNonNull(key, "key must not be null"));
        if (state == null) {
            return false;
        }
        if (key.equals(capturedKey)) {
            cancelCapture();
        }
        state.close();
        return true;
    }

    public synchronized void resizeViewport(int width, int height) {
        if (width <= 0 || height <= 0) {
            throw new IllegalArgumentException("viewport size must be positive");
        }
        viewportWidth = width;
        viewportHeight = height;
        for (WindowState state : windows.values()) {
            state.bounds(clamp(state.desiredBounds, state.definition()));
        }
    }

    public synchronized WindowKey key(String windowType, String identity) {
        return new WindowKey(windowType, generation, identity);
    }

    /** 先撤销全部 identity，再执行全部清理；清理失败也不能保留旧连接的窗口。 */
    public synchronized void reset() {
        generation = Math.incrementExact(generation);
        cancelCapture();
        List<WindowState> previous = snapshot();
        windows.clear();
        clearing = true;
        DefaultUiScreenScope cleanup = new DefaultUiScreenScope();
        previous.forEach(state -> cleanup.addCleanup(state::close));
        try {
            cleanup.close();
        } finally {
            clearing = false;
        }
    }

    public synchronized void tick(long nowMs) {
        snapshot().forEach(state -> state.scope().onTick(nowMs));
    }

    public synchronized List<WindowState> snapshot() {
        return List.copyOf(windows.values());
    }

    public synchronized WindowKey capturedKey() {
        return capturedKey;
    }

    private Rect clamp(Rect bounds, UiWindowDefinition definition) {
        int width = Math.min(viewportWidth, Math.max(definition.minimumWidth(), bounds.width()));
        int height = Math.min(viewportHeight, Math.max(definition.minimumHeight(), bounds.height()));
        int x = Math.max(0, Math.min(bounds.x(), viewportWidth - width));
        int y = Math.max(0, Math.min(bounds.y(), viewportHeight - height));
        return new Rect(x, y, width, height);
    }

    public record WindowKey(String windowType, long connectionGeneration, String identity) {
        public WindowKey {
            windowType = requireText(windowType, "windowType");
            identity = requireText(identity, "identity");
            if (connectionGeneration < 0) {
                throw new IllegalArgumentException("connectionGeneration must not be negative");
            }
        }

        private static String requireText(String value, String name) {
            Objects.requireNonNull(value, name + " must not be null");
            String normalized = value.strip();
            if (normalized.isEmpty()) {
                throw new IllegalArgumentException(name + " must not be blank");
            }
            return normalized;
        }
    }

    public record Rect(int x, int y, int width, int height) {
        public Rect {
            if (width <= 0 || height <= 0) {
                throw new IllegalArgumentException("rectangle size must be positive");
            }
        }

        public boolean contains(double pointX, double pointY) {
            return pointX >= x && pointX < x + width && pointY >= y && pointY < y + height;
        }
    }

    public static final class WindowState {
        private final WindowKey key;
        private final UiWindowDefinition definition;
        private final UiScreenScope scope;
        private Rect bounds;
        private Rect desiredBounds;
        private boolean closed;
        private boolean minimized;
        private boolean pinned;

        private WindowState(WindowKey key, UiWindowDefinition definition, Rect bounds, UiScreenScope scope) {
            this.key = key;
            this.definition = definition;
            this.bounds = bounds;
            this.desiredBounds = bounds;
            this.scope = scope;
        }

        public WindowKey key() { return key; }
        public UiWindowDefinition definition() { return definition; }
        public UiScreenScope scope() { return scope; }
        public Rect bounds() { return bounds; }
        private void bounds(Rect value) { bounds = value; }
        public boolean closed() { return closed; }
        public boolean minimized() { return minimized; }
        public boolean pinned() { return pinned; }
        public Rect desiredBounds() { return desiredBounds; }

        private void close() {
            if (!closed) {
                closed = true;
                scope.close();
            }
        }
    }
}
