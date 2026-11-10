# Replayability, remote drafting, persistence, and delivery — J101–J120

These questions examine whether a new character offers an attractive different life, whether durable
state preserves the chosen path, and whether optional drafting stays outside gameplay authority. Use
fresh-character longitudinal observations, core/public-entry tests, save/load fixtures, and explicitly
bounded draft records.

Sources: [vision](../systems/vision.md), [build diversity](../systems/builds.md),
[architecture](../systems/architecture.md), [arc generation](../systems/arc-generation.md),
[checkpoints](../systems/checkpoints-development.md), and [validation](../systems/validation.md).

## Replayability and delivery

- [ ] **J101.** After encountering a capability the current character does not express, can an uncoached player name a specific next-character combination and describe how its play would differ?
  - **Evidence:** longitudinal playtest notes linked to the observed interaction and proposed next path.
  - **Falsifier:** the only desired restart is a cosmetic weapon change or escape from a broken build.
- [ ] **J102.** In fresh-start tests, does a new identity behave differently before long mastery, rather than following the same universal route with different late-game labels?
  - **Evidence:** early action histories and distinct tactical choices across comparable characters.
  - **Falsifier:** meaningful divergence appears only after an impractically long grind.
- [ ] **J103.** Can an existing character continue to gain worthwhile refinement while a different specialization remains a meaningful reason to start again?
  - **Evidence:** matched current-character refinement and new-character opportunity records.
  - **Falsifier:** the new path makes present advancement feel obsolete or the current path crowds out all alternatives.
- [ ] **J104.** When two characters start separate worlds, can saving, loading, and switching between them preserve independent development, inventories, and world state?
  - **Evidence:** interleaved two-character save/load test.
  - **Falsifier:** one character overwrites or inherits unapproved state from the other.
- [ ] **J105.** Does evidence for a “very long path to godlike power” come from fresh-start routes and observed relevant practice rather than an accelerated developer fixture?
  - **Evidence:** longitudinal route logs, pacing observations, and separate endgame fixture results.
  - **Falsifier:** the design makes a real-time or play-duration claim from accelerated test characters alone.
- [ ] **J106.** Across procedural seeds, do changed sites and ecology create different contexts for a build without making its learned rules or development history arbitrary?
  - **Evidence:** same character capability tested in multiple seeded site contexts.
  - **Falsifier:** replayability relies on hidden balance changes or the same route every seed.
- [ ] **J107.** Can an uncoached player compose and develop a viable combination absent from the authored examples, then retain it through the ordinary play and save path?
  - **Evidence:** independently discovered composition, earned integration evidence, and reload fixture.
  - **Falsifier:** novelty requires a developer-only recipe or cannot persist.

## Remote drafting and authority boundaries

- [ ] **J108.** Are accepted encounter drafts independent of the player's stats, equipment, prepared techniques, or hidden build preferences?
  - **Evidence:** snapshot schema and paired draft-input records for different character states.
  - **Falsifier:** the drafter is given build state to tailor a counter or guaranteed reward.
- [ ] **J109.** Can each draft decision cite bounded site facts and permitted template/catalog IDs without inventing new mechanics or authoritative world facts?
  - **Evidence:** input snapshot, output references, and validator rejection of unknown IDs.
  - **Falsifier:** a model assertion creates an unsupported ability, placement, or resource.
- [ ] **J110.** When imported names, prior drafts, or site text contain instruction-like content, does the draft/validation boundary keep them as untrusted data?
  - **Evidence:** adversarial text fixture and resulting schema/catalog authority.
  - **Falsifier:** content changes rules, prompts, or validator behavior.
- [ ] **J111.** Does only the core validator determine whether a draft is mechanically legal, with any model review limited to advisory text-quality observations?
  - **Evidence:** validator ownership and outcomes when model opinion conflicts with a core rule.
  - **Falsifier:** a model score or approval overrides a mechanical rejection.
- [ ] **J112.** In development, are candidate arcs played and reviewed for readable clues and varied approaches rather than accepted solely because they parse or receive a favorable model verdict?
  - **Evidence:** representative accepted-arc playtests and human presentation review.
  - **Falsifier:** syntactic validity or model confidence is treated as proof of fairness or fun.
- [ ] **J113.** With all remote services disabled, can a player continue ordinary exploration and encounter a supported offline fallback?
  - **Evidence:** full offline route through eligible and unrevealed sites.
  - **Falsifier:** normal play waits on a remote request or requires an account/provider response.
- [ ] **J114.** If a draft times out, is refused, malformed, canceled, or arrives after fallback commitment, does the site remain playable with its selected definition immutable?
  - **Evidence:** each failure transition and late-response fixture.
  - **Falsifier:** a service failure blocks play or a later response rewrites revealed mechanics.
- [ ] **J115.** Is the accepted draft-versus-fallback choice recorded before it affects gameplay so replay of the same saved state and actions remains deterministic?
  - **Evidence:** save record, replay fixture, and request-completion timing cases.
  - **Falsifier:** load-time timing or a new request changes an already revealed site's outcome.

## Persistence, costs, and player-facing clarity

- [ ] **J116.** Are draft attempts, spending reservations, retries, and ambiguous post-crash submissions bounded and recorded without assuming an unverified provider offers exactly-once billing?
  - **Evidence:** local request ledger, configured limits, and crash-after-dispatch fixture.
  - **Falsifier:** automatic unbounded retries or a duplicate submission after an ambiguous outcome.
- [ ] **J117.** Around an action that both spends resources and grants learning or an adaptation, does an interrupted save preserve either the complete committed transition or the prior complete state?
  - **Evidence:** action-boundary crash/save fixtures and subsequent reload.
  - **Falsifier:** reload applies an action twice or retains only part of its costs/effects/credit.
- [ ] **J118.** Does a remote draft snapshot exclude credentials, raw saves, player-authored notes, personal information, and unnecessary build history?
  - **Evidence:** serialized request boundary inspected without exposing credential values.
  - **Falsifier:** secrets or unrelated personal/player state enter the request payload.
- [ ] **J119.** When terrain, rules, or technique-composition versions change, does an unsupported save fail clearly or migrate explicitly without silently reinterpreting a character's signature weave?
  - **Evidence:** versioned save fixtures and compatibility/migration behavior.
  - **Falsifier:** a changed catalog silently deletes or changes persisted identity.
- [ ] **J120.** Through the actual player-facing action and development interface, can a player preview legality, costs, commitment consequences, and failure reasons without relying on model-generated prose?
  - **Evidence:** public core query/resolution path, UI previews, and error-state fixtures.
  - **Falsifier:** critical rules exist only in design text, a remote response, or opaque internal logs.
