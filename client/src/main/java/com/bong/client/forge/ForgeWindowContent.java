package com.bong.client.forge;

import com.bong.client.forge.screen.ConsecrationPanelComponent;
import com.bong.client.forge.screen.InscriptionPanelComponent;
import com.bong.client.forge.screen.TemperingTrackComponent;
import com.bong.client.network.ClientRequestProtocol.TemperBeat;
import com.bong.client.inventory.model.InventoryItem;
import com.bong.client.ui.window.UiWindowManager;
import io.wispforest.owo.ui.component.ButtonComponent;
import io.wispforest.owo.ui.component.Components;
import io.wispforest.owo.ui.container.Containers;
import io.wispforest.owo.ui.container.FlowLayout;
import io.wispforest.owo.ui.container.ScrollContainer;
import io.wispforest.owo.ui.core.Color;
import io.wispforest.owo.ui.core.Insets;
import io.wispforest.owo.ui.core.Sizing;
import io.wispforest.owo.ui.core.Surface;
import net.minecraft.text.Text;
import org.lwjgl.glfw.GLFW;

import java.util.Objects;

/** 锻造内容只投影领域状态，所有操作通过窗口所有者发送。 */
public final class ForgeWindowContent implements AutoCloseable {
    private final ForgeWindows windows;
    private final UiWindowManager.WindowState owner;
    private final FlowLayout root;
    private final ForgeWorkbenchComponent workbench;
    private FlowLayout body;
    private final ScrollContainer<FlowLayout> scroll;
    private final Runnable openCarrier;
    private ForgeViewModel rendered;
    private boolean available;
    private String feedback;
    private int layoutWidth = -1;
    private int layoutHeight = -1;
    private boolean compact;

    public ForgeWindowContent(FlowLayout root, ForgeWindows windows,
                              UiWindowManager.WindowState owner, Runnable openCarrier) {
        this.windows = windows;
        this.owner = owner;
        this.openCarrier = openCarrier;
        this.root = root;
        root.padding(Insets.of(4));
        root.gap(6);
        workbench = new ForgeWorkbenchComponent(windows);
        body = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
        body.gap(7);
        scroll = Containers.verticalScroll(Sizing.fill(100), Sizing.fill(100), body);
        scroll.id("forge-scroll");
        scroll.scrollbarThiccness(3);
        refresh();
    }

    public void layout(int width, int height) {
        if (width == layoutWidth && height == layoutHeight) return;
        layoutWidth = width;
        layoutHeight = height;
        compact = width < 470;
        // 改布局时先解除父子关系；同一个模型组件不能同时挂在两棵布局树中。
        if (workbench.parent() instanceof FlowLayout parent) parent.removeChild(workbench);
        root.clearChildren();
        int innerWidth = Math.max(1, width - 8);
        int innerHeight = Math.max(1, height - 8);
        if (compact) {
            scroll.sizing(Sizing.fill(100), Sizing.fixed(innerHeight));
            workbench.sizing(Sizing.fill(100), Sizing.fixed(Math.max(150, innerWidth * 2 / 3)));
            root.child(scroll);
        } else {
            var row = Containers.horizontalFlow(Sizing.fill(100), Sizing.fixed(innerHeight));
            row.gap(6);
            workbench.sizing(Sizing.fixed(innerWidth - 200), Sizing.fixed(innerHeight));
            scroll.sizing(Sizing.fixed(194), Sizing.fixed(innerHeight));
            row.child(workbench);
            row.child(scroll);
            root.child(row);
        }
        refresh();
    }

    public void tick() {
        if (!Objects.equals(rendered, windows.model()) || available != windows.available()
            || !Objects.equals(feedback, windows.feedback())) refresh();
    }

