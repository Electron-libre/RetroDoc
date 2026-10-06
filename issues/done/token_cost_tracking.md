# Suivi des tokens et des coûts

# Objectif

Savoir ce que coûte un `generate` : nombre d'appels LLM, tokens en entrée et en sortie, durée, par passe et
par modèle, puis un montant optionnel. Aujourd'hui rien n'est compté, et les temps par passe du run du
2026-10-02 sur le dépôt Rails de test ont été perdus avec son log. C'est le point 7 de la phase 8
(`PLAN.md` §7.2), le seul encore ouvert.

# Moyen

Lire le champ `usage` (`prompt_tokens`, `completion_tokens`) que OpenRouter, Gemini et Ollama renvoient et que
le client ignore. Agréger dans un provider-wrapper, comme `HeartbeatProvider`, puis afficher un récapitulatif à
la fin de `generate`.

# Resources

* `PLAN.md` §7.2, item 7 (description de départ et mesures existantes)
* `crates/retrodoc-llm` : `CompletionResponse` (`content`, `model`), `OpenRouterProvider`, `HeartbeatProvider`
* `crates/retrodoc-pipeline/src/progress.rs` (une ligne de log par unité) et `repo_map` (`estimate_repo_map`)
* `crates/retrodoc-core/src/config.rs` (section `[llm]`)
* ADR `0011` (cost control) et `0012` (LLM request resilience)

# Hints

* Pas de prix en dur dans le code : `llm.price_per_mtok_in` / `llm.price_per_mtok_out` optionnels dans
  `retrodoc.toml`. Sans eux, pas de montant, seulement des tokens.
* Un serveur peut ne pas renvoyer `usage` : le champ est optionnel, on compte alors les appels et on le dit
  (pas de tokens inventés).
* L'agrégation doit tenir avec `llm.concurrency` > 1 (compteurs atomiques ou verrou, pas d'ordre supposé).
* Les retries comptent : un appel retenté consomme des tokens deux fois.
* Les tests utilisent un faux `LlmProvider` (voir `CountingProvider` dans `repo_map/tests.rs`).
* Ne jamais nommer les dépôts de test confidentiels dans les fichiers commités (chiffres seulement).

# Suivi

## Plan (validé)

1. [x] **Capture** : `CompletionResponse` porte un `usage` optionnel ; `OpenRouterProvider` le lit. Test sur
   des réponses JSON avec et sans `usage`. Code dans `crates/retrodoc-llm/src/usage.rs` ; un `usage` absent,
   incomplet ou malformé donne `None` sans faire échouer la réponse.
2. [x] **Agrégation par passe** : `UsageTracker` partagé (passe courante via `set_pass`) + `UsageProvider<P>`
   qui cumule appels, tokens in/out, durée, appels sans `usage`, par passe et par modèle. Test avec un faux
   provider, dont un cas concurrent et un cas sans `usage`. Branché dans la CLI au livrable 3.
   Limite : les appels en échec et les retries internes d'`OpenRouterProvider` ne sont pas comptés, donc les
   tokens sont une borne basse avec un serveur instable (à documenter au livrable 5).
3. [x] **Récapitulatif** à la fin de `generate` et de chaque commande autonome qui appelle le LLM (`roles`,
   `glossary`, `entry-points`, `actors` ; `surface` n'appelle pas le LLM) : tableau par passe + total. Détail
   écrit dans `.retrodoc/cache/usage.json` (20 derniers runs, champ `command`), pas dans `run-metadata.json`.
   Un run sans appel n'est pas enregistré ; une commande en échec montre ce qu'elle a dépensé.
4. [~] **Prix optionnels** : abandonné (voir Décisions). Le recap ne donne que des tokens.
5. [x] **Docs** : `PLAN.md` §7.2 item 7, `CLAUDE.md`, `docs/ARCHITECTURE.md`, ADR `0015`.

## Décisions

* Le recap n'est **pas** écrit dans `_retrodoc/run-metadata.json` (il changerait à chaque run et casserait
  l'idempotence du rendu) mais dans `.retrodoc/cache/usage.json`.
* Un recap pour chaque commande qui appelle le LLM, pas seulement `generate`.
* Historique de `usage.json` : 20 derniers runs.
* Pas de prix ni de montant : on ne suit que le coût en tokens (livrable 4 abandonné, pas de
  `llm.price_per_mtok_in/out`). Le montant se calcule à la main à partir des tokens et du tarif du fournisseur.

## Smoke test

Petit dépôt Rust (17 fichiers, `qwen3.6:35b-a3b` local) : recap et `usage.json` corrects (37 appels, 22 881
tokens en entrée, 7 830 en sortie, 4m34s au premier run), rendu idempotent (0 fichier au second run). Le verdict
« concluant » du script est un faux positif (WARN non détectés à cause des codes ANSI) et le second run a fait
2 appels LLM : suivis dans `issues/smoke_test_false_positive.md` et `issues/smoke_empty_feature_retry.md`.

## Hors périmètre (issue suivante)

Estimation avant run pour tout le pipeline : elle exige des mesures réelles (ratios features/use cases par
fichier), donc à faire une fois ce suivi livré et quelques runs enregistrés. Voir `PLAN.md` §7.2 item 7, étape 2.
