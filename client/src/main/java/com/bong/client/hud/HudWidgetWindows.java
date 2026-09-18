package com.bong.client.hud;

import com.bong.client.ui.window.UiWindowDefinition;
import com.bong.client.ui.window.UiWindowManager;
import com.bong.client.ui.window.WindowLayoutPreferenceStore;
import com.bong.client.ui.window.WindowLayoutPreferenceStore.HudPreference;
import java.util.ArrayList;
import java.util.EnumMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;

/** 原有 HUD 的呈现布局。只读取绘制命令，窗口操作不修改技能、状态或服务端数据。 */
public final class HudWidgetWindows {
    public enum Widget {
        MINI_BODY("人体与气血", HudRenderLayer.MINI_BODY),
        QUICK_BAR("双手与快捷栏", HudRenderLayer.QUICK_BAR),
        MOVEMENT("身法", HudRenderLayer.MOVEMENT_HUD),
        STATUS_EFFECTS("状态效果", HudRenderLayer.STATUS_EFFECTS),
        CAST("施法", HudRenderLayer.CAST_BAR),
        GATHERING("采集", HudRenderLayer.GATHERING),
        ZONE("区域信息", HudRenderLayer.ZONE),
        ATTRIBUTES("战斗属性", HudRenderLayer.DERIVED_ATTR),
        BOTANY("草木辨识", HudRenderLayer.BOTANY),
        PROCESSING("制作进度", HudRenderLayer.PROCESSING_HUD),
        LINGTIAN("灵田", HudRenderLayer.LINGTIAN_OVERLAY),
        SEARCH("搜寻进度", HudRenderLayer.SEARCH_PROGRESS),
        FLIGHT("飞行", HudRenderLayer.FLIGHT_HUD),
        COFFIN("棺木状态", HudRenderLayer.COFFIN),
        TRIBULATION("灾劫广播", HudRenderLayer.TRIBULATION),
        EXTRACT("撤离进度", HudRenderLayer.TSY_EXTRACT),
        HOME("归龛整理", HudRenderLayer.HOME_SEQUENCE),
        COLLAPSE("秘境崩塌", HudRenderLayer.REALM_COLLAPSE),
        MERIDIAN("经脉", HudRenderLayer.MERIDIAN_OPEN),
        SPELL_VOLUME("招式容量", HudRenderLayer.SPELL_VOLUME),
        CARRIER("暗器载体", HudRenderLayer.CARRIER),
        YIDAO("医道", HudRenderLayer.YIDAO),
        JIEMAI("截脉防御", HudRenderLayer.JIEMAI_RING),
        VORTEX_CHARGE("涡流蓄力", HudRenderLayer.VORTEX_CHARGE),
        VORTEX_COOLDOWN("涡流冷却", HudRenderLayer.VORTEX_COOLDOWN),
        VORTEX_BACKFIRE("涡流反噬", HudRenderLayer.VORTEX_BACKFIRE),
        VORTEX_TURBULENCE("涡流乱流", HudRenderLayer.VORTEX_TURBULENCE),
        TAINT("毒蛊侵染", HudRenderLayer.DUGU_TAINT_INDICATOR),
        REVEAL("暴露风险", HudRenderLayer.DUGU_REVEAL_RISK),
        SELF_CURE("自愈进度", HudRenderLayer.DUGU_SELF_CURE_PROGRESS),
        QI_DECAY("真元衰减", HudRenderLayer.DUGU_QI_DECAY),
        POISON("毒性", HudRenderLayer.POISON_TRAIT),
        MUTATION("丹道异变", HudRenderLayer.DANDAO_MUTATION),
        SWORD_BOND("剑契", HudRenderLayer.SWORD_BOND),
        ELDER("大能遭遇", HudRenderLayer.DYING_ELDER),
        RECHALLENGE("重渡时机", HudRenderLayer.HALFSTEP_RECHALLENGE),
        PARRY("弹反反馈", HudRenderLayer.ZHENMAI_PARRY),
        NEUTRALIZE("中和反馈", HudRenderLayer.ZHENMAI_NEUTRALIZE),
        MULTIPOINT("反震点数", HudRenderLayer.ZHENMAI_MULTIPOINT),
        HARDEN("护脉", HudRenderLayer.ZHENMAI_HARDEN),
        SEVER("绝脉", HudRenderLayer.ZHENMAI_SEVER),
        GUARDIAN("灵龛守护", HudRenderLayer.NICHE_GUARDIAN),
        MORPH("易形", HudRenderLayer.MORPH);

        private final String title;
        private final HudRenderLayer layer;
        private final UiWindowDefinition definition;