    private void refresh() {
        rendered = windows.model();
        available = windows.available();
        feedback = windows.feedback();
        workbench.refresh();
        // 一次替换完整内容，避免 clearChildren 的瞬时零高度将滚动位置弹回顶部。
        body.removeChild(workbench);
        body = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
        body.gap(7);
        if (compact) body.child(workbench);
        var station = rendered.station();
        var header = section("炼器砧 · " + station.tier() + " 阶");
        label(header, available ? "完整度 " + Math.round(station.integrity() * 100) + "%  ·  " + station.ownerName()
            : "工位已不可用，请靠近后按交互键重新开启。", 0xFFB1B9B3);
        label(header, rendered.session().active() ? "炉次已锁定 · 关闭后可回来继续" : "开炉前可取回 · 关闭时返还材料", 0xFF899B99);
        var blueprint = rendered.blueprint();
        if (blueprint != null) label(header, String.join(" → ", blueprint.steps().stream()
            .map(ForgeViewModel::stepLabel).toList()), 0xFFE7BB79);
        var controls = row();
        controls.child(button(workbench.camera().autoRotate() ? "停止旋转" : "自动旋转", "forge-rotate", 78,
            () -> workbench.camera().autoRotate(!workbench.camera().autoRotate()), true));
        controls.child(button("复位", "forge-reset-view", 54, () -> workbench.camera().reset(), true));
        header.child(controls);
        if (!feedback.isBlank()) label(body, feedback, 0xFFE7BB79);
        var outcome = windows.outcome();
        if (outcome.sessionId() > 0) {
            var result = section("本炉结果");
            label(result, ForgeWorkbenchComponent.resultLabel(outcome.bucket()), 0xFFE7BB79);
            label(result, "品阶  " + outcome.achievedTier() + " 阶", 0xFFB1B9B3);
            label(result, "品质  " + Math.round(outcome.quality() * 100) + "%", 0xFFB1B9B3);
            if (!outcome.sideEffectsCsv().isBlank()) label(result, outcome.sideEffectsCsv(), 0xFFB1B9B3);
        }
        if (rendered.session().active()) activeStep();
        else prepare();
        body.child(button("暗器注入", "forge-carrier", 84, openCarrier, available));
        scroll.child(body);
    }

    private void prepare() {
        var book = section("图谱");
        var blueprint = rendered.blueprint();
        label(book, blueprint == null ? "尚未学会锻造图谱" : blueprint.displayName(), 0xFFE9D7B6);
        if (blueprint != null) label(book, blueprint.stepCount() + " 道工序 · 上限 " + blueprint.tierCap() + " 阶",
            0xFFB1B9B3);
        var pages = row();
        pages.child(button("上一页", "forge-prev", 64, () -> windows.turnPage(-1),
            ready() && rendered.preparedMaterials().isEmpty() && rendered.page() > 0));
        pages.child(button("下一页", "forge-next", 64, () -> windows.turnPage(1),
            ready() && rendered.preparedMaterials().isEmpty() && rendered.page() + 1 < rendered.blueprints().size()));
        book.child(pages);
        if (blueprint != null) {
            var needs = section("本炉所需");
            for (var requirement : blueprint.requiredMaterials()) {
                int count = rendered.preparedMaterials().stream()
                    .filter(item -> item.forgeMaterialKey().equals(requirement.material()))
                    .mapToInt(InventoryItem::stackCount).sum();
                String name = java.util.stream.Stream.concat(rendered.preparedMaterials().stream(), rendered.materials().stream())
                    .filter(item -> item.forgeMaterialKey().equals(requirement.material()))
                    .map(InventoryItem::displayName).findFirst().orElse(requirement.material());
                label(needs, name + "  " + count + " / " + requirement.count(), count >= requirement.count() ? 0xFFADCDB6 : 0xFFE7BB79);
            }
        }
        var materials = section("炉中暂存");
        label(materials, "将背包材料拖入炉口。多余的必需材料会在开炉时返还。", 0xFFB1A798);
        if (rendered.preparedMaterials().isEmpty()) label(materials, "炉中尚无材料", 0xFF899B99);
        for (var item : rendered.preparedMaterials()) {
            label(materials, item.displayName() + " × " + item.stackCount(), 0xFFE0DACA);
            materials.child(button("取回", "forge-return-" + item.instanceId(), 64,
                () -> windows.material(item.instanceId(), true), ready()));
        }
        body.child(button("开炉 · 锁定投料", "forge-start", 144, windows::start, ready() && blueprint != null
            && !rendered.preparedMaterials().isEmpty()));
    }

