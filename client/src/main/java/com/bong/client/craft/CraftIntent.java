package com.bong.client.craft;

import com.bong.client.ui.contract.UiIntent;

/** 手搓屏幕允许发送的全部用户动作。 */
public sealed interface CraftIntent extends UiIntent permits CraftIntent.Start, CraftIntent.Cancel {
    record Start(String recipeId, int quantity) implements CraftIntent {
    }

    record Cancel() implements CraftIntent {
    }
}
