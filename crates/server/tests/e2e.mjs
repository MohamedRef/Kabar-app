// End-to-end test of Kabar Cloud on a local workerd (`wrangler dev`), with
// a fake Google: this script signs its own ID tokens and serves their JWKS.
// Nothing leaves the machine. Run with `just cloud-e2e` (needs Node 22+).
//
// It checks M2's acceptance criterion — a word saved on the phone appears on
// the tablet within seconds — plus sign-in, the allowlist, the Devices
// screen, remote sign-out, refresh rotation and the proxy's privacy rule.
// Then it runs the app's own sync engine (Rust, two devices) against the
// same server. KABAR_E2E_SKIP_APP=1 skips that part.

import { spawn, execFileSync } from "node:child_process";
import { generateKeyPairSync, createSign, createHash, randomBytes } from "node:crypto";
import { createServer } from "node:http";
import { writeFileSync, rmSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";

const here = dirname(fileURLToPath(import.meta.url));
const serverDir = join(here, "..");
const PORT = 8787;
const JWKS_PORT = 8790;
const BASE = `http://127.0.0.1:${PORT}`;
const CLIENT_ID = "kabar-e2e.apps.googleusercontent.com";
const EMAIL = "learner@example.com";
const wrangler = process.env.WRANGLER ?? "wrangler";

// --- fake Google ---------------------------------------------------------------
const { privateKey, publicKey } = generateKeyPairSync("rsa", { modulusLength: 2048 });
const jwk = { ...publicKey.export({ format: "jwk" }), kid: "e2e", alg: "RS256", use: "sig" };
const jwks = createServer((req, res) => {
  // /token: an ID token for the app's end-to-end test (stands in for the browser sign-in).
  if (req.url === "/token") {
    res.writeHead(200, { "content-type": "text/plain" });
    return res.end(idToken(EMAIL));
  }
  res.writeHead(200, { "content-type": "application/json", "cache-control": "max-age=60" });
  res.end(JSON.stringify({ keys: [jwk] }));
}).listen(JWKS_PORT, "127.0.0.1");

const b64 = (v) => Buffer.from(typeof v === "string" ? v : JSON.stringify(v)).toString("base64url");
function idToken(email, sub = "110169484474386276334") {
  const now = Math.floor(Date.now() / 1000);
  const input = `${b64({ alg: "RS256", kid: "e2e", typ: "JWT" })}.${b64({
    iss: "https://accounts.google.com", aud: CLIENT_ID, sub, email, email_verified: true, iat: now, exp: now + 3600,
  })}`;
  const sig = createSign("RSA-SHA256").update(input).sign(privateKey).toString("base64url");
  return `${input}.${sig}`;
}

// --- local Worker ------------------------------------------------------------
writeFileSync(join(serverDir, ".dev.vars"), [
  `ALLOWED_EMAILS=${EMAIL}`,
  `TOKEN_SIGNING_KEY=${randomBytes(32).toString("base64")}`,
  `GOOGLE_CLIENT_IDS=${CLIENT_ID}`,
  `INSECURE_DEV_JWKS_URL=http://127.0.0.1:${JWKS_PORT}/certs`,
  "",
].join("\n"));
const persist = join(serverDir, ".wrangler", "e2e-state");
rmSync(persist, { recursive: true, force: true });
execFileSync(wrangler, ["d1", "migrations", "apply", "kabar", "--local", "--env", "e2e", "--persist-to", persist], {
  cwd: serverDir, stdio: ["ignore", "inherit", "inherit"],
});
const dev = spawn(wrangler, ["dev", "--env", "e2e", "--port", String(PORT), "--ip", "127.0.0.1", "--persist-to", persist], {
  cwd: serverDir, stdio: ["ignore", "inherit", "inherit"],
});

function cleanup() {
  dev.kill("SIGTERM");
  jwks.close();
  rmSync(join(serverDir, ".dev.vars"), { force: true });
}
process.on("exit", cleanup);

async function waitForHealth() {
  for (let i = 0; i < 300; i++) {
    try {
      const r = await fetch(`${BASE}/v1/health`);
      if (r.ok) return;
    } catch {}
    await new Promise((r) => setTimeout(r, 1000));
  }
  throw new Error("wrangler dev did not start");
}

// --- client helpers --------------------------------------------------------
async function api(method, path, { token, body } = {}) {
  const res = await fetch(BASE + path, {
    method,
    headers: { "content-type": "application/json", ...(token ? { authorization: `Bearer ${token}` } : {}) },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const text = await res.text();
  return { status: res.status, json: text ? JSON.parse(text) : null };
}

async function signIn(deviceId, name, email = EMAIL) {
  return api("POST", "/v1/auth/google", {
    body: { id_token: idToken(email), device: { device_id: deviceId, name, model: "SM-X", platform: "android" } },
  });
}

class Socket {
  constructor(token) {
    this.inbox = [];
    this.waiters = [];
    // Node's WebSocket (undici) accepts headers through this option.
    this.ws = new WebSocket(`ws://127.0.0.1:${PORT}/v1/sync`, { headers: { authorization: `Bearer ${token}` } });
    this.ws.onmessage = (e) => {
      this.inbox.push(JSON.parse(e.data));
      this.waiters.forEach((w) => w());
    };
    this.closed = new Promise((r) => (this.ws.onclose = (e) => r(e.code)));
    this.open = new Promise((r, j) => {
      this.ws.onopen = r;
      this.ws.onerror = j;
    });
  }
  send(msg) {
    this.ws.send(JSON.stringify(msg));
  }
  async next(type, timeoutMs = 5000) {
    const deadline = Date.now() + timeoutMs;
    for (;;) {
      const i = this.inbox.findIndex((m) => m.type === type);
      if (i >= 0) return this.inbox.splice(i, 1)[0];
      const left = deadline - Date.now();
      if (left <= 0) throw new Error(`no ${type} within ${timeoutMs} ms; got ${JSON.stringify(this.inbox)}`);
      await new Promise((r) => {
        const t = setTimeout(r, left);
        this.waiters.push(() => (clearTimeout(t), r()));
      });
    }
  }
}

// --- the test ------------------------------------------------------------------
const step = (s) => console.log(`\n▶ ${s}`);
await waitForHealth();

step("sign-in: allowlist");
const stranger = await signIn("intruder", "Nope", "someone@example.com");
assert.equal(stranger.status, 403);
assert.equal(stranger.json.code, "not_allowed");

step("sign-in on the phone and the tablet");
const phone = await signIn("s24u", "Galaxy S24 Ultra");
const tablet = await signIn("tab-s11", "Galaxy Tab S11");
assert.equal(phone.status, 200, JSON.stringify(phone.json));
assert.equal(phone.json.account_id, tablet.json.account_id);

step("both connect and catch up");
const phoneWs = new Socket(phone.json.access_token);
const tabletWs = new Socket(tablet.json.access_token);
await Promise.all([phoneWs.open, tabletWs.open]);
phoneWs.send({ type: "hello", protocol: 1, last_seq: 0 });
tabletWs.send({ type: "hello", protocol: 1, last_seq: 0 });
await phoneWs.next("caught_up");
await tabletWs.next("caught_up");

step("a word saved on the phone appears on the tablet");
const op = {
  id: "0193d2a0-0000-7000-8000-000000000001",
  table: "word_status",
  row_id: "lapor",
  kind: "upsert",
  hlc: { wall_ms: Date.now(), counter: 0, device: "s24u" },
  payload: { lexeme: "lapor", status: "learning", lang: "jv" },
};
const t0 = Date.now();
phoneWs.send({ type: "push", batch: 1, ops: [op] });
const ack = await phoneWs.next("ack");
assert.deepEqual(ack.done, [op.id]);
const arrived = await tabletWs.next("ops", 5000);
const latency = Date.now() - t0;
assert.equal(arrived.ops[0].row_id, "lapor");
assert.equal(arrived.ops[0].payload.status, "learning");
console.log(`  arrived on the tablet in ${latency} ms`);

step("a spoofed device id is rejected");
phoneWs.send({ type: "push", batch: 2, ops: [{ ...op, id: "spoof-1", hlc: { ...op.hlc, device: "tab-s11" } }] });
const spoof = await phoneWs.next("ack");
assert.equal(spoof.rejected[0].reason, "wrong_device");

step("Devices screen");
const list = await api("GET", "/v1/devices", { token: tablet.json.access_token });
assert.equal(list.status, 200);
const names = list.json.devices.map((d) => d.name).sort();
assert.deepEqual(names, ["Galaxy S24 Ultra", "Galaxy Tab S11"]);
assert.ok(list.json.devices.find((d) => d.device_id === "tab-s11").current);
assert.ok(list.json.devices.find((d) => d.device_id === "s24u").last_sync_ms);

step("push tokens (M7): stored per device; Firebase isn't set up here");
const push = await api("PUT", "/v1/devices/me/push", { token: tablet.json.access_token, body: { token: "e2e-fcm:APA91b-token" } });
assert.equal(push.status, 200, JSON.stringify(push.json));
assert.deepEqual(push.json, { registered: true, push_configured: false });
const badPush = await api("PUT", "/v1/devices/me/push", { token: tablet.json.access_token, body: { token: "has spaces" } });
assert.equal(badPush.status, 400);
const noPush = await api("PUT", "/v1/devices/me/push", { token: tablet.json.access_token, body: { token: "" } });
assert.deepEqual(noPush.json, { registered: false, push_configured: false });
assert.equal((await signIn("server", "Impostor")).status, 400);

step("refresh rotates the token");
const refreshed = await api("POST", "/v1/auth/refresh", { body: { refresh_token: tablet.json.refresh_token } });
assert.equal(refreshed.status, 200, JSON.stringify(refreshed.json));
assert.notEqual(refreshed.json.refresh_token, tablet.json.refresh_token);

step("AI proxy: personal tasks never reach gemini_free; missing keys say so");
const call = (task, engine) => ({
  cache_key: { task, input_hash: "e2e", engine, prompt_version: 1 },
  system: "", user: "Sek tak lapor pusat", json: true,
});
const blocked = await api("POST", "/v1/ai", { token: tablet.json.access_token, body: call("daily_plan", "gemini_free") });
assert.equal(blocked.status, 403);
assert.equal(blocked.json.kind, "privacy_blocked");
const noKey = await api("POST", "/v1/ai", { token: tablet.json.access_token, body: call("analyze_text", "gemini_free") });
assert.equal(noKey.status, 503);
assert.equal(noKey.json.kind, "unavailable");
const vault = await api("GET", "/v1/vault", { token: tablet.json.access_token });
assert.equal(vault.json.tts_configured, false);
assert.equal(vault.json.push_configured, false);

step("vision: images are uploaded by hash, then named in the AI call");
const png = Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), randomBytes(64)]);
const sha = createHash("sha256").update(png).digest("hex");
const putBlob = (name, bytes) =>
  fetch(`${BASE}/v1/blobs/${name}`, {
    method: "PUT",
    headers: { "content-type": "image/png", authorization: `Bearer ${tablet.json.access_token}` },
    body: bytes,
  });