    private void activeStep() {
        var session = rendered.session();
        var step = section(rendered.stepLabel());
        label(step, session.blueprintName() + " · 已达 " + session.achievedTier() + " 阶", 0xFFE9D7B6);
        switch (session.currentStep()) {
            case "billet" -> label(step, "坯料已入炉，可以推进下一道工序。", 0xFFB1B9B3);
            case "tempering" -> {
                var track = TemperingTrackComponent.renderStateFrom(session);
                label(step, "余下节拍：" + String.join(" · ", track.patternRemaining().stream()
                    .map(ForgeWindowContent::beatName).toList()), 0xFFE7BB79);
                label(step, "命中 " + track.hits() + "  失误 " + track.misses() + "  偏差 " + track.deviation(), 0xFFB1B9B3);
                var hits = row();
                hits.child(button("轻 J", "forge-light", 49, () -> windows.hit(TemperBeat.L), ready()));
                hits.child(button("重 K", "forge-heavy", 49, () -> windows.hit(TemperBeat.H), ready()));
                hits.child(button("折 L", "forge-fold", 49, () -> windows.hit(TemperBeat.F), ready()));
                step.child(hits);
            }
            case "inscription" -> {
                var slots = InscriptionPanelComponent.renderStateFrom(session);
                label(step, "铭文 " + slots.filledCount() + " / " + slots.maxSlots() + "  "
                    + slots.failChanceLabel(), 0xFFB1B9B3);
                if (slots.failed()) label(step, "铭文失败，可继续结算本炉。", 0xFFE7BB79);
                label(step, "从背包拖入铭文残卷，在器身刻下铭文。", 0xFFB1A798);
            }
            case "consecration" -> {
                var qi = ConsecrationPanelComponent.renderStateFrom(session, rendered.inventory().realm());
                label(step, "真元 " + qi.qiLabel() + " · " + qi.colorLabel(), 0xFFB1B9B3);
                if (!qi.realmAllowed()) label(step, qi.realmGateLabel(), 0xFFE7BB79);
                step.child(button(qi.isComplete() ? "已注满" : "按住注入真元", "forge-inject", 112,
                    windows::beginInjection, ready() && qi.canInject()));
            }
            default -> { }
        }
        step.child(button("推进 / 结算", "forge-advance", 112, windows::advance, ready()));
    }

    public boolean acceptsDrop(double x, double y, InventoryItem item) {
        if (!ready() || owner.closed() || owner.minimized()) return false;
        if (compact && (!scroll.isInBoundingBox(x, y) || x >= scroll.x() + scroll.width() - scroll.scrollbarThiccness())) return false;
        if (rendered.session().active()) {
            return "inscription".equals(rendered.session().currentStep()) && !item.inscriptionId().isBlank()
                && workbench.isInBoundingBox(x, y);
        }
        return rendered.blueprint() != null && workbench.furnaceAt(x, y);
    }

    public void drop(InventoryItem item) {
        if (rendered.session().active()) windows.inscribe(item.instanceId());
        else windows.material(item.instanceId(), false);
    }

    @Override public void close() { workbench.close(); }

    public boolean keyPressed(int key) {
        if (owner.closed() || owner.minimized() || !"tempering".equals(windows.model().session().currentStep())) return false;
        TemperBeat beat = switch (key) {
            case GLFW.GLFW_KEY_J -> TemperBeat.L;
            case GLFW.GLFW_KEY_K -> TemperBeat.H;
            case GLFW.GLFW_KEY_L -> TemperBeat.F;
            default -> null;
        };
        if (beat == null) return false;
        windows.hit(beat);
        return true;
    }

    private boolean ready() { return available && !windows.pending(); }

    private ButtonComponent button(String text, String id, int width, Runnable action, boolean enabled) {
        var button = Components.button(Text.literal(text), ignored -> {
            if (owner.closed() || owner.minimized()) return;
            action.run();
            refresh();
        });
        button.id(id);
        button.sizing(Sizing.fixed(width), Sizing.fixed(22));
        button.textShadow(false);
        button.renderer(ButtonComponent.Renderer.flat(0xFF463C32, 0xFF705B43, 0xFF252320));
        button.active(enabled);
        return button;
    }

    private FlowLayout section(String title) {
        var section = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
        section.padding(Insets.of(7));
        section.gap(5);
        section.surface(Surface.flat(0xDD201F1D).and(Surface.outline(0xFF4A4034)));
        label(section, title, 0xFFE7BB79);
        body.child(section);
        return section;
    }

    private static FlowLayout row() {
        var row = Containers.horizontalFlow(Sizing.fill(100), Sizing.content());
        row.gap(5);
        return row;
    }

    private static void label(FlowLayout parent, String text, int color) {
        var label = Components.label(Text.literal(text));
        label.color(Color.ofArgb(color));
        label.horizontalSizing(Sizing.fill(100));
        parent.child(label);
    }

    private static String beatName(String beat) {
        return switch (beat) {
            case "L" -> "轻";
            case "H" -> "重";
            case "F" -> "折";
            default -> beat;
        };
    }
}
