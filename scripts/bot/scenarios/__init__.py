"""Bot e2e 场景包。

场景模块契约（runner 按此发现和执行）：
- 模块级常量 ``DESCRIPTION: str`` —— 一句话说明锁的是什么行为
- 模块级常量 ``MODULES: list[str]`` —— 覆盖的 server 模块（review 时对照"改了模块要配场景"）
- ``def run(env) -> None`` —— 用 env.new_bot(tag) 建 bot，断言失败抛 BotAssertionError

命名：``<module>_<behavior>.py``，下划线开头的文件不会被 runner 发现。
"""
