# Fabrique la carte à partir de src/carte.template.html.
# Usage (depuis la racine du dépôt) : python3 src/build.py
#
# Étapes :
#   1. Télécharge les fonds IGN et Natural Earth dans src/geo/sources/ s'ils manquent (non versionnés).
#   2. Lance l'outil Rust src/carto : tuiles vectorielles (build/t/*.wasm) + page remplie (build/index.html).
# Produit :
#   build/carte-regiments.html + build/t/  -> version pour l'artifact Claude (Leaflet chargé depuis cdnjs)
#   build/Carte-des-regiments.html          -> version hors ligne en un seul fichier (tuiles embarquées)
#   index.html, sw.js, manifest.webmanifest, t/ à la racine -> le site GitHub Pages (appli installable)
#
# Les tuiles portent l'extension .wasm : c'est le seul type binaire que l'hébergement des artifacts accepte.
# Ce ne sont pas des modules WebAssembly, juste des octets lus par la page.
import base64, hashlib, json, os, shutil, subprocess, urllib.request
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
BUILD = os.path.join(ROOT, 'build')
def lire(*p): return open(os.path.join(HERE, *p), encoding='utf-8').read()
os.makedirs(BUILD, exist_ok=True)

# 1. Fonds sources (environ 33 Mo, téléchargés une fois)
SRC = os.path.join(HERE, 'geo', 'sources')
os.makedirs(SRC, exist_ok=True)
FG = 'https://raw.githubusercontent.com/gregoiredavid/france-geojson/master/'
NE = 'https://raw.githubusercontent.com/nvkelso/natural-earth-vector/master/geojson/'
for url in [FG + 'departements.geojson', FG + 'regions.geojson'] + [NE + f for f in [
        'ne_10m_admin_0_countries.geojson', 'ne_10m_rivers_lake_centerlines.geojson',
        'ne_10m_rivers_europe.geojson', 'ne_10m_lakes.geojson', 'ne_10m_lakes_europe.geojson']]:
    dest = os.path.join(SRC, url.rsplit('/', 1)[1])
    if not os.path.exists(dest):
        print('téléchargement', url)
        urllib.request.urlretrieve(url, dest)

# 2. Outil Rust : tuiles + page
gabarit = lire('carte.template.html')
assert gabarit.count('/*__LEAFLET_CSS__*/') == 1
gabarit = gabarit.replace('/*__LEAFLET_CSS__*/', lire('vendor', 'leaflet.css'))
open(os.path.join(BUILD, 'gabarit.html'), 'w', encoding='utf-8').write(gabarit)
shutil.rmtree(os.path.join(BUILD, 't'), ignore_errors=True)
subprocess.run(['cargo', 'run', '--release', '--quiet', '--manifest-path', os.path.join(HERE, 'carto', 'Cargo.toml'), '--',
                os.path.join(HERE, 'geo'), BUILD, os.path.join(BUILD, 'gabarit.html')], check=True)
s = open(os.path.join(BUILD, 'index.html'), encoding='utf-8').read()
os.replace(os.path.join(BUILD, 'index.html'), os.path.join(BUILD, 'carte-regiments.html'))
tuiles = sorted(os.listdir(os.path.join(BUILD, 't')))

# 3. Version hors ligne : Leaflet et tuiles intégrés, page HTML complète
tag = '<script src="https://cdnjs.cloudflare.com/ajax/libs/leaflet/1.9.4/leaflet.js"></script>'
assert s.count(tag) == 1
js = lire('vendor', 'leaflet.js'); assert '</script' not in js
inline = {f[:-5]: base64.b64encode(open(os.path.join(BUILD, 't', f), 'rb').read()).decode() for f in tuiles}
s_off = s.replace(tag, '<script>/* Leaflet 1.9.4, BSD-2-Clause, (c) Volodymyr Agafonkin */\n' + js + '\n</script>\n'
                  '<script>window.TILES_INLINE = ' + json.dumps(inline, separators=(',', ':')) + ';</script>')
def page_complete(x):
    i = x.index('<div class="app">')
    return ('<!doctype html>\n<html lang="fr">\n<head>\n<meta charset="utf-8">\n'
            '<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">\n'
            + x[:i] + '<style>html,body{margin:0}</style>\n</head>\n<body>\n' + x[i:] + '\n</body>\n</html>\n')
open(os.path.join(BUILD, 'Carte-des-regiments.html'), 'w', encoding='utf-8').write(page_complete(s_off))

# 4. Site / appli (PWA) à la racine du dépôt : Leaflet intégré, tuiles servies à côté
s = page_complete(s.replace(tag, '<script>/* Leaflet 1.9.4, BSD-2-Clause, (c) Volodymyr Agafonkin */\n' + js + '\n</script>'))
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
open(os.path.join(ROOT, 'index.html'), 'w', encoding='utf-8').write(s)
shutil.rmtree(os.path.join(ROOT, 't'), ignore_errors=True)
shutil.copytree(os.path.join(BUILD, 't'), os.path.join(ROOT, 't'))
# La version change dès que la page, une tuile ou ce script change : l'appli se met alors à jour.
h = hashlib.sha1((s + open(__file__, encoding='utf-8').read()).encode())
for f in tuiles: h.update(open(os.path.join(ROOT, 't', f), 'rb').read())
version = h.hexdigest()[:10]
# Préchargés à l'installation : le monde aux zooms 2-3 et la métropole jusqu'au zoom 6
# (paquets des carrés z3 x 3-4, y 2-3). Le reste se garde au fil de la visite.
def metro(f):
    z, gx, gy = map(int, f[:-5].split('_'))
    return z <= 3 or (z <= 6 and gx in (3, 4) and gy in (2, 3))
pre = ['t/' + f for f in tuiles if metro(f)]
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
}, open(os.path.join(ROOT, 'manifest.webmanifest'), 'w'), ensure_ascii=False, indent=2)
open(os.path.join(ROOT, 'sw.js'), 'w').write('''// Version : ''' + version + '''
const CACHE = "regiments-''' + version + '''";
const FILES = ["./", "index.html", "manifest.webmanifest", "icon-192.png", "icon-512.png", "apple-touch-icon.png",
  ''' + ', '.join(json.dumps(p) for p in pre) + '''];

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
''')
print('version', version, '-', len(tuiles), 'paquets de tuiles,', len(pre), 'préchargés')
