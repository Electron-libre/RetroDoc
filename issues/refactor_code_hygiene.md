# Refactorisations d'hygiène, de maintenabilité et de lisibilité

# Objectif

Réduire la duplication et les fichiers trop gros relevés lors d'une passe de revue sur tout le dépôt, sans
changer aucun comportement : les tests existants, `cargo clippy` (pedantic) et un second `generate` à 0 fichier
écrit doivent rester verts après chaque livrable.

# Constat

État de santé : `cargo clippy --workspace --all-targets` est sans warning, les `#[allow]` sont rares et
justifiés (7, tous `cast_precision_loss`/`float_cmp` commentés ou locaux), pas de `todo!`/`dbg!`, pas de
`println!` hors du CLI, `unwrap`/`expect` cantonnés aux modules de tests. Le socle est sain ; les points
ci-dessous sont de la dette de structure, par ordre décroissant d'intérêt.

1. **Boucle de lecture par lots dupliquée** (`glossary.rs` ~l.296-345, `entry_points.rs` ~l.225-285). Même
   algorithme copié : lots de fichiers → prompt `--- path ---` → `complete_json` → repli fichier par fichier
   si le lot est inutilisable → attribution par fichier → comptage des chunks restants → fusion → sauvegarde
   après chaque lot. Seuls changent le type de réponse, le prompt système et la fusion. `entry_points.rs`
   importe en plus `batches` depuis `glossary.rs` (couplage entre deux passes sœurs). Toute correction (ex. le
   repli, la sauvegarde partielle) doit aujourd'hui être faite deux fois. Les tests sont dupliqués aussi
   (`OnlyOneFileAtATime`, `ScriptedProvider` dans les deux).
2. **Préambule des commandes CLI copié 7 fois** (`glossary`, `actors`, `entry_points`, `roles`, `surface`,
   `scan`, `generate`) : `canonicalize` + `Config::load` + `retrodoc_ingest::run` avec les mêmes messages
   `context(...)`, et la collecte des `source_files` (`FileKind::Source`) qui existe en deux versions
   (`generate::source_paths` et le bloc inline de `actors.rs`). Les messages d'erreur sont identiques à
   l'octet près, donc faciles à diverger.
3. **Chemins `.retrodoc/cache/*.{yaml,json}` éparpillés** : une constante `*_RELATIVE_PATH` par module (13),
   chacun avec son couple `load`/`save` de même forme (`artifact::load_yaml` + `join`), plus des littéraux
   dans les messages du CLI (`generate.rs`, `glossary.rs`, `actors.rs`, `entry_points.rs`, `roles.rs`) et la
   liste de fichiers de `clear_caches`. Renommer un artefact oblige à toucher tous ces endroits, et
   `clear_caches` peut oublier un artefact sans que rien ne le signale (déjà vrai pour `scope.yaml`, à
   vérifier voulu).
4. **`OpenRouterProvider::complete` fait ~112 lignes avec 5 niveaux d'imbrication** (`retrodoc-llm/src/lib.rs`
   l.269-380) : construction de la requête, boucle de retry, décodage de la réponse, avertissement de
   troncature, calcul du délai y sont mêlés. `lib.rs` (793 lignes) regroupe aussi types publics, décorateur
   `HeartbeatProvider`, client HTTP et tests.
