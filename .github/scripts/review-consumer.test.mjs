import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

const workflowPath = new URL('../workflows/review-next.yml', import.meta.url);
const canaryWorkflowPath = new URL('../workflows/review-provider-canary.yml', import.meta.url);
const policyPath = new URL('../review-policy/bong.v1.json', import.meta.url);

async function workflow() {
  return readFile(workflowPath, 'utf8');
}

async function canaryWorkflow() {
  return readFile(canaryWorkflowPath, 'utf8');
}

async function policy() {
  return JSON.parse(await readFile(policyPath, 'utf8'));
}

test('shadow caller pins the central workflow and preserves the trusted trigger gate', async () => {
  const yaml = await workflow();
  assert.match(yaml, /^  issue_comment:\n    types: \[created\]$/m);
  assert.match(yaml, /^  workflow_dispatch:/m);
  assert.match(yaml, /^permissions: \{\}$/m);
  assert.match(yaml, /github\.event\.comment\.body == '\/review-next'/);
  assert.doesNotMatch(yaml, /startsWith\([^\n]*\/review-next/);
  assert.match(yaml, /\["OWNER","MEMBER","COLLABORATOR"\]/);
  assert.match(
    yaml,
    /uses: Kizunad\/review\/\.github\/workflows\/review\.yml@417e55e55737b8fe42803b97f85b59fce8bbfb2a/,
  );
  assert.doesNotMatch(yaml, /Kizunad\/review\/[^\n]*@(main|master|v?\d|[0-9a-f]{1,39})\b/);
  assert.match(yaml, /pr_number: \$\{\{ fromJSON\(github\.event\.issue\.number \|\| inputs\.pr_number\) \}\}/);
  assert.match(yaml, /shadow: true/);
  assert.match(yaml, /worker_timeout_ms: 120000/);
  assert.match(yaml, /circuit_manual_retry: \$\{\{ github\.event_name == 'workflow_dispatch' \}\}/);
  assert.match(yaml, /policy_path: \.github\/review-policy\/bong\.v1\.json/);
  assert.match(yaml, /review_base_url: \$\{\{ vars\.REVIEW_CLAUDE_BASE_URL \|\| 'https:\/\/api\.claudeopus\.world' \}\}/);
});

test('shadow caller maps only the existing Claude credential and grants the central permission ceiling', async () => {
  const yaml = await workflow();
  assert.match(yaml, /actions: read/);
  assert.match(yaml, /contents: read/);
  assert.match(yaml, /pull-requests: write/);
  assert.match(yaml, /issues: write/);
  assert.match(yaml, /review_api_key: \$\{\{ secrets\.REVIEW_CLAUDE_API_KEY \}\}/);
  assert.doesNotMatch(yaml, /secrets:\s*inherit/);
  assert.doesNotMatch(yaml, /REVIEW_CODEX_API_KEY|PI_CLIPROXY_KEY|HLOOL_API_KEY|OPENAI_API_KEY/);
  assert.equal((yaml.match(/uses: Kizunad\/review\//g) ?? []).length, 1);
});

test('provider canary remains dispatch-only, minimally permissioned, and secret-isolated', async () => {
  const yaml = await canaryWorkflow();
  assert.match(yaml, /^  workflow_dispatch:$/m);
  assert.doesNotMatch(yaml, /^  (?:pull_request|pull_request_target|issue_comment|push|schedule):/m);
  assert.match(yaml, /^permissions: \{\}$/m);
  assert.match(yaml, /permissions:\n      actions: read/);
  assert.doesNotMatch(yaml, /contents:|pull-requests:|issues:/);
  assert.match(
    yaml,
    /uses: Kizunad\/review\/\.github\/workflows\/provider-canary\.yml@[0-9a-f]{40}/,
  );
  assert.doesNotMatch(yaml, /provider-canary\.yml@(main|master|v?\d|[0-9a-f]{1,39})\b/);
  assert.match(yaml, /review_base_url: \$\{\{ vars\.REVIEW_CLAUDE_BASE_URL \|\| 'https:\/\/api\.claudeopus\.world' \}\}/);
  assert.match(yaml, /worker_timeout_ms: 60000/);
  assert.match(yaml, /review_api_key: \$\{\{ secrets\.REVIEW_CLAUDE_API_KEY \}\}/);
  assert.doesNotMatch(yaml, /secrets:\s*inherit|CLAUDE_CODE_OAUTH_TOKEN|PI_CLIPROXY_KEY|OPENAI_API_KEY/);
});
test('Bong policy is bounded declarative data with canonical project rules', async () => {
  const value = await policy();
  assert.deepEqual(Object.keys(value).sort(), [
    'minorFindingsRequestChanges', 'project', 'rules', 'version',
  ]);
  assert.equal(value.version, 'project-review-policy.v1');
  assert.equal(value.project, 'Kizunad/Bong');
  assert.equal(value.minorFindingsRequestChanges, true);
  assert.ok(value.rules.length >= 20 && value.rules.length <= 256);

  const ids = new Set();
  for (const rule of value.rules) {
    assert.deepEqual(Object.keys(rule).sort(), ['id', 'severity', 'text']);
    assert.match(rule.id, /^[a-z][a-z0-9-]{0,79}$/);
    assert.ok(!ids.has(rule.id), `duplicate policy rule: ${rule.id}`);
    ids.add(rule.id);
    assert.ok(['blocker', 'major', 'minor'].includes(rule.severity));
    assert.ok(rule.text.length >= 1 && rule.text.length <= 4000);
  }

  for (const required of [
    'plan-intent',
    'production-wiring',
    'three-layer-contract',
    'schema-source-of-truth',
    'qi-conservation',
    'worldview-realms',
    'worldview-economy',
    'layer-registry',
    'skill-full-stack-av',
    'saturated-tests',
  ]) {
    assert.ok(ids.has(required), `missing Bong policy rule: ${required}`);
  }

  const serialized = JSON.stringify(value);
  assert.match(serialized, /醒灵.*引气.*凝脉.*固元.*通灵.*化虚/);
  assert.match(serialized, /骨币/);
  assert.match(serialized, /SPIRIT_QI_TOTAL/);
  assert.match(serialized, /LAYER_REGISTRY/);
  assert.doesNotMatch(serialized, /(?:^|\W)(?:command|shell|runner|action ref|MCP server)(?:\W|$)/i);
});
