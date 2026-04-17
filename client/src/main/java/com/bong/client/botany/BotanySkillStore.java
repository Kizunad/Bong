package com.bong.client.botany;

public final class BotanySkillStore {
    private static volatile BotanySkillViewModel snapshot = BotanySkillViewModel.defaultView();

    private BotanySkillStore() {
    }

    public static BotanySkillViewModel snapshot() {
        return snapshot;
    }

    public static void replace(BotanySkillViewModel next) {
        snapshot = next == null ? BotanySkillViewModel.defaultView() : next;
    }

    public static void clearOnDisconnect() {
        snapshot = BotanySkillViewModel.defaultView();
    }

    public static void resetForTests() {
        snapshot = BotanySkillViewModel.defaultView();
    }
}
