package com.bong.client.network;

import com.bong.client.state.NarrationState;
import com.bong.client.state.PlayerStateViewModel;
import com.bong.client.state.RealmCollapseHudState;
import com.bong.client.state.SeasonState;
import com.bong.client.state.UiOpenState;
import com.bong.client.state.VisualEffectState;
import com.bong.client.state.ZoneState;
import net.minecraft.text.Text;

import java.util.List;
import java.util.Objects;
import java.util.Optional;

public final class ServerDataDispatch {
    private final String routeType;
    private final boolean handled;
    private final String logMessage;
    private final String legacyMessage;
    private final List<Text> chatMessages;
    private final NarrationState narrationState;
    private final NarrationState toastNarrationState;
    private final PlayerStateViewModel playerStateViewModel;
    private final SeasonState seasonState;
    private final ZoneState zoneState;
    private final VisualEffectState visualEffectState;
    private final ToastSpec alertToast;
    private final RealmCollapseHudState realmCollapseHudState;
    private final UiOpenState uiOpenState;

    private ServerDataDispatch(
        String routeType,
        boolean handled,
        String logMessage,
        String legacyMessage,
        List<Text> chatMessages,
        NarrationState narrationState,
        NarrationState toastNarrationState,
        PlayerStateViewModel playerStateViewModel,
        SeasonState seasonState,
        ZoneState zoneState,
        VisualEffectState visualEffectState,
        ToastSpec alertToast,
        RealmCollapseHudState realmCollapseHudState,
        UiOpenState uiOpenState
    ) {
        this.routeType = Objects.requireNonNull(routeType, "routeType");
        this.handled = handled;
        this.logMessage = Objects.requireNonNull(logMessage, "logMessage");
        this.legacyMessage = legacyMessage;
        this.chatMessages = List.copyOf(chatMessages == null ? List.of() : chatMessages);
        this.narrationState = sanitizeNarrationState(narrationState);
        this.toastNarrationState = sanitizeNarrationState(toastNarrationState);
        this.playerStateViewModel = sanitizePlayerStateViewModel(playerStateViewModel);
        this.seasonState = seasonState;
        this.zoneState = sanitizeZoneState(zoneState);
        this.visualEffectState = sanitizeVisualEffectState(visualEffectState);
        this.alertToast = sanitizeToastSpec(alertToast);
        this.realmCollapseHudState = sanitizeRealmCollapseHudState(realmCollapseHudState);
        this.uiOpenState = sanitizeUiOpenState(uiOpenState);
    }

    public static ServerDataDispatch handled(String routeType, String logMessage) {
        return new ServerDataDispatch(routeType, true, logMessage, null, List.of(), null, null, null, null, null, null, null, null, null);
    }

    public static ServerDataDispatch handledWithLegacyMessage(String routeType, String legacyMessage, String logMessage) {
        return new ServerDataDispatch(
            routeType,
            true,
            logMessage,
            Objects.requireNonNull(legacyMessage, "legacyMessage"),
            List.of(),
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null
        );
    }

    public static ServerDataDispatch handledWithNarration(
        String routeType,
        List<Text> chatMessages,
        NarrationState narrationState,
        NarrationState toastNarrationState,
        String logMessage
    ) {
        return new ServerDataDispatch(
            routeType,
            true,
            logMessage,
            null,
            chatMessages,
            narrationState,
            toastNarrationState,
            null,
            null,
            null,
            null,
            null,
            null,
            null
        );
    }

    public static ServerDataDispatch handledWithPlayerState(
        String routeType,
        PlayerStateViewModel playerStateViewModel,
        String logMessage
    ) {
        return new ServerDataDispatch(
            routeType,
            true,
            logMessage,
            null,
            List.of(),
            null,
            null,
            playerStateViewModel,
            null,
            null,
            null,
            null,
            null,
            null
        );
    }

    public static ServerDataDispatch handledWithPlayerStateAndSeason(
        String routeType,
        PlayerStateViewModel playerStateViewModel,
        SeasonState seasonState,
        String logMessage
    ) {
        return new ServerDataDispatch(
            routeType,
            true,
            logMessage,
            null,
            List.of(),
            null,
            null,
            playerStateViewModel,
            seasonState,
            null,
            null,
            null,
            null,
            null
        );
    }

    public static ServerDataDispatch handledWithZoneState(String routeType, ZoneState zoneState, String logMessage) {
        return new ServerDataDispatch(routeType, true, logMessage, null, List.of(), null, null, null, null, zoneState, null, null, null, null);
    }

