# METHODE — Carte des régiments

Réponses au questionnaire, règles propres au projet, routine git, pièges déjà payés.
Créé le 2026-10-09 (fin du premier fil).

## Questionnaire de départ

Le questionnaire n'a pas été posé au premier fil (projet lancé dans l'urgence, dans le mauvais dépôt).
Réponses déduites du fil, à confirmer en quiz au prochain fil :

| Question | Réponse constatée | À confirmer |
|---|---|---|
| Machines | PC (Windows, Chrome) + iPhone (appli installée depuis Safari) | oui |
| Versionnement | Git, dépôt GitHub `Adeh44/carte-regiments` (public) | non |
| Mode de travail | Vitesse : Claude écrit, publie, Adeh teste sur ses appareils | oui |
| Outils | HTML/CSS/JS + Leaflet 1.9.4, scripts Python de fabrication, aucun éditeur graphique | non |
| Profil | Profil habituel d'Adeh (quiz, messages courts, ludique) | oui |

## Règles propres au projet

- **Un seul dépôt : `Adeh44/carte-regiments`.** Ne jamais toucher à `ember-guard` (le jeu), même si la session s'y ouvre par erreur.
- **Source sûre = sengager.fr.** Puis defense.gouv.fr et le Bulletin officiel des armées. Wikipédia en dernier recours, et toute info qui n'en vient que de lui est signalée comme « à confirmer ».
- Depuis le conteneur cloud, sengager.fr et wikipedia.org sont bloqués en accès direct : passer par la recherche web avec `allowed_domains: ["sengager.fr"]`.
- **Toute modification de données se fait dans `src/carte.template.html`**, jamais dans `index.html` (fichier fabriqué).
- Après chaque changement : `python3 src/build.py`, test hors ligne, puis publication (site + artifact). Voir REFERENCE.md.
- Adeh veut rester sur git ET garder un lien public partageable (le site GitHub Pages) ; il ne veut pas consommer de crédits cloud inutilement.
- Adeh a autorisé Claude à pousser sur `carte-regiments` pendant le premier fil. Au début d'un fil, redemander si c'est toujours d'accord (règle § 4 de la méthode).

## Routine git

1. `git status` et `git diff --stat`, lus.
2. Commit `vX.YY : description`, puis `git push origin main`. Lire la ligne `main -> main`.
3. Vérifier le déploiement : `gh run list -R Adeh44/carte-regiments -L 3` (workflow « Site »). Le site `adeh44.github.io` n'est pas joignable depuis le conteneur.
4. Le fond topo se refait avec le workflow « Fond topographique » (~45 min, gratuit). Ne pas le relancer pour un simple changement de la page.

Numérotation : v0.9 = fin du premier fil. Augmenter la constante `VERSION` du gabarit à chaque publication (affichée en bas de la carte). C'est Adeh qui déclare la v1.0.

## Pièges déjà payés

| Symptôme | Cause | Réflexe |
|---|---|---|
| 8 commits « en attente » impossibles à pousser, Adeh mécontent | Session ouverte dans `ember-guard`, carte rangée dans le dépôt du jeu | Vérifier le dépôt au premier message. Carte = `carte-regiments` uniquement. |
| iPhone en mode avion : filtres visibles, pas de carte | Lien Google Fonts bloquant : le script attendait la police | Polices chargées par script (déjà fait). Ne jamais remettre de `<link rel=stylesheet>` externe en tête. |
| Le mode hors ligne ne s'activait pas | Service worker enregistré sur l'événement `load`, qui n'arrive pas si une ressource pend | Enregistrer le service worker immédiatement. |
| Page blanche après ajout de données | Constante `ZONES` déclarée deux fois | `node --check` sur le script extrait avant de publier. |
| Page blanche malgré `node --check` OK | Virgule manquante entre deux lignes du tableau `U` : `[...] [...]` devient une indexation | Toujours charger la page dans le test (src/outils/test_hors_ligne.mjs). |
| Étiquettes d'unités décalées sous les symboles | Leaflet mesure les étiquettes quand elles sont masquées (`display:none`) | `refreshLabels()` après chaque changement de zoom ou de filtre (déjà fait). |
| Traits droits visibles en outre-mer | Contour des polygones découpés tracé sur les bords de découpe | Remplissage sans contour + côtes en lignes séparées (déjà fait). |
| Fonds de carte vides dans l'artifact | La CSP de l'artifact bloque les images externes (tuiles OSM) | Fonds vectoriels embarqués, pas de tuiles. |
| Le shell se tue tout seul | `pkill -f motif` qui correspond à sa propre commande | Tuer par numéro de processus. |
| Marqueurs collés en pile hors de leur place | `position: relative` ajouté sur `.unit`, qui écrase le `position: absolute` de Leaflet | Ne jamais repositionner les éléments que Leaflet place. |
| Deux symboles invisibles (ALAT, santé) | SVG en image data: avec un attribut en double : invalide en XML, toléré en HTML | Les symboles passent par `cleanGlyph()`. |
| Symboles absents malgré la règle CSS | Le `<style>` de la page vient après celui injecté : `.sym` écrasait `.s-inf` | Sélecteur `.sym.s-xxx`. |
| Morceaux de carte mélangés après des zooms rapides | Canvas recyclé alors qu'un dessin en attente le visait encore | Jeton `_key` par tuile, vérifié avant de dessiner. |
| Ancienne version affichée en 4G faible | Page « réseau d'abord » avec repli sur la copie après 4 s | Copie locale d'abord + rechargement auto quand le nouveau service worker prend la main. |
| Planetiler refuse de démarrer (93 Go demandés) | `--nodemap-type=array` réserve la place de tous les points d'OSM | `sparsearray`. |
| Rendu topo lent (traits) | Angles arrondis sur des milliers de petits traits | Arrondis seulement si épaisseur >= 3,5 px (`strokeLayer`). |
| Publication refusée sur l'artifact | Type `.bin` non servi | Tuiles en `.wasm`. |
| Données fausses publiées (2e RG dissous en 2010, DLEM devenu 5e RE…) | Données tirées de mémoire ou de Wikipédia | Vérifier chaque unité sur sengager.fr avant ajout. |
