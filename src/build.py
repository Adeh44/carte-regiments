# Fabrique la carte à partir de src/carte.template.html.
# Usage (depuis la racine du dépôt) : python3 src/build.py
# Produit :
#   build/carte-regiments.html  -> version pour l'artifact Claude (Leaflet chargé depuis cdnjs)
#   build/Carte-des-regiments.html -> version hors ligne en un seul fichier (PC, Android)
#   index.html, sw.js, manifest.webmanifest à la racine -> le site GitHub Pages (appli installable)
import hashlib, json, os
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
def lire(*p): return open(os.path.join(HERE, *p), encoding='utf-8').read()
os.makedirs(os.path.join(ROOT, 'build'), exist_ok=True)

# 1. Version artifact : données et CSS injectées dans le gabarit
s = lire('carte.template.html')
remplacements = {
    '/*__LEAFLET_CSS__*/': lire('vendor', 'leaflet.css'),
    '/*__WORLD__*/null': lire('geo', 'world.min.json'),
    '/*__DEPS__*/null': lire('geo', 'dep.min.json'),
    '/*__REGS__*/null': lire('geo', 'reg.min.json'),
    '/*__RIVERS__*/null': lire('geo', 'rivers.min.json'),
    '/*__ROADS__*/null': lire('geo', 'roads.min.json'),
    '/*__LAKES__*/null': lire('geo', 'lakes.min.json'),
    '/*__CITIES__*/null': lire('geo', 'cities.min.json'),
    '/*__ZONES__*/null': lire('geo', 'zones.min.json'),
}
for k, v in remplacements.items():
    assert s.count(k) == 1, k
    s = s.replace(k, v)
open(os.path.join(ROOT, 'build', 'carte-regiments.html'), 'w', encoding='utf-8').write(s)

# 2. Version hors ligne : Leaflet intégré, page HTML complète
tag = '<script src="https://cdnjs.cloudflare.com/ajax/libs/leaflet/1.9.4/leaflet.js"></script>'
assert s.count(tag) == 1
js = lire('vendor', 'leaflet.js'); assert '</script' not in js
s = s.replace(tag, '<script>/* Leaflet 1.9.4, BSD-2-Clause, (c) Volodymyr Agafonkin */\n' + js + '\n</script>')
i = s.index('<div class="app">')
s = ('<!doctype html>\n<html lang="fr">\n<head>\n<meta charset="utf-8">\n'
     '<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">\n'
     + s[:i] + '<style>html,body{margin:0}</style>\n</head>\n<body>\n' + s[i:] + '\n</body>\n</html>\n')
open(os.path.join(ROOT, 'build', 'Carte-des-regiments.html'), 'w', encoding='utf-8').write(s)

# 3. Site / appli (PWA) à la racine du dépôt
out = ROOT
head_add = '''<meta name="theme-color" content="#3d4c2b">
<meta name="apple-mobile-web-app-capable" content="yes">
<meta name="mobile-web-app-capable" content="yes">
<meta name="apple-mobile-web-app-title" content="Régiments">
<meta name="apple-mobile-web-app-status-bar-style" content="default">
<link rel="manifest" href="manifest.webmanifest">
<link rel="icon" type="image/png" sizes="192x192" href="icon-192.png">
<link rel="apple-touch-icon" href="apple-touch-icon.png">
<style>body{box-sizing:border-box;padding-top:env(safe-area-inset-top,0px)}</style>
'''
a = '<title>Carte des régiments</title>'
assert s.count(a) == 1
s = s.replace(a, a + '\n' + head_add)
sw = '''<script>
// Mode hors ligne : le service worker garde une copie de la carte sur l'appareil.
if ("serviceWorker" in navigator) {
  navigator.serviceWorker.register("sw.js").catch(() => {});
}
</script>
</body>'''
assert s.count('</body>') == 1
s = s.replace('</body>', sw)
version = hashlib.sha1((s + open(__file__, encoding='utf-8').read()).encode()).hexdigest()[:10]
open(out + '/index.html', 'w').write(s)
json.dump({
  "name": "Carte des régiments de l'Armée de terre",
  "short_name": "Régiments",
  "start_url": "./",
  "scope": "./",
  "display": "standalone",
  "background_color": "#eceee4",
  "theme_color": "#3d4c2b",
  "lang": "fr",
  "icons": [
    {"src": "icon-192.png", "sizes": "192x192", "type": "image/png", "purpose": "any"},
    {"src": "icon-512.png", "sizes": "512x512", "type": "image/png", "purpose": "any maskable"}
  ]
}, open(out + '/manifest.webmanifest', 'w'), ensure_ascii=False, indent=2)
open(out + '/sw.js', 'w').write('''// Version : ''' + version + '''
const CACHE = "regiments-''' + version + '''";
const FILES = ["./", "index.html", "manifest.webmanifest", "icon-192.png", "icon-512.png", "apple-touch-icon.png"];

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
  // La page : réseau d'abord (pour recevoir les mises à jour), copie locale si pas de réseau.
  if (req.mode === "navigate") {
    const timeout = new Promise((_, reject) => setTimeout(() => reject(new Error("timeout")), 4000));
    e.respondWith(Promise.race([fetch(req), timeout]).then(r => {
      const copy = r.clone(); caches.open(CACHE).then(c => c.put("./", copy)); return r;
    }).catch(() => caches.match("./").then(r => r || caches.match("index.html"))));
    return;
  }
  // Icônes, manifeste et polices Google : copie locale d'abord, réseau sinon.
  if (url.origin === location.origin || url.hostname.endsWith("fonts.googleapis.com") || url.hostname.endsWith("fonts.gstatic.com")) {
    // Sans réseau, on n'attend jamais plus de 3 secondes.
    const timeout = new Promise((_, reject) => setTimeout(() => reject(new Error("timeout")), 3000));
    e.respondWith(caches.match(req).then(hit => hit || Promise.race([fetch(req), timeout]).then(r => {
      if (r.ok || r.type === "opaque") { const copy = r.clone(); caches.open(CACHE).then(c => c.put(req, copy)); }
      return r;
    }).catch(() => new Response("", { status: 504 }))));
  }
});
''')
print('version', version)
