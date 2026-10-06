# Smoke test : faux « concluant » et rerun qui appelle encore le LLM

# Objectif

Rendre le verdict du smoke test fiable : il ne doit plus déclarer « concluant » un run dont les logs contiennent
des WARN, et il doit vérifier que le second `generate` ne fait aucun appel LLM, ce qui est maintenant mesurable.

# Constat

Smoke test du suivi des tokens (petit dépôt Rust de 17 fichiers, `qwen3.6:35b-a3b` local) : verdict
`SMOKE TEST CONCLUSIVE: ... no warning ...`, alors que les deux logs contiennent des lignes WARN (chunk_check,
actors, réponse JSON non parsable, « LLM answered with no use case »).

* Cause : `evaluate` (`.claude/skills/smoke-test/smoke.rs`) filtre les lignes contenant `" WARN "`. Le
  subscriber `tracing` écrit des codes couleur ANSI même quand stderr est redirigé vers un fichier : le niveau
  est suivi d'un code d'échappement, pas d'un espace, donc rien ne correspond.
* Le critère 3 (« second run = 0 fichier écrit ») ne voit pas que le second run a fait 2 appels LLM
  (`use-cases`, 922 tokens en entrée). Depuis `d91bde4`, `.retrodoc/cache/usage.json` donne le nombre d'appels
  par run.

# Moyen

* Retirer les séquences ANSI avant de filtrer (ou désactiver les couleurs du subscriber quand stderr n'est pas un
  terminal, ce qui sert aussi à lire les logs). Un test (`test_smoke.rs`) avec une ligne colorée doit échouer
  avant le correctif.
* Ajouter un critère : le second run n'a fait aucun appel LLM (lire la dernière entrée de `usage.json`, ou la
  ligne `LLM usage:` du log). Décider si c'est bloquant ou seulement signalé tant que
  `issues/smoke_empty_feature_retry.md` n'est pas traitée.

# Resources

* `.claude/skills/smoke-test/smoke.rs`, `test_smoke.rs`, `SKILL.md` (critères « Conclusive means »)
* `crates/retrodoc-pipeline/src/usage_log.rs` (format de `usage.json`)
* `crates/retrodoc-cli/src/main.rs` (initialisation de `tracing_subscriber`)

# Hints

* Ne jamais nommer les dépôts de test confidentiels dans les fichiers commités (chiffres seulement).
* Les tests du harness sont en Rust (`rust-script`), lancés par `just test-harness`.
