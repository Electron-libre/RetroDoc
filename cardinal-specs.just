# The recipes of the specs, imported by `cardinal.just`. Managed by `cardinal sync`: change them in the Cardinal repository.

# Validate specs/*.md and trace their requirements to the `// spec: ID` comments of the tests.
check-specs *args:
    @cardinal run check-specs {{args}}

# List the test files that cite no requirement of an accepted spec, largest first: the capabilities worth a spec.
spec-coverage *args:
    @cardinal run spec-coverage {{args}}
