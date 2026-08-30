package com.bong.client.combat.screen;

import com.bong.client.combat.TerminateClientIntentSink;
import com.bong.client.combat.TerminateIntent;
import com.bong.client.combat.store.TerminateStateStore;
import com.bong.client.ui.adapter.owo.OwoXmlScreenHost;
import com.bong.client.ui.intent.UiIntentSink;
import io.wispforest.owo.ui.component.ButtonComponent;
import io.wispforest.owo.ui.component.LabelComponent;
import io.wispforest.owo.ui.container.FlowLayout;
import net.minecraft.text.Text;

import java.util.Objects;
import java.util.stream.Collectors;

/**
 * Final termination overlay (plan §U4). Displays final words + epilogue + a
 * "create new character" button.
 */
public final class TerminateScreen extends OwoXmlScreenHost<FlowLayout> {
    private final TerminateStateStore.State state;
    private final UiIntentSink<TerminateIntent> intentSink;
    private LabelComponent finalWordsLabel;
    private LabelComponent epilogueLabel;
    private LabelComponent archetypeLabel;

    public TerminateScreen(TerminateStateStore.State state) {
        this(state, TerminateClientIntentSink.production());
    }

    TerminateScreen(
        TerminateStateStore.State state,
        UiIntentSink<TerminateIntent> intentSink
    ) {
        super(Text.literal("\u7ec8\u7ed3"), FlowLayout.class, "terminate");
        this.state = state == null ? TerminateStateStore.State.HIDDEN : state;
        this.intentSink = Objects.requireNonNull(intentSink, "intentSink must not be null");
    }

    @Override
    public boolean shouldPause() { return true; }

    @Override
    public boolean shouldCloseOnEsc() { return false; }

    /** XML 负责布局；这里仅绑定服务端快照和 typed button callback。 */
    @Override
    protected void bindTemplate(FlowLayout root) {
        label("terminate-title");
        finalWordsLabel = label("terminate-final-words");
        epilogueLabel = label("terminate-epilogue");
        archetypeLabel = label("terminate-archetype");

        finalWordsLabel.text(Text.literal(formatFinalWords(state.finalWords())));
        epilogueLabel.text(Text.literal(state.epilogue()));
        archetypeLabel.text(Text.literal(formatArchetype(state.archetypeSuggestion())));

        component(ButtonComponent.class, "terminate-create-character")
            .onPress(button -> intentSink.dispatch(new TerminateIntent.CreateNewCharacter()));
    }

    @Override
    public void tick() {
        super.tick();
        if (!TerminateStateStore.snapshot().visible()) {
            this.close();
        }
    }

    static String formatFinalWords(String raw) {
        if (raw == null || raw.isBlank()) {
            return "";
        }
        return raw.lines()
            .filter(line -> !line.isBlank())
            .collect(Collectors.joining("\n"));
    }

    static String formatArchetype(String raw) {
        return raw == null || raw.isBlank() ? "" : "\u5efa\u8bae\u65b0\u89d2\u8272\u539f\u578b: " + raw;
    }

    TerminateStateStore.State stateForTests() { return state; }
}
