# PROCHAINE SESSION — Carte des régiments

Réécrit en entier le 2026-10-09, fin du premier fil.

## État réel

- Version : **v0.9**, poussée sur `main` du dépôt `Adeh44/carte-regiments` (commit « v0.9 : sources de fabrication et documents de relais »).
- Site en ligne : https://adeh44.github.io/carte-regiments/ (déploiement GitHub Pages réussi à chaque push).
- Artifact Claude à jour : https://claude.ai/artifact/UbpoP14fi3tASNN2s3XXWu (version 10, identique au site).
- Rien sur le disque qui ne soit commité. Le dossier `build/` se refabrique avec `python3 src/build.py`.
- Contenu : 186 unités + 11 camps. Détail dans REFERENCE.md.
- Non vérifié : l'appli iPhone en mode avion après la dernière correction (Adeh n'a pas encore confirmé).

## Programme

1. Questions d'ouverture (quiz) : machine, iPhone à jour ?, mode avion OK ?, toujours d'accord pour que Claude pousse sur `carte-regiments` ?
2. Confirmer les réponses du questionnaire dans METHODE.md (non posé au premier fil).
3. Corriger ce qu'Adeh a constaté sur ses appareils, s'il y a lieu.
4. Choisir le chantier suivant dans IDEES_EN_ATTENTE.md (proposer le quiz en premier).

## Réflexes

- Vérifier que la session est ouverte sur `carte-regiments`, pas sur `ember-guard`.
- Données : sengager.fr d'abord, recherche web avec `allowed_domains` (accès direct bloqué).
- Éditer `src/carte.template.html`, jamais `index.html`.
- Après chaque changement : build, test hors ligne, push, vérification du déploiement, republication de l'artifact.
- Messages courts, quiz, ludique (profil d'Adeh).

## Documents restés périmés

Aucun.

## Prompt de relais (à coller dans le nouveau fil)

> Reprise du projet « Carte des régiments ». La session doit être ouverte sur le dépôt GitHub Adeh44/carte-regiments (pas ember-guard).
> Utilise ma méthode de travail (skill methode-dev).
> Lis dans cet ordre : docs/claude/METHODE.md, docs/claude/REFERENCE.md, docs/claude/IDEES_EN_ATTENTE.md, puis docs/claude/PROCHAINE_SESSION.md en dernier.
> Ensuite pose-moi en quiz les questions d'ouverture du programme (machine, iPhone, mode avion, droits de push), et propose-moi la suite.
