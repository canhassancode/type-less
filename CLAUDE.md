This is a AI dictation tool similar to Wispr Flow, but local-first, local model, local everything. Completely open-source.
Check `.CONTEXT.md` for terminology questions.
Check `docs/adr` for a list of ADR decisions made.

# Practice

- This is a `typescript` and `rust` project. The default of the project owner is `typescript`, however, `rust` is required to build this with performance in mind.
- NO `any` types.
- I combine the concept of John Ousterhout `deep modules`, meaning a simple interface for usability. With Robert C. Martins `clean code`, suggesting minimal functions, easy to read and no code comments.
- TDD first, using the `/tdd` skill on ALL implementation where possible.

# Others

- Issue tracker is on Github issues for this repo.
