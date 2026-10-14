# PROCHAINE SESSION — Carte des régiments

Réécrit en entier le 2026-10-14.

## État réel

- Version : **v0.14** : fond topographique OSM + relief (France, outre-mer, Djibouti, Émirats), zoom max 16, unités regroupées.
- L'artifact Claude est abandonné (trop lourd) : seul le site compte.
- Publication par GitHub Actions (réglage Pages > Source = GitHub Actions, fait par Adeh). (fond Rust en tuiles, optimisation mobile, correctif zooms rapides et mises à jour), poussée sur `main` de `Adeh44/carte-regiments`.
- Site public : https://adeh44.github.io/carte-regiments/ — artifact privé : https://claude.ai/artifact/UbpoP14fi3tASNN2s3XXWu
- Contenu inchangé : 186 unités + 11 camps.
- Non vérifié : fluidité du fond topo sur le Xiaomi Note 14 d'Adeh (simulé : pire blocage ~0,3-0,6 s à Paris z14).
- Pistes si trop lourd : chemins (z14) et ruisseaux (z13) sont les plus gros postes ; voir REFERENCE « Fond topographique ».

## Programme

1. Quiz d'ouverture : la carte est-elle fluide sur le Xiaomi ? Le mode avion marche-t-il ? Toujours d'accord pour le push ?
2. Corriger ce qu'Adeh a constaté.
3. Choisir le chantier suivant dans IDEES_EN_ATTENTE.md (proposer le quiz en premier).

## Réflexes

- Éditer `src/carte.template.html` ou `src/carto/src/main.rs`, jamais `index.html` ni `t/`.
- Après chaque changement : `python3 src/build.py`, test hors ligne, push, vérification du déploiement, republication de l'artifact.
- Messages courts, quiz, ludique (profil d'Adeh).

## Prompt de relais (à coller dans le nouveau fil)

> Reprise du projet « Carte des régiments ». La session doit être ouverte sur le dépôt GitHub Adeh44/carte-regiments (pas ember-guard).
> Utilise ma méthode de travail (skill methode-dev).
> Lis dans cet ordre : docs/claude/METHODE.md, docs/claude/REFERENCE.md, docs/claude/IDEES_EN_ATTENTE.md, puis docs/claude/PROCHAINE_SESSION.md en dernier.
> Ensuite pose-moi en quiz les questions d'ouverture du programme, et propose-moi la suite.
