"""Bong 协议级黑盒 Bot e2e 框架（AGENTS.md §15）。

- mc_protocol.py — MC 763 offline 传输/编解码底座
- bot.py — Bot 动作/观察/断言 API
- scenarios/ — 每模块一个（或多个）场景；模块更新必须同步配场景
- run_scenarios.py — CLI runner（CI 由 scripts/bot-e2e.sh 调用）
"""
