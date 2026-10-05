# Improve Harness

# Objectif 

Fiabiliser l'autonomie de l'agent de code, limiter la répétition d'instructions dans les prompts

# Moyen

Complèter le harness du dépot avec des skills et hooks.

# Resources

Historique des conversations avec l'agent de code et commits.
Fichiers de documentation du dépot

# Hints

Je vais donner les instructions a l'agent avec des fichiers comme celui-ci (document courrant).

Pour chaque issue voici ce qui doit être fait:

* Reformuler le besoin et valider la compréhension avec l'utilisateur
* Une fois la compréhension validée préparer un plan d'action en plusieurs livrables.
* Pour chaque livrable:
* * Rassembler les informations nécessaires, demander les informations manquantes
* * Ecrire un test de comportement ou end-to-end pour valider le livrable.
* * Ecrire le code pour implémenter le livrable
* * Mettre a jour la documentation du dépot, les décisions d'architecture, diagrammes, etc.
* * Faire une review du code et de la documentation.
* * Soumettre a l'utilisateur pour une review humaine.
* * Mettre a jour l'issue pour suivre le progrès.
* * Préparer le message de commit.

# Suivi

## Plan (validé)

1. [x] Hooks : `cargo fmt` après édition, clippy bloquant à l'arrêt (`.claude/hooks/`, `.claude/settings.json`) — commit 64910f1
2. [x] Skill `issue-workflow` (boucle de l'issue, arrêts de validation, suivi dans l'issue) + `docs/adr/` — en attente de review humaine
2b. [x] Scripts en `rust-script` (hooks + tests) et `justfile` (`just check`, `just test-harness`) — ajouté à la demande de l'utilisateur, en attente de review humaine
3. [ ] Skill `commit-message` (règles `AGENTS.md`, sans attribution)
4. [ ] Skill `smoke-test` (the Rails test repo, Ollama local, Gemini)
5. [ ] Nettoyage des permissions (`settings.json` partagé vs `settings.local.json`)

## Décisions

* Skills en anglais ; l'agent parle français avec l'utilisateur.
* L'agent peut committer, mais seulement après validation de la review humaine d'un livrable.
* Scripts du harness en Rust (`rust-script`), commandes de dev dans un `justfile` (pas de bash).
* ADR dans `docs/adr/`.
* Clippy en échec : l'agent corrige (hook Stop, une seule relance pour éviter la boucle).
* `.claude/` (hors `settings.local.json`) et `issues/` sont versionnés dans le dépôt.
