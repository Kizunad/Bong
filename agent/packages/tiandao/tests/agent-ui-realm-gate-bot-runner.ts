/**
 * Bot e2e adapter around the production AgentUiRuntime.
 *
 * This process uses real Redis clients and the production UiRenderer +
 * UiResponseConsumer. It deliberately supplies a stale high-realm player
 * snapshot while the connected protocol Bot remains Awaken on the server:
 * UiRenderer therefore publishes the clear realm-gated panel, the server
 * authoritatively rejects it, and UiResponseConsumer publishes the private
 * system-warning narration back to the server.
 */

import Redis from "ioredis";
import { AgentUiRuntime } from "../src/ui/agentUiRuntime.js";

const redisUrl = process.env.REDIS_URL ?? "redis://127.0.0.1:6379";
const targetPlayer = process.env.TARGET_PLAYER;
const targetName = process.env.TARGET_NAME;

if (!targetPlayer || !targetName) {
  throw new Error("TARGET_PLAYER and TARGET_NAME are required");
}

const IORedisCtor = ((Redis as unknown as { default?: unknown }).default ??
  Redis) as new (url: string) => {
  publish(channel: string, message: string): Promise<number>;
  subscribe(channel: string): Promise<unknown>;
  on(event: string, listener: (channel: string, message: string) => void): unknown;
  off(event: string, listener: (channel: string, message: string) => void): unknown;
  unsubscribe(): Promise<unknown>;
  disconnect(): void;
};

const commandPub = new IORedisCtor(redisUrl);
const responseSub = new IORedisCtor(redisUrl);
const narrationPub = new IORedisCtor(redisUrl);
const runtime = new AgentUiRuntime({
  pub: commandPub,
  sub: responseSub,
  narPub: narrationPub,
  logger: { info: () => undefined, warn: () => undefined },
});

const delay = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

try {
  await runtime.connect();
  const rendered = await runtime.triggerUi({
    scenario: "tiandao_revelation",
    targetPlayer: {
      uuid: targetPlayer,
      name: targetName,
      // Intentionally stale: production renderer sends a clear realm_gate=5
      // panel, while the authoritative server-side Bot is kept at Awaken.
      realm: "Spirit",
      composite_power: 0.5,
      breakdown: { combat: 0.5, wealth: 0.5, social: 0.5, karma: 0, territory: 0.5 },
      trend: "stable",
      active_hours: 1,
      zone: "spawn",
      pos: [0, 64, 0],
      recent_kills: 0,
      recent_deaths: 0,
    },
    params: { tiandao_message: "bot e2e stale realm snapshot" },
  });

  if (rendered.sentBlurVersion || rendered.command.realm_gate !== 5) {
    throw new Error(
      `expected production renderer clear realm_gate=5 command: ${JSON.stringify(rendered)}`,
    );
  }

  const deadline = Date.now() + 20_000;
  while (
    runtime.stats.realmGateRejected !== 1 ||
    runtime.stats.narrationPublished !== 1
  ) {
    if (Date.now() >= deadline) {
      throw new Error(`timed out waiting for realm gate response: ${JSON.stringify(runtime.stats)}`);
    }
    await delay(25);
  }

  process.stdout.write(
    `${JSON.stringify({
      request_id: rendered.requestId,
      target_player: rendered.command.target_player,
      realm_gate: rendered.command.realm_gate,
      sent_blur_version: rendered.sentBlurVersion,
      stats: runtime.stats,
    })}\n`,
  );
} finally {
  await runtime.disconnect();
  commandPub.disconnect();
}
