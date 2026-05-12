package com.bong.client.ui;

import com.bong.client.alchemy.AlchemyScreen;
import com.bong.client.combat.screen.DeathScreen;
import com.bong.client.combat.screen.ForgeCarrierScreen;
import com.bong.client.combat.screen.RepairScreen;
import com.bong.client.combat.screen.TerminateScreen;
import com.bong.client.combat.screen.ZhenfaLayoutScreen;
import com.bong.client.cultivation.voidaction.VoidActionScreen;
import com.bong.client.forge.ForgeScreen;
import com.bong.client.identity.IdentityPanelScreen;
import com.bong.client.inspect.ItemInspectScreen;
import com.bong.client.insight.InsightOfferScreen;
import com.bong.client.inventory.InspectScreen;
import com.bong.client.lingtian.LingtianActionScreen;
import com.bong.client.processing.ProcessingActionScreen;
import com.bong.client.social.SparringInviteScreen;
import com.bong.client.social.TradeOfferScreen;
import net.minecraft.client.gui.screen.DownloadingTerrainScreen;
import net.minecraft.client.gui.screen.GameMenuScreen;
import net.minecraft.client.gui.screen.Screen;

import java.util.Map;
import java.util.Optional;
import java.util.concurrent.ConcurrentHashMap;

public final class ScreenTransitionRegistry {
    private static final Map<Class<? extends Screen>, TransitionConfig> CONFIGS = new ConcurrentHashMap<>();
    private static final Map<ScreenPair, TransitionConfig.TransitionSpec> CHAINS = new ConcurrentHashMap<>();
    private static volatile boolean defaultsBootstrapped;

    private ScreenTransitionRegistry() {
    }

    public static void bootstrapDefaults() {
        if (defaultsBootstrapped) {
            return;
        }
        synchronized (ScreenTransitionRegistry.class) {
            if (defaultsBootstrapped) {
                return;
            }
            bootstrapDefaultsLocked();
            defaultsBootstrapped = true;
        }
    }

    private static void bootstrapDefaultsLocked() {
        register(DownloadingTerrainScreen.class, TransitionConfig.of(
            DownloadingTerrainScreen.class, ScreenTransition.Type.NONE, 0, ScreenTransition.Type.NONE, 0
        ));
        register(InspectScreen.class, TransitionConfig.of(
            InspectScreen.class, ScreenTransition.Type.SLIDE_UP, 300, ScreenTransition.Type.SLIDE_DOWN, 300
        ));
        register(ForgeScreen.class, TransitionConfig.of(
            ForgeScreen.class, ScreenTransition.Type.SCALE_UP, 400, ScreenTransition.Type.SCALE_DOWN, 400
        ));
        register(AlchemyScreen.class, new TransitionConfig(
            AlchemyScreen.class,
            ScreenTransition.Type.SCALE_UP,
            400,
            ScreenTransition.Type.FADE,
            400,
            ScreenTransition.Easing.EASE_OUT_QUAD,
            TransitionConfig.OverlayStyle.FOG,
            false
        ));
        register(CultivationScreen.class, new TransitionConfig(
            CultivationScreen.class,
            ScreenTransition.Type.FADE,
            600,
            ScreenTransition.Type.FADE,
            600,
            ScreenTransition.Easing.EASE_OUT_CUBIC,
            TransitionConfig.OverlayStyle.VIGNETTE,
            false
        ));
        register(ItemInspectScreen.class, TransitionConfig.of(
            ItemInspectScreen.class, ScreenTransition.Type.SLIDE_RIGHT, 200, ScreenTransition.Type.SLIDE_LEFT, 200
        ));
        register(GameMenuScreen.class, TransitionConfig.of(
            GameMenuScreen.class, ScreenTransition.Type.FADE, 150, ScreenTransition.Type.FADE, 150
        ));
        register(TradeOfferScreen.class, TransitionConfig.of(
            TradeOfferScreen.class, ScreenTransition.Type.SLIDE_RIGHT, 200, ScreenTransition.Type.FADE, 200
        ));
        register(SparringInviteScreen.class, TransitionConfig.of(
            SparringInviteScreen.class, ScreenTransition.Type.FADE, 250, ScreenTransition.Type.FADE, 200
        ));
        register(InsightOfferScreen.class, TransitionConfig.of(
            InsightOfferScreen.class, ScreenTransition.Type.FADE, 250, ScreenTransition.Type.FADE, 200
        ));
        register(ProcessingActionScreen.class, TransitionConfig.of(
            ProcessingActionScreen.class, ScreenTransition.Type.SCALE_UP, 300, ScreenTransition.Type.FADE, 200
        ));
        register(IdentityPanelScreen.class, TransitionConfig.of(
            IdentityPanelScreen.class, ScreenTransition.Type.FADE, 250, ScreenTransition.Type.FADE, 200
        ));
        register(LingtianActionScreen.class, TransitionConfig.of(
            LingtianActionScreen.class, ScreenTransition.Type.FADE, 300, ScreenTransition.Type.FADE, 200
        ));
        register(VoidActionScreen.class, TransitionConfig.of(
            VoidActionScreen.class, ScreenTransition.Type.FADE, 300, ScreenTransition.Type.FADE, 200
        ));
        register(ForgeCarrierScreen.class, TransitionConfig.of(
            ForgeCarrierScreen.class, ScreenTransition.Type.SCALE_UP, 400, ScreenTransition.Type.SCALE_DOWN, 300
        ));
        register(RepairScreen.class, TransitionConfig.of(
            RepairScreen.class, ScreenTransition.Type.SCALE_UP, 300, ScreenTransition.Type.FADE, 200
        ));
        register(ZhenfaLayoutScreen.class, new TransitionConfig(
            ZhenfaLayoutScreen.class,
            ScreenTransition.Type.FADE,
            500,
            ScreenTransition.Type.FADE,
            300,
            ScreenTransition.Easing.EASE_OUT_CUBIC,
            TransitionConfig.OverlayStyle.PURPLE_TINT,
            false
        ));
        register(DeathScreen.class, new TransitionConfig(
            DeathScreen.class,
            ScreenTransition.Type.NONE,
            0,
            ScreenTransition.Type.NONE,
            0,
            ScreenTransition.Easing.LINEAR,
            TransitionConfig.OverlayStyle.NONE,
            true
        ));
        register(TerminateScreen.class, TransitionConfig.of(
            TerminateScreen.class, ScreenTransition.Type.FADE, 200, ScreenTransition.Type.FADE, 100
        ));
        registerChain(SparringInviteScreen.class, TradeOfferScreen.class, new TransitionConfig.TransitionSpec(
            ScreenTransition.Type.SLIDE_RIGHT,
            200,
            ScreenTransition.Easing.EASE_OUT_CUBIC,
            TransitionConfig.OverlayStyle.NONE,
            false
        ));
    }

