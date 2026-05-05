package com.bong.client.processing;

import com.bong.client.processing.state.ProcessingSessionStore;
import io.wispforest.owo.ui.base.BaseOwoScreen;
import io.wispforest.owo.ui.component.Components;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.core.Color;
import io.wispforest.owo.ui.core.CursorStyle;
import io.wispforest.owo.ui.core.HorizontalAlignment;
import io.wispforest.owo.ui.core.Insets;
import io.wispforest.owo.ui.core.OwoUIAdapter;
import io.wispforest.owo.ui.core.Sizing;
import io.wispforest.owo.ui.core.Surface;
import io.wispforest.owo.ui.core.VerticalAlignment;
import net.minecraft.text.Text;

import java.util.List;

/**
 * plan-lingtian-process-v1 P3 — 四类加工统合浮窗。
 *
 * <p>屏幕只展示当前可选工艺和 active session 进度。启动 intent 的网络协议留给
 * 后续 inventory/forge 操作切片补齐；本 plan 先把 UI 结构和进度数据面固定。</p>
 */
public final class ProcessingActionScreen extends BaseOwoScreen<FlowLayout> {
    private static final Text TITLE = Text.literal("作物加工");
    private static final int PANEL_W = 320;
    private static final int PANEL_H = 220;

    public ProcessingActionScreen() {
        super(TITLE);
    }

    @Override
    protected OwoUIAdapter<FlowLayout> createAdapter() {
        return OwoUIAdapter.create(this, Containers::verticalFlow);
    }

    @Override
    protected void build(FlowLayout root) {
        root.surface(Surface.VANILLA_TRANSLUCENT);
        root.horizontalAlignment(HorizontalAlignment.CENTER);
        root.verticalAlignment(VerticalAlignment.CENTER);

        FlowLayout panel = Containers.verticalFlow(Sizing.fixed(PANEL_W), Sizing.fixed(PANEL_H));
        panel.surface(Surface.flat(0xFF111816).and(Surface.outline(0xFF5A735F)));
        panel.padding(Insets.of(8));
        panel.gap(6);
        panel.horizontalAlignment(HorizontalAlignment.CENTER);

        panel.child(Components.label(Text.literal("§f§l作物加工")));
        panel.child(kindRow());
        panel.child(Components.label(Text.literal(formatProgress(ProcessingSessionStore.snapshot()))));
        panel.child(Components.label(Text.literal("§7输入 0 · 输出 0")));
        root.child(panel);
    }

    private FlowLayout kindRow() {
        FlowLayout row = Containers.horizontalFlow(Sizing.fill(100), Sizing.content());
        row.gap(4);
        row.horizontalAlignment(HorizontalAlignment.CENTER);
        for (String label : visibleKindLabelsForTests()) {
            row.child(button(label));
        }
        return row;
    }

    private FlowLayout button(String text) {
        LabelComponent lbl = Components.label(Text.literal(text));
        lbl.color(Color.ofArgb(0xFFE8E8E8));
        FlowLayout wrap = Containers.horizontalFlow(Sizing.content(), Sizing.content());
        wrap.padding(Insets.of(3, 3, 7, 7));
        wrap.surface(Surface.flat(0xFF223027).and(Surface.outline(0xFF78906F)));
        wrap.cursorStyle(CursorStyle.HAND);
        wrap.child(lbl);
        return wrap;
    }

    public static List<String> visibleKindLabelsForTests() {
        return List.of("晾晒", "碾粉", "炮制", "萃取");
    }

    public static String formatProgress(ProcessingSessionStore.Snapshot snapshot) {
        if (snapshot == null || !snapshot.active()) {
            return "§8当前无加工";
        }
        int percent = Math.round(snapshot.progress() * 100.0f);
        return "§a" + snapshot.kind().label() + " " + percent + "% §7"
            + snapshot.progressTicks() + "/" + snapshot.durationTicks();
    }
}
