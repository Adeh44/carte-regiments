// Version : 2c3941f2ef
const CACHE = "regiments-2c3941f2ef";
const FILES = ["./", "index.html", "manifest.webmanifest", "icon-192.png", "icon-512.png", "apple-touch-icon.png",
  "t/2_0_0.wasm", "t/3_0_0.wasm", "t/4_3_2.wasm", "t/4_3_3.wasm", "t/4_4_2.wasm", "t/4_4_3.wasm", "t/5_3_2.wasm", "t/5_3_3.wasm", "t/5_4_2.wasm", "t/5_4_3.wasm", "t/6_3_2.wasm", "t/6_3_3.wasm", "t/6_4_2.wasm", "t/6_4_3.wasm"];

self.addEventListener("install", e => {
  e.waitUntil(caches.open(CACHE).then(c => c.addAll(FILES)).then(() => self.skipWaiting()));
});

self.addEventListener("activate", e => {
  e.waitUntil(caches.keys()
    .then(keys => Promise.all(keys.filter(k => k.startsWith("regiments-") && k !== CACHE).map(k => caches.delete(k))))
    .then(() => self.clients.claim()));
});

self.addEventListener("fetch", e => {
  const req = e.request;
  if (req.method !== "GET") return;
  const url = new URL(req.url);
  // La page : copie locale tout de suite (ouverture instantanée même avec une 4G faible) ;
  // les mises à jour arrivent par un nouveau service worker, et la page se recharge alors seule.
  if (req.mode === "navigate") {
    e.respondWith(caches.match("./").then(hit => hit || fetch(req).then(r => {
      const copy = r.clone(); caches.open(CACHE).then(c => c.put("./", copy)); return r;
    }).catch(() => caches.match("index.html"))));
    return;
  }
  // Tuiles, icônes, manifeste et polices Google : copie locale d'abord, réseau sinon (et on garde la copie).
  if (url.origin === location.origin || url.hostname.endsWith("fonts.googleapis.com") || url.hostname.endsWith("fonts.gstatic.com")) {
    // Sans réseau, on n'attend jamais plus de 3 secondes.
    const timeout = new Promise((_, reject) => setTimeout(() => reject(new Error("timeout")), 3000));
    e.respondWith(caches.match(req).then(hit => hit || Promise.race([fetch(req), timeout]).then(r => {
      if (r.ok || r.type === "opaque") { const copy = r.clone(); caches.open(CACHE).then(c => c.put(req, copy)); }
      return r;
    }).catch(() => new Response("", { status: 504 }))));
  }
});
