"""dev 命令链路 —— `/clearinv` 三分支 chat + inventory snapshot 反馈。

黑盒断言面 = `server/src/cmd/dev/clearinv.rs` 的 send_chat_message 反馈契约，
以及 PlayerInventory revision 变化后经 `bong:server_data/inventory_snapshot` 回推。
"""

from ._inventory_helpers import wait_inventory_revision_after, wait_join_and_inventory

DESCRIPTION = "/clearinv pack|all|naked 三分支都有 chat 反馈并回推 inventory_snapshot"
MODULES = ["cmd", "inventory"]

SCOPES = [
    ("pack", "PackOnly"),
    ("all", "PackAndHotbar"),
    ("naked", "All"),
]


def run(env) -> None:
    with env.new_bot("Clr") as bot:
        snapshot = wait_join_and_inventory(bot)
        revision = snapshot["revision"]

        for scope, debug_name in SCOPES:
            bot.cmd(f"clearinv {scope}")
            bot.expect_chat(f"[dev] clearinv {debug_name} revision=", timeout=10.0)
            snapshot = wait_inventory_revision_after(bot, revision, timeout=10.0)
            revision = snapshot["revision"]

        bot.assert_alive("clearinv 三分支执行后")
