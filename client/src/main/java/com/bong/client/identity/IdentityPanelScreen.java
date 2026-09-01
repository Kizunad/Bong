package com.bong.client.identity;

import com.bong.client.ui.adapter.owo.OwoXmlScreenHost;
import com.bong.client.ui.contract.UiScreenScope;
import com.bong.client.ui.contract.UiStateSource;
import com.bong.client.ui.intent.UiIntentResult;
import com.bong.client.ui.intent.UiIntentSink;
import io.wispforest.owo.ui.component.ButtonComponent;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.component.TextBoxComponent;
import io.wispforest.owo.ui.container.FlowLayout;
import net.minecraft.client.MinecraftClient;
import net.minecraft.text.Text;

import java.util.Objects;

/** 身份面板；XML 声明布局，Java 只绑定状态、行按钮和 typed intent。 */
public final class IdentityPanelScreen extends OwoXmlScreenHost<FlowLayout> {
    private static final int MAX_VISIBLE_IDENTITIES = 6;
    private static final Text EMPTY_TEXT = Text.literal(" ");

    private final UiStateSource<IdentityPanelState> stateSource;
    private final UiIntentSink<IdentityPanelIntent> intentSink;
    private IdentityPanelState state;
    private TextBoxComponent nameField;
    private LabelComponent cooldownLabel;
    private LabelComponent emptyLabel;
    private LabelComponent overflowLabel;
    private final LabelComponent[] entryLabels = new LabelComponent[MAX_VISIBLE_IDENTITIES];
    private final ButtonComponent[] switchButtons = new ButtonComponent[MAX_VISIBLE_IDENTITIES];

    public IdentityPanelScreen(
        UiStateSource<IdentityPanelState> stateSource,
        UiIntentSink<IdentityPanelIntent> intentSink
    ) {
        super(Text.literal("身份"), FlowLayout.class, "identity-panel");
        this.stateSource = Objects.requireNonNull(stateSource, "stateSource must not be null");
        this.intentSink = Objects.requireNonNull(intentSink, "intentSink must not be null");
        this.state = stateSource.snapshot();
    }

    @Override
    public boolean shouldPause() {
        return false;
    }

    /** XML 已声明六行固定身份槽；这里只挂载按钮动作并填充首帧快照。 */
    @Override
    protected void bindTemplate(FlowLayout root) {
        nameField = component(TextBoxComponent.class, "identity-name");
        nameField.setPlaceholder(Text.literal("新身份 / 改名"));
        cooldownLabel = label("identity-cooldown");
        emptyLabel = label("identity-empty");
        overflowLabel = label("identity-overflow");
        component(ButtonComponent.class, "identity-new")
            .onPress(button -> dispatchNew());
        component(ButtonComponent.class, "identity-rename")
            .onPress(button -> dispatchRename());
        for (int index = 0; index < MAX_VISIBLE_IDENTITIES; index++) {
            int row = index;
            entryLabels[index] = label("identity-entry-" + index);
            switchButtons[index] = component(ButtonComponent.class, "identity-switch-" + index)
                .onPress(button -> dispatchSwitch(row));
        }
        refresh(state);
    }

    @Override
    protected void onHostOpened(UiScreenScope scope) {
        var subscription = stateSource.subscribe(next -> {
            Runnable refresh = () -> refresh(next);
            MinecraftClient client = MinecraftClient.getInstance();
            if (client != null) {
                client.execute(refresh);
            } else {
                refresh.run();
            }
        });
        // Store 监听器属于屏幕生命周期，关闭时必须先撤销再释放 owo 组件树。
        scope.addCleanup(subscription::close);
    }

    private void dispatchNew() {
        dispatchName(true);
    }

    private void dispatchRename() {
        dispatchName(false);
    }

    private void dispatchName(boolean create) {
        String rawName = nameField == null ? "" : nameField.getText();
        String normalized = rawName == null ? "" : rawName.trim().replaceAll("\\s+", " ");
        if (normalized.isEmpty()) {
            return;
        }
        dispatch(create
            ? new IdentityPanelIntent.NewIdentity(normalized)
            : new IdentityPanelIntent.RenameIdentity(normalized));
    }

    private void dispatchSwitch(int row) {
        if (row < 0 || row >= state.identities().size()) {
            return;
        }
        dispatch(new IdentityPanelIntent.SwitchIdentity(state.identities().get(row).identityId()));
    }

    private void dispatch(IdentityPanelIntent intent) {
        UiIntentResult result = intentSink.dispatch(intent);
        if (result.kind() == UiIntentResult.Kind.LOCAL_ACCEPTED) {
            closeIfCurrentScreen();
        }
    }

    private void refresh(IdentityPanelState next) {
        state = next == null ? IdentityPanelState.empty() : next;
        if (cooldownLabel == null) {
            return;
        }
        cooldownLabel.text(Text.literal(cooldownLine(state)));
        emptyLabel.text(state.identities().isEmpty() ? Text.literal("暂无身份数据") : EMPTY_TEXT);
        int count = Math.min(state.identities().size(), MAX_VISIBLE_IDENTITIES);
        for (int index = 0; index < MAX_VISIBLE_IDENTITIES; index++) {
            if (index >= count) {
                // owo 0.11.2 的空 LabelComponent 在 tooltip 命中时会访问 -1 样式索引；
                // 用不可见空格保留合法文本样式，视觉上仍等同于空行。
                entryLabels[index].text(EMPTY_TEXT);
                switchButtons[index].setMessage(EMPTY_TEXT);
                switchButtons[index].active(false);
                continue;
            }
            IdentityPanelEntry entry = state.identities().get(index);
            entryLabels[index].text(Text.literal(formatEntryLine(entry, state.activeIdentityId())));
            switchButtons[index].setMessage(
                Text.literal(entry.identityId() == state.activeIdentityId() ? "当前" : "切换")
            );
            switchButtons[index].active(
                entry.identityId() != state.activeIdentityId() && state.cooldownPassed()
            );
        }
        int overflow = state.identities().size() - MAX_VISIBLE_IDENTITIES;
        overflowLabel.text(overflow > 0 ? Text.literal("另有 " + overflow + " 个身份") : EMPTY_TEXT);
    }

    private void closeIfCurrentScreen() {
        MinecraftClient client = MinecraftClient.getInstance();
        if (client != null && client.currentScreen == this) {
            client.setScreen(null);
        }
    }

    static String switchIdentityCommand(int identityId) {
        return IdentityPanelIntent.command(new IdentityPanelIntent.SwitchIdentity(identityId));
    }

    static String newIdentityCommand(String rawName) {
        return commandWithName("identity new", rawName);
    }

    static String renameIdentityCommand(String rawName) {
        return commandWithName("identity rename", rawName);
    }

    static String formatEntryLine(IdentityPanelEntry entry, int activeIdentityId) {
        String marker = entry.identityId() == activeIdentityId ? "*" : " ";
        String frozen = entry.frozen() ? " [冷藏]" : "";
        return marker + " #" + entry.identityId() + " " + entry.displayName() + frozen;
    }

    private static String commandWithName(String prefix, String rawName) {
        String name = rawName == null ? "" : rawName.trim().replaceAll("\\s+", " ");
        return name.isEmpty() ? "" : prefix + " " + name;
    }

    private static String cooldownLine(IdentityPanelState state) {
        return state.cooldownPassed()
            ? "切换冷却：可用"
            : "切换冷却：" + state.cooldownRemainingTicks() + " ticks";
    }

    IdentityPanelState stateForTests() {
        return state;
    }
}