    public static ServerDataDispatch handledWithEventAlert(
        String routeType,
        ToastSpec alertToast,
        VisualEffectState visualEffectState,
        RealmCollapseHudState realmCollapseHudState,
        String logMessage
    ) {
        return new ServerDataDispatch(routeType, true, logMessage, null, List.of(), null, null, null, null, null, visualEffectState, alertToast, realmCollapseHudState, null);
    }

    public static ServerDataDispatch handledWithEventAlert(
        String routeType,
        ToastSpec alertToast,
        VisualEffectState visualEffectState,
        String logMessage
    ) {
        return handledWithEventAlert(routeType, alertToast, visualEffectState, null, logMessage);
    }

    public static ServerDataDispatch handledWithUiOpen(String routeType, UiOpenState uiOpenState, String logMessage) {
        return new ServerDataDispatch(routeType, true, logMessage, null, List.of(), null, null, null, null, null, null, null, null, uiOpenState);
    }

    public static ServerDataDispatch noOp(String routeType, String logMessage) {
        return new ServerDataDispatch(routeType, false, logMessage, null, List.of(), null, null, null, null, null, null, null, null, null);
    }

    private static NarrationState sanitizeNarrationState(NarrationState narrationState) {
        if (narrationState == null || narrationState.isEmpty()) {
            return null;
        }
        return narrationState;
    }

    private static ZoneState sanitizeZoneState(ZoneState zoneState) {
        if (zoneState == null || zoneState.isEmpty()) {
            return null;
        }
        return zoneState;
    }

    private static PlayerStateViewModel sanitizePlayerStateViewModel(PlayerStateViewModel playerStateViewModel) {
        if (playerStateViewModel == null || playerStateViewModel.isEmpty()) {
            return null;
        }
        return playerStateViewModel;
    }

    private static ToastSpec sanitizeToastSpec(ToastSpec toastSpec) {
        if (toastSpec == null || toastSpec.isEmpty()) {
            return null;
        }
        return toastSpec;
    }

    private static VisualEffectState sanitizeVisualEffectState(VisualEffectState visualEffectState) {
        if (visualEffectState == null || visualEffectState.isEmpty()) {
            return null;
        }
        return visualEffectState;
    }

    private static RealmCollapseHudState sanitizeRealmCollapseHudState(RealmCollapseHudState realmCollapseHudState) {
        if (realmCollapseHudState == null || realmCollapseHudState.isEmpty()) {
            return null;
        }
        return realmCollapseHudState;
    }

    private static UiOpenState sanitizeUiOpenState(UiOpenState uiOpenState) {
        if (uiOpenState == null || uiOpenState.isEmpty()) {
            return null;
        }
        return uiOpenState;
    }

    public String routeType() {
        return routeType;
    }

    public boolean handled() {
        return handled;
    }

    public String logMessage() {
        return logMessage;
    }

    public Optional<String> legacyMessage() {
        return Optional.ofNullable(legacyMessage);
    }

    public List<Text> chatMessages() {
        return chatMessages;
    }

    public Optional<NarrationState> narrationState() {
        return Optional.ofNullable(narrationState);
    }

    public Optional<NarrationState> toastNarrationState() {
        return Optional.ofNullable(toastNarrationState);
    }

    public Optional<PlayerStateViewModel> playerStateViewModel() {
        return Optional.ofNullable(playerStateViewModel);
    }

    public Optional<SeasonState> seasonState() {
        return Optional.ofNullable(seasonState);
    }

    public Optional<ZoneState> zoneState() {
        return Optional.ofNullable(zoneState);
    }

    public Optional<VisualEffectState> visualEffectState() {
        return Optional.ofNullable(visualEffectState);
    }

    public Optional<ToastSpec> alertToast() {
        return Optional.ofNullable(alertToast);
    }

    public Optional<RealmCollapseHudState> realmCollapseHudState() {
        return Optional.ofNullable(realmCollapseHudState);
    }

    public Optional<UiOpenState> uiOpenState() {
        return Optional.ofNullable(uiOpenState);
    }

    public record ToastSpec(String text, int color, long durationMillis) {
        public ToastSpec {
            text = text == null ? "" : text.trim();
            durationMillis = Math.max(0L, durationMillis);
        }

        public boolean isEmpty() {
            return text.isEmpty() || durationMillis == 0L;
        }
    }
}
