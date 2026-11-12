# The language of the user

Talk to the user in the `user_language` of `.claude/cardinal.local.toml`, else of `.claude/cardinal.toml`: every
text the user sees, the short notes between two tool calls included, even when a skill, a review or a tool output
around them is in another language. What the agent writes in the repository (a commit message, a spec, an issue)
keeps the language the project requires, also when it is shown to the user.
