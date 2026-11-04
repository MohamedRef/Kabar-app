// Kabar web's service worker (docs/WEB.md), adapted from batna-app main:
// web/sw.js. The app opens from the files kept on the device, and a new deploy arrives in the background for the
// next open. scripts/web/build.sh fills in this build's id and file list, so
// the files in a cache always belong together (kabar.js and its wasm must
// match). Only this origin's GETs are served; Kabar Cloud goes to the network.
const BUILD = "__BUILD__";
const FILES = ["__FILES__"];
const CACHE = "kabar-" + BUILD;

self.addEventListener("install", (event) => {
  event.waitUntil(
    caches.open(CACHE).then((cache) => cache.addAll(FILES)).then(() => self.skipWaiting()),
  );
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) => Promise.all(keys.filter((k) => k.startsWith("kabar-") && k !== CACHE).map((k) => caches.delete(k))))
      .then(() => self.clients.claim()),
  );
});

self.addEventListener("fetch", (event) => {
  const req = event.request;
  if (req.method !== "GET" || new URL(req.url).origin !== self.location.origin) return;
  // Every page of the app is index.html (a sign-in comes back with its
  // answer in the address).
  const key = req.mode === "navigate" ? "./" : req;
  event.respondWith(
    caches
      .open(CACHE)
      .then((cache) => cache.match(key, { ignoreSearch: true }))
      .then((hit) => hit || fetch(req)),
  );
});