        Widget(String title, HudRenderLayer layer) {
            this.title = title;
            this.layer = layer;
            this.definition = new UiWindowDefinition("hud-" + name().toLowerCase(Locale.ROOT),
                "hud-widget", 180, 54, Set.of(UiWindowDefinition.Capability.WINDOW));
        }
        public String title() { return title; }
        public String id() { return definition.windowType(); }
        public UiWindowDefinition definition() { return definition; }
    }

    private static final Map<HudRenderLayer, Widget> BY_LAYER = new EnumMap<>(HudRenderLayer.class);
    static { for (var widget : Widget.values()) BY_LAYER.put(widget.layer, widget); }
    private final UiWindowManager manager;
    private final WindowLayoutPreferenceStore preferences;
    private final Map<Widget, Frame> frames = new EnumMap<>(Widget.class);
    private final Map<Widget, UiWindowManager.Rect> editingOrigins = new EnumMap<>(Widget.class);
    private final Map<Widget, UiWindowManager.Rect> editingBounds = new EnumMap<>(Widget.class);
    private int width = 320;
    private int height = 240;

    public HudWidgetWindows(UiWindowManager manager, WindowLayoutPreferenceStore preferences) {
        this.manager = manager;
        this.preferences = preferences;
    }

    public static Widget widget(UiWindowManager.WindowKey key) {
        for (var widget : Widget.values()) if (widget.id().equals(key.windowType())) return widget;
        return null;
    }

    public void capture(List<HudRenderCommand> commands, int width, int height, HudTextHelper.WidthMeasurer measurer) {
        boolean resized = this.width != width || this.height != height;
        this.width = Math.max(1, width);
        this.height = Math.max(1, height);
        var grouped = new EnumMap<Widget, List<HudRenderCommand>>(Widget.class);
        for (var command : commands) {
            var widget = movableWidget(command);
            if (widget != null) grouped.computeIfAbsent(widget, ignored -> new ArrayList<>()).add(command);
        }
        frames.clear();
        grouped.forEach((widget, group) -> frames.put(widget, new Frame(List.copyOf(group), bounds(group, measurer))));
        if (resized) {
            editingOrigins.clear();
            editingBounds.clear();
            for (var state : manager.snapshot()) {
                var widget = widget(state.key());
                if (widget != null) {
                    manager.settleAt(state.key(), initialBounds(widget));
                    editingBounds.put(widget, state.bounds());
                }
            }
        }
    }

    public List<HudRenderCommand> layout(List<HudRenderCommand> commands) {
        var result = new ArrayList<HudRenderCommand>(commands.size());
        for (var command : commands) {
            var widget = movableWidget(command);
            if (widget == null) {
                result.add(command);
                continue;
            }
            var preference = preferences.hud(widget.id());
            if (preference.visible()) result.add(command.transformed(
                preference.translationX(width), preference.translationY(height), preference.scale()));
        }
        return List.copyOf(result);
    }

    private Widget movableWidget(HudRenderCommand command) {
        if (command.isScreenTint() || command.isEdgeVignette() || command.isEdgeInkWash()
            || command.isEdgeIndicator() || command.isToast()) return null;
        // 混合 layer 中的全屏遮罩仍属于屏幕，不能跟随一个小面板移动或消失。
        if (command.width() >= width * .8 && command.height() >= height * .8) return null;
        return BY_LAYER.get(command.layer());
    }

    public boolean active(Widget widget) { return frames.containsKey(widget); }
    public boolean visible(Widget widget) { return preferences.hud(widget.id()).visible(); }

    public UiWindowManager.WindowState open(Widget widget) {
        var key = manager.key(widget.id(), "player");
        boolean existing = manager.contains(key);
        var state = manager.openOrFocus(widget.definition(), key, initialBounds(widget));
        editingBounds.putIfAbsent(widget, state.bounds());
        if (existing) preferences.hud(widget.id(), preferences.hud(widget.id()).visible(state.pinned()));
        else manager.pin(state.key(), visible(widget));
        return state;
    }

    private UiWindowManager.Rect initialBounds(Widget widget) {
        var source = editingOrigins.computeIfAbsent(widget, this::sourceBounds);
        var preference = preferences.hud(widget.id());
        int x = (int) Math.round(source.x() * preference.scale() + preference.translationX(width));
        int y = (int) Math.round(source.y() * preference.scale() + preference.translationY(height));
        return new UiWindowManager.Rect(x - 4, y - 26,
            Math.max(widget.definition().minimumWidth(), (int) Math.round(source.width() * preference.scale()) + 8),
            Math.max(54, (int) Math.round(source.height() * preference.scale()) + 30));
    }