5. **`generate::run` orchestre 11 passes à la suite avec des `println!` et des `tracker.set_pass` entrelacés**
   (`generate.rs`, 441 lignes dont ~140 d'affichage). Les `set_pass`/`end_pass` manuels sont faciles à oublier
   (un oubli attribue les tokens à la mauvaise passe) ; l'affichage (`print_*`) est mélangé à l'orchestration.
6. **Fakes de `LlmProvider` recopiés dans les tests** : 28 implémentations (`CannedProvider` ×4,
   `CountingProvider` ×4, `ScriptedProvider` ×3, `FlakyProvider` ×2, `RecordingProvider` ×2…), la plupart de
   10 à 30 lignes qui ne diffèrent que par la réponse renvoyée.
7. **Gros fichiers mêlant types, logique et tests** : `glossary.rs` (759), `confidence.rs` (663),
   `features.rs` (646), `entry_points.rs` (562), `roles.rs` (552), `markdown.rs` (535). Les modules
   `repo_map/`, `domains/` et `use_cases/` ont déjà le bon découpage (`mod.rs` + `tests.rs`) ; il n'est pas
   appliqué ailleurs, les tests représentant 40 à 60 % de ces fichiers.
8. **Constantes de taille sans lien** : une trentaine de `MAX_*_CHARS` / `MAX_*` locaux aux passes
   (`MAX_FILE_CHARS` 6000, `MAX_MODEL_FILE_CHARS` 4000, `MAX_ENTRY_FILE_CHARS` 5000, `BATCH_CHARS` 12000…),
   les unes avec `_`, les autres sans (`6000`, `4_000`). Pas un défaut en soi, mais le style de littéraux est
   incohérent et la raison des valeurs n'est pas toujours documentée.

# Moyen

Un livrable par point, dans cet ordre, chacun avec son commit `refactor(...)` et sans changement de
comportement :

1. **Lecture par lots commune** : extraire dans le pipeline (module `batched_read.rs`, par exemple) un pilote
   générique paramétré par le prompt système, la fonction d'attribution et la fusion ; y déplacer `batches`.
   `glossary` et `entry_points` ne gardent que leur schéma et leur fusion. Mutualiser les fakes de test
   correspondants. Vérifier que `a_batch_that_cannot_be_answered_is_retried_file_by_file` et la sauvegarde partielle
   passent inchangés.
2. **Contexte de commande CLI** : une structure `Workspace { repo_root, config }` (`commands/context.rs`) avec
   `open(path)` (canonicalize + config, mêmes messages) et des méthodes `ingest()` / `source_files()`. Les 7
   commandes l'utilisent ; supprimer `source_paths` et le bloc inline.
3. **Artefacts** : un module `artifacts` exposant les noms de fichiers en un seul endroit (enum ou constantes),
   utilisé par les `load`/`save`, par `clear_caches` (liste dérivée plutôt que recopiée) et par les messages
   du CLI. Décider explicitement si `scope.yaml`/`roles.yaml`/`usage.json` sont conservés par `--force` et le
   consigner dans le doc de la fonction.
4. **`OpenRouterProvider::complete`** : extraire `build_request`, `decode_success` (choix, troncature, usage)
   et `retry_delay(attempt, server_delay)` ; l'erreur réseau et le statut retryable partagent le calcul du
   backoff. Éclater `lib.rs` en `types.rs`, `heartbeat.rs`, `openrouter.rs` en gardant les ré-exports (l'API
   publique ne change pas).
5. **`generate::run`** : extraire l'affichage dans `commands/generate/report.rs` (ou `print.rs`) et
   introduire un petit garde `tracker.pass("roles")` qui appelle `end_pass` au drop, pour que les passes
   non-LLM ne volent plus de tokens par oubli. Au passage, `run` prend 6 paramètres : regrouper
   `dry_run/force/confidence/max_files` dans une `GenerateOptions`.
6. **Fakes de test** : un module `testing` (`#[cfg(test)]`, ou une feature `test-support` de `retrodoc-llm` si
   les crates en ont besoin) avec `Scripted` (liste de réponses), `Recording` (prompts reçus) et `Counting`
   (compteur d'appels) ; migrer les fakes triviaux, garder les fakes spécifiques (`PeakProvider`,
   `SlowProvider`) en place.
7. **Fichiers volumineux** : passer `glossary`, `confidence`, `features`, `entry_points`, `roles` en
   `dir/mod.rs + tests.rs` comme `repo_map/`. Pur déplacement, un commit par fichier, `git mv` pour garder
   l'historique lisible. Ne pas toucher `markdown.rs` avant d'avoir décidé si les tests restent à côté.
8. **Constantes** : uniformiser les littéraux (`6_000`, tout avec séparateur) et ajouter un commentaire
   « pourquoi cette valeur » aux constantes qui n'en ont pas. Pas de centralisation dans un seul fichier
   (chaque borne appartient à sa passe).

Hors périmètre : toute évolution fonctionnelle, tout changement de format des artefacts ou du schéma des
prompts (les fingerprints invalideraient les caches), et l'ADR 0006 qui reste valable. Si le livrable 3 ou 4
change une décision d'architecture, l'ajouter dans `docs/adr/`.

# Resources

* `crates/retrodoc-pipeline/src/glossary.rs`, `crates/retrodoc-pipeline/src/entry_points.rs` (point 1)
* `crates/retrodoc-cli/src/commands/` (points 2 et 5), `generate.rs` en particulier
* `crates/retrodoc-pipeline/src/artifact.rs` et les constantes `*_RELATIVE_PATH` (point 3)
* `crates/retrodoc-llm/src/lib.rs` (point 4)
* `crates/retrodoc-pipeline/src/repo_map/` : le modèle de découpage à reproduire (point 7)
* ADR `0001` (crates à sens unique), `0005` (fingerprints, à ne pas invalider), `0012` (résilience des requêtes)
* `just check` pour la validation de chaque livrable

# Suivi

1. [x] Lecture par lots commune (`batched_read.rs`, `glossary` + `entry_points`)
2. [x] Contexte de commande CLI (`Workspace`)
3. [x] Module d'artefacts (noms centralisés, `clear_caches` dérivé)
4. [x] `OpenRouterProvider::complete` découpé, `lib.rs` éclaté
5. [x] `generate::run` : affichage extrait, garde de passe, `GenerateOptions`
6. [x] Fakes de test mutualisés
7. [x] Gros fichiers en `dir/mod.rs` + `tests.rs`
8. [ ] Constantes : littéraux uniformisés, raisons documentées

## Décisions

* Le pilote du livrable 1 est `pub(crate)` dans `crates/retrodoc-pipeline/src/batched_read.rs`.
* Fakes de test (livrable 6) : module `#[cfg(test)]` interne au crate pipeline, pas de feature `test-support`.
* `--force` garde son comportement actuel (`roles.yaml`, `usage.json`, `scope.yaml` conservés) ; on le documente sans le changer.