    public static void register(Class<? extends Screen> screenClass, TransitionConfig config) {
        if (screenClass == null || config == null) {
            return;
        }
        CONFIGS.put(screenClass, config);
    }

    public static void registerChain(
        Class<? extends Screen> from,
        Class<? extends Screen> to,
        TransitionConfig.TransitionSpec spec
    ) {
        if (from == null || to == null || spec == null) {
            return;
        }
        CHAINS.put(new ScreenPair(from, to), spec);
    }

    public static Optional<TransitionConfig> get(Class<?> screenClass) {
        if (screenClass == null) {
            return Optional.empty();
        }
        return Optional.ofNullable(findConfig(screenClass));
    }

    public static TransitionConfig getOrDefault(Class<?> screenClass) {
        TransitionConfig config = findConfig(screenClass);
        return config == null ? TransitionConfig.DEFAULT_FADE_200MS : config;
    }

    public static TransitionConfig.TransitionSpec resolve(Screen oldScreen, Screen newScreen) {
        bootstrapDefaults();
        if (oldScreen != null && newScreen != null) {
            TransitionConfig.TransitionSpec chained = findChain(oldScreen.getClass(), newScreen.getClass());
            if (chained != null) {
                return chained;
            }
        }
        if (newScreen == null) {
            return getOrDefault(oldScreen == null ? null : oldScreen.getClass()).closeSpec();
        }
        return getOrDefault(newScreen.getClass()).openSpec();
    }

    public static TransitionConfig.TransitionSpec preview(Screen oldScreen, Screen newScreen) {
        return resolve(oldScreen, newScreen);
    }

    static void resetForTests() {
        CONFIGS.clear();
        CHAINS.clear();
        defaultsBootstrapped = false;
    }

    private static TransitionConfig findConfig(Class<?> screenClass) {
        Class<?> cursor = screenClass;
        while (cursor != null && Screen.class.isAssignableFrom(cursor)) {
            @SuppressWarnings("unchecked")
            TransitionConfig config = CONFIGS.get((Class<? extends Screen>) cursor);
            if (config != null) {
                return config;
            }
            cursor = cursor.getSuperclass();
        }
        return null;
    }

    private static TransitionConfig.TransitionSpec findChain(Class<?> from, Class<?> to) {
        // Chain overrides intentionally match by class hierarchy, not by screen instance identity.
        Class<?> fromCursor = from;
        while (fromCursor != null && Screen.class.isAssignableFrom(fromCursor)) {
            Class<?> toCursor = to;
            while (toCursor != null && Screen.class.isAssignableFrom(toCursor)) {
                @SuppressWarnings("unchecked")
                ScreenPair pair = new ScreenPair((Class<? extends Screen>) fromCursor, (Class<? extends Screen>) toCursor);
                TransitionConfig.TransitionSpec spec = CHAINS.get(pair);
                if (spec != null) {
                    return spec;
                }
                toCursor = toCursor.getSuperclass();
            }
            fromCursor = fromCursor.getSuperclass();
        }
        return null;
    }

    private record ScreenPair(Class<? extends Screen> from, Class<? extends Screen> to) {
    }
}
