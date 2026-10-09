# REFERENCE — Carte des régiments

Ce que le projet est. Mis à jour le 2026-10-09 (v0.9).

## Le produit

Carte interactive des implantations de l'Armée de terre : 186 unités (régiments, centres, écoles,
groupements d'instruction, bases du matériel, détachements, musiques, lycées) + 11 grands camps.

| Accès | Adresse | Public |
|---|---|---|
| Site / appli installable | https://adeh44.github.io/carte-regiments/ | public |
| Artifact Claude | https://claude.ai/artifact/UbpoP14fi3tASNN2s3XXWu | privé (compte d'Adeh) |
| Fichier hors ligne (PC, Android) | `build/Carte-des-regiments.html` après fabrication, envoyé à Adeh | — |

Fonctions : filtres par arme (11), spécialité (16 étiquettes) et zone (métropole, outre-mer, étranger),
recherche (ancien sigle compris : « CENZUB » trouve le 94e RI), couche « Camps d'entraînement »,
menu « Aller à » (DOM-TOM, Djibouti, Émirats), zoom jusqu'à la ville, thème clair/sombre,
hors ligne après une première ouverture (service worker).

## Arborescence

```
index.html, sw.js, manifest.webmanifest   <- FABRIQUÉS par src/build.py (ne pas éditer)
icon-192.png, icon-512.png, apple-touch-icon.png
src/carte.template.html   <- LA source : CSS, HTML, JS et le tableau U des unités
src/build.py              <- fabrique tout (voir plus bas)
src/geo/*.min.json        <- fonds de carte prêts à l'emploi
src/vendor/leaflet.*      <- Leaflet 1.9.4 (BSD-2)
src/outils/               <- scripts de régénération des données + test hors ligne
docs/claude/              <- documents de suivi (ce dossier)
build/                    <- sorties non versionnées (artifact, fichier hors ligne)
```

## Fabriquer et publier

```
python3 src/build.py                                   # depuis la racine du dépôt
node src/outils/test_hors_ligne.mjs "$PWD" /tmp/x.png  # doit afficher « hors ligne : carte affichée »
git add -A && git commit -m "vX.YY : ..." && git push origin main
```
Puis republier l'artifact : outil Artifact, `action: read` sur l'URL ci-dessus, puis `publish` de
`build/carte-regiments.html` avec `url` = l'URL de l'artifact.

Le test utilise Playwright (Chromium) installé dans le conteneur : `/opt/node22/lib/node_modules/playwright`.
Changer le port dans le script s'il est déjà pris.

## Format d'une unité (tableau `U` dans le gabarit)

```
["sigle","nom complet","arme",["étiquettes"],"ville","département",lat,lon,"rattachement","mission","zone?"]
```
- arme : inf, cav, art, gen, trs, trn, mat, alat, sou, sma, ia
- étiquettes : montagne, para, legion, marine, fs, centre, ecole, cfim, rens, cyber, nrbc, secours, reserve, site, musique, lycee
- zone : absente = métropole, "om" = outre-mer, "etr" = étranger
- rattachement vide "" = ligne masquée dans la fiche
- **Attention à la virgule** à la fin de chaque ligne sauf la dernière (piège déjà payé).
- Les unités à moins de 10 km sont regroupées automatiquement en petite grille.

Les camps sont dans le tableau `CAMPS` : [nom, lat, lon, surface en hectares].

## Données et sources

- Unités : sengager.fr d'abord, puis defense.gouv.fr, Bulletin officiel des armées (BOA), presse spécialisée.
- Noms 2025 : depuis le 1er juillet 2025, les centres ont perdu leur double appellation
  (CENZUB -> 94e RI, CAPCIA -> 51e RI, CENTAC -> 1er BCP, CNEC -> 1er CHOC, CEITO -> 122e RI,
  CEPC -> 3e RA) et les CFIM sont devenus des groupements d'instruction.
- Fonds : Natural Earth (pays 1:50m, détail 1:10m dans 8 zones d'outre-mer et étranger),
  départements et régions IGN via france-geojson, villes GeoNames (paquet npm all-the-cities, >= 1 500 hab.,
  >= 1 000 hab. outre-mer).

## Décisions

- Fonds vectoriels embarqués plutôt que tuiles : l'artifact bloque les images externes, et le hors ligne l'exige.
- Site GitHub Pages + appli installable (PWA) pour l'iPhone : l'iPhone ouvre mal un fichier HTML local.
- Détails (routes, côtes 1:10m) hors métropole seulement là où l'armée est présente, pour limiter la taille (~3 Mo).
- Camps dessinés en cercles de même surface : pas de contours officiels disponibles.
- Villes en gris, sigles d'unités en pastilles foncées : demande d'Adeh (les villes se confondaient avec l'armée).

## Décisions rejetées

- Tuiles OpenStreetMap : bloquées dans l'artifact, et ne marchent pas hors ligne.
- Création du dépôt par Claude : refusée par GitHub (droits de l'app). Adeh l'a créé lui-même.
- Netlify Drop : demande un compte en plus, GitHub déjà disponible.
- Ranger la carte dans `ember-guard` : erreur du premier fil, annulée.

## Dettes ouvertes (infos à confirmer)

- 1er GI BMAINT (camp des Garrigues) : nom tiré de Wikipédia et d'un forum.
- 6e GIM (Gap) et 18e RIT (Dieuze) : confirmés seulement par Wikipédia.
- 12e BSMAT, détachement du Mans : cité par la seule fiche officielle récente.
- 8e RT : rattachement au Commissariat au numérique de défense, une seule source.
- 17e GA : sengager.fr dit « groupe », d'autres sources « 17e RA » depuis 2025.
- Unités de la Légion, 1er CHOC, 122e RI, 3e RA, 24e RI, 5e RE : absents de sengager.fr, sources defense.gouv.fr.
- Rattachements de brigade : plusieurs sources divergent (2e RD, 28e RAG, 31e RG…).
- Petites îles (Réunion, Mayotte, Tahiti) : pas de routes dans Natural Earth. Rivière Isère absente.
- Le menu « Aller à » ne se remet pas à jour quand on déplace la carte à la main.
