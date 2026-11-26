# Working on Offish

- Read [the master plan](plans/MASTER_PLAN.md) before architectural changes. Read the applicable phase plan and accepted records in [plans/decisions](plans/decisions) before implementation. Surface conflicts between instructions, plans, and decisions before proceeding with the affected work.
- Create or update the applicable phase plan before substantial work. Tie tasks and acceptance tests to stable requirement IDs. Keep tasks small, testable, and reviewable.
- Prefer open standards and maintained open-source components. Assess licensing, portability, and maintenance before adding dependencies. Avoid unnecessary dependencies and abstractions without a concrete use.
- Write automated tests for implemented functionality, run the relevant checks, and record results. Mark work complete only when it is implemented and the required verification has passed.
- Record significant decisions in ADRs, including considered options, evidence, consequences, status, and how the decision can be revisited. Supersede accepted ADRs with linked records when decisions change.
- Keep plans, decisions, and implementation consistent. Report blockers, assumptions, unresolved questions, and verification gaps. Seek agreement before changing the product vision or expanding the agreed scope.
- Write project documentation in English. Keep phase plans under `plans/phases/` and keep the planning index current.