assert.equal((await putBlob(sha, png)).status, 204);
assert.equal((await putBlob(sha, Buffer.from("<svg/>"))).status, 400, "the body must match its name");
assert.equal((await putBlob("0".repeat(64), png)).status, 400);
const sketch = { ...call("guess_sketch", "workers_ai"), personal: true, user: "What is this drawing?" };
const missing = await api("POST", "/v1/ai", { token: tablet.json.access_token, body: { ...sketch, image_sha256: "f".repeat(64) } });
assert.equal(missing.status, 404);
assert.equal(missing.json.code, "blob_missing");
// The local Worker has no AI binding: the call gets past the blob store to the engine.
const seen = await api("POST", "/v1/ai", { token: tablet.json.access_token, body: { ...sketch, image_sha256: sha } });
assert.equal(seen.status, 503);
assert.equal(seen.json.kind, "unavailable");
const noImage = await api("POST", "/v1/ai", { token: tablet.json.access_token, body: sketch });
assert.equal(noImage.status, 502, "a vision task needs an image");

step("remote sign-out from the tablet closes the phone's socket");
const revoked = await api("DELETE", "/v1/devices/s24u", { token: refreshed.json.access_token });
assert.equal(revoked.status, 200);
await phoneWs.next("revoked");
assert.equal(await phoneWs.closed, 4001);
const again = await api("POST", "/v1/auth/refresh", { body: { refresh_token: phone.json.refresh_token } });
assert.equal(again.status, 401);
assert.equal(again.json.code, "device_revoked");

step("no token, no entry");
assert.equal((await api("GET", "/v1/devices")).status, 401);

tabletWs.ws.close();

if (!process.env.KABAR_E2E_SKIP_APP) {
  step("the app's sync engine: phone → tablet, Devices, remote sign-out");
  // Async, so this process keeps serving the fake Google meanwhile.
  const app = spawn("cargo", [
    "test", "-p", "kabar-app", "--no-default-features", "--",
    "--ignored", "--exact", "cloud::e2e::phone_and_tablet_sync_through_kabar_cloud", "--nocapture",
  ], {
    cwd: join(serverDir, "..", ".."),
    stdio: ["ignore", "inherit", "inherit"],
    env: { ...process.env, KABAR_E2E_URL: BASE, KABAR_E2E_TOKEN_URL: `http://127.0.0.1:${JWKS_PORT}/token` },
  });
  const code = await new Promise((r) => app.on("exit", r));
  assert.equal(code, 0, "the app's end-to-end test failed");
}

console.log(`\n✅ Kabar Cloud end-to-end: all checks passed (phone → tablet in ${latency} ms)`);
process.exit(0);