    private UiWindowManager.Rect sourceBounds(Widget widget) {
        var frame = frames.get(widget);
        return frame == null ? new UiWindowManager.Rect(Math.max(4, (width - 160) / 2),
            Math.max(30, (height - 48) / 2), 160, 48) : frame.bounds();
    }

    public void remember(UiWindowManager.WindowState state) {
        var widget = widget(state.key());
        if (widget == null) return;
        var source = editingOrigins.computeIfAbsent(widget, this::sourceBounds);
        var target = state.bounds();
        var previous = editingBounds.getOrDefault(widget, target);
        var preference = preferences.hud(widget.id());
        double scale = resizedScale(preference.scale(), previous, target);
        // 锁定/最小化不改变几何；最小外框及 viewport 裁剪不能反向改写原 HUD 布局。
        double x = preference.offsetX(), y = preference.offsetY();
        if (!target.equals(previous)) {
            x = (target.x() + 4 - source.x() * scale) / width - (1 - scale) / 2;
            y = (target.y() + 26 - source.y() * scale) / height - (1 - scale) / 2;
        }
        preferences.hud(widget.id(), new HudPreference(x, y, scale, state.pinned() && !state.minimized()));
        editingBounds.put(widget, target);
    }

    public void visible(Widget widget, boolean visible) {
        preferences.hud(widget.id(), preferences.hud(widget.id()).visible(visible));
        var key = manager.key(widget.id(), "player");
        manager.pin(key, visible);
        if (visible && manager.contains(key)) manager.restore(key);
    }

    public void close(Widget widget) {
        visible(widget, false);
        manager.close(manager.key(widget.id(), "player"));
        editingOrigins.remove(widget);
        editingBounds.remove(widget);
    }

    public List<HudRenderCommand> preview(Widget widget, UiWindowManager.Rect content) {
        var frame = frames.get(widget);
        if (frame == null) return List.of();
        var source = editingOrigins.computeIfAbsent(widget, this::sourceBounds);
        var state = manager.snapshot().stream().filter(window -> widget(window.key()) == widget).findFirst().orElse(null);
        if (state == null) return List.of();
        double scale = resizedScale(preferences.hud(widget.id()).scale(),
            editingBounds.getOrDefault(widget, state.bounds()), state.bounds());
        double x = content.x() + 4 - source.x() * scale;
        double y = content.y() + 4 - source.y() * scale;
        return frame.commands().stream().map(command -> command.transformed(x, y, scale)).toList();
    }

    public void resetLayouts() {
        editingOrigins.clear();
        editingBounds.clear();
        for (var widget : Widget.values()) preferences.hud(widget.id(), HudPreference.DEFAULT);
        for (var state : manager.snapshot()) {
            var widget = widget(state.key());
            if (widget == null) continue;
            manager.settleAt(state.key(), initialBounds(widget));
            editingBounds.put(widget, state.bounds());
            manager.pin(state.key(), true);
            manager.restore(state.key());
        }
    }

    public void resetSession() { frames.clear(); editingOrigins.clear(); editingBounds.clear(); }

    private static double resizedScale(double scale, UiWindowManager.Rect previous, UiWindowManager.Rect target) {
        return Math.max(.25, Math.min(4, scale * Math.min(
            Math.max(1, target.width() - 8.0) / Math.max(1, previous.width() - 8.0),
            Math.max(1, target.height() - 30.0) / Math.max(1, previous.height() - 30.0))));
    }

    private static UiWindowManager.Rect bounds(List<HudRenderCommand> commands, HudTextHelper.WidthMeasurer measurer) {
        int left = Integer.MAX_VALUE, top = Integer.MAX_VALUE, right = Integer.MIN_VALUE, bottom = Integer.MIN_VALUE;
        for (var command : commands) {
            int width = command.width(), height = command.height();
            if (command.isText() || command.isScaledText()) {
                width = (int) Math.ceil(measurer.measure(command.text()) * command.textScale()) + 1;
                height = (int) Math.ceil(10 * command.textScale());
            }
            left = Math.min(left, command.x());
            top = Math.min(top, command.y());
            right = Math.max(right, command.x() + Math.max(1, width));
            bottom = Math.max(bottom, command.y() + Math.max(1, height));
        }
        return new UiWindowManager.Rect(left, top, Math.max(1, right - left), Math.max(1, bottom - top));
    }

    private record Frame(List<HudRenderCommand> commands, UiWindowManager.Rect bounds) {}
}
