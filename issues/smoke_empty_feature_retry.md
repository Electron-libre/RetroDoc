# Une feature sans use case est réessayée à chaque run

# Objectif

Qu'un rerun sur un dépôt inchangé ne rappelle pas le LLM pour une feature dont il a déjà répondu « aucun use
case », ou que ce retour soit un choix explicite et documenté.

# Constat

Smoke test du suivi des tokens (petit dépôt Rust de 17 fichiers) : une feature (sur 6) reçoit « LLM answered
with no use case » aux deux tentatives, à chaque `generate`. Elle n'a aucun use case (confiance 0 % au rapport)
et n'est pas mémorisée comme « traitée » : le second run refait 2 appels (922 tokens en entrée, 20 en sortie)
alors que tout le reste est réutilisé. Le recap de tokens le rend visible (`LLM usage` du second run).

Le retry d'une unité en échec est voulu (ADR 0005 : reprise), mais ici le LLM a répondu proprement, ce n'est pas
une panne. Le résultat est le même à chaque run tant que le modèle et l'entrée ne changent pas.

# Moyen

À trancher d'abord, avant de coder :
* soit mémoriser « aucun use case » dans le cache de la passe (avec l'empreinte d'entrée), donc un rerun l'évite,
  et `--force` ou un changement d'entrée le relance ;
* soit garder le retry (le modèle peut répondre autrement) et l'écrire dans l'ADR 0005 et `PLAN.md`.

La première option a un coût : une réponse vide due à un mauvais tirage du modèle serait figée jusqu'à
`--force`. Voir aussi `issues/lenient_use_case_parsing.md`, qui traite les réponses rejetées par le parseur
(cause différente, même symptôme côté rapport).

# Resources

* `crates/retrodoc-pipeline/src/use_cases/` (cache par feature, `LLM answered with no use case`)
* `crates/retrodoc-pipeline/src/fingerprints.rs`, `cache.rs`
* ADR `0005` (incremental re-run, fingerprints and resume)
* `.retrodoc/cache/usage.json` pour mesurer les appels d'un rerun

# Hints

* Reproduire avec un faux `LlmProvider` qui répond une liste vide de use cases (voir `CountingProvider` dans
  `repo_map/tests.rs`) : le second appel de `build_use_cases` ne doit pas appeler le provider si l'option 1 est
  retenue.
* Ne jamais nommer les dépôts de test confidentiels dans les fichiers commités (chiffres seulement).
