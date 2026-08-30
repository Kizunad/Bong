package com.bong.client.combat;

import com.bong.client.ui.contract.UiIntent;

/** 终结屏允许发送的 typed action。 */
public sealed interface TerminateIntent extends UiIntent permits TerminateIntent.CreateNewCharacter {
    record CreateNewCharacter() implements TerminateIntent {
    }
}
