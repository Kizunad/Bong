package com.bong.client.debug;

import com.mojang.brigadier.CommandDispatcher;
import com.mojang.brigadier.ParseResults;
import com.mojang.brigadier.context.CommandContext;
import com.mojang.brigadier.exceptions.CommandSyntaxException;
import net.fabricmc.fabric.api.client.command.v2.FabricClientCommandSource;
import net.minecraft.util.Identifier;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * 参数树结构回归测试——确认 id 用 {@link net.minecraft.command.argument.IdentifierArgumentType}
 * 而不是 {@code StringArgumentType.greedyString()}。greedyString 会把后续 priority / fade_in_ticks
 * 全部吞进 id，导致命令契约被破坏（命令历史里就发生过：{@code <id> <priority>} 文档在，实际只吃一个
 * 大字符串）——这个测试的意图就是防止回归到那个状态。
 *
 * <p>注意：只验证 parse 阶段的参数抽取，不执行命令——执行依赖运行时 {@code MinecraftClient.getInstance().player}，
 * 单测里没有游戏实例。parse 就足以覆盖本次修复的核心点：参数树是否能正确切分 id / priority /
 * fade_in_ticks 三段。
 */
public class BongAnimCommandTest {

    private CommandDispatcher<FabricClientCommandSource> buildDispatcher() {
        CommandDispatcher<FabricClientCommandSource> dispatcher = new CommandDispatcher<>();
        BongAnimCommand.registerTo(dispatcher);
        return dispatcher;
    }

    private CommandContext<FabricClientCommandSource> parseArgs(String input) {
        CommandDispatcher<FabricClientCommandSource> dispatcher = buildDispatcher();
        ParseResults<FabricClientCommandSource> parse = dispatcher.parse(input, null);
        assertTrue(parse.getExceptions().isEmpty(),
            "expected clean parse but got: " + parse.getExceptions());
        CommandContext<FabricClientCommandSource> ctx = parse.getContext().build(input);
        assertNotNull(ctx.getCommand(), "parser should have resolved a terminal executor for: " + input);
        return ctx;
    }

    @Test
    void testParsesIdOnly() {
        CommandContext<FabricClientCommandSource> ctx = parseArgs("anim test bong:foo");
        assertEquals(new Identifier("bong", "foo"), ctx.getArgument("id", Identifier.class));
        // priority / fade_in_ticks 未在输入里出现——Brigadier 不会把它们放进 context
        assertThrows(IllegalArgumentException.class, () -> ctx.getArgument("priority", Integer.class));
        assertThrows(IllegalArgumentException.class, () -> ctx.getArgument("fade_in_ticks", Integer.class));
    }

    @Test
    void testParsesIdPlusPriority() {
        CommandContext<FabricClientCommandSource> ctx = parseArgs("anim test bong:meditate_sit 500");
        assertEquals(new Identifier("bong", "meditate_sit"), ctx.getArgument("id", Identifier.class));
        assertEquals(500, ctx.getArgument("priority", Integer.class));
        assertThrows(IllegalArgumentException.class, () -> ctx.getArgument("fade_in_ticks", Integer.class));
    }

    @Test
    void testParsesIdPlusPriorityPlusFadeInTicks() {
        // 这个正是 greedyString 实现会失败的 case：如果 id 用了 greedyString，
        // "bong:foo 500 10" 会被一口气吞进 id，后续 int 参数永远不到
        CommandContext<FabricClientCommandSource> ctx = parseArgs("anim test bong:sword_swing_horiz 1500 8");
        assertEquals(new Identifier("bong", "sword_swing_horiz"), ctx.getArgument("id", Identifier.class));
        assertEquals(1500, ctx.getArgument("priority", Integer.class));
        assertEquals(8, ctx.getArgument("fade_in_ticks", Integer.class));
    }

    @Test
    void stopParsesIdentifier() {
        CommandContext<FabricClientCommandSource> ctx = parseArgs("anim stop bong:meditate_sit");
        assertEquals(new Identifier("bong", "meditate_sit"), ctx.getArgument("id", Identifier.class));
    }

    @Test
    void rejectsInvalidIdentifier() {
        CommandDispatcher<FabricClientCommandSource> dispatcher = buildDispatcher();
        // 带非法字符的 id 应在 parse 阶段就被 IdentifierArgumentType 拒绝，
        // 而不是走到 executes 里再 try/catch InvalidIdentifierException
        ParseResults<FabricClientCommandSource> parse = dispatcher.parse("anim test BONG:FOO!", null);
        assertFalse(parse.getExceptions().isEmpty(), "bad identifier should produce a parse exception");
    }

    @Test
    void rejectsFadeTicksOutOfRange() throws CommandSyntaxException {
        CommandDispatcher<FabricClientCommandSource> dispatcher = buildDispatcher();
        // IntegerArgumentType.integer(0, 40) 上限 40，41 应被 parse 拒
        ParseResults<FabricClientCommandSource> parse = dispatcher.parse("anim test bong:foo 1000 41", null);
        assertFalse(parse.getExceptions().isEmpty(), "fade_in_ticks=41 should be rejected");
    }

    @Test
    void rejectsNegativePriority() {
        CommandDispatcher<FabricClientCommandSource> dispatcher = buildDispatcher();
        ParseResults<FabricClientCommandSource> parse = dispatcher.parse("anim test bong:foo -5", null);
        assertFalse(parse.getExceptions().isEmpty(), "priority=-5 should be rejected");
    }
}
