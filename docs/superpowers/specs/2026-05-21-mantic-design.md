# Mantic — v1 Product Design Spec

**Date:** 2026-05-21
**Status:** Brainstorm complete, design approved by founder. Ready for plan decomposition.
**Home:** brainctl.org
**Token:** $BRNDB (Solana)
**Tagline (working):** *read the tape*

---

## Product narrative

Mantic is a fleet of memecoin trading agents that run on the user's machine, learn from the user's trades via brainctl's memory layer, and execute through gmgn-skills. It is the flagship consumer product surfacing brainctl as the intelligence layer behind a real, monetizable workflow.

The wedge: every other "AI trading bot" is a stateless prompt loop. Mantic agents *remember* — every trade, every signal, every loss becomes durable memory in a local brain.db. Lossy trades trigger reflexions that bias future decisions. Wins reinforce patterns. The bot literally gets sharper the more the user trades.

The structural differentiator: keys, wallet, brain.db, and execution all live on the user's machine. brainctl.org hosts only the storefront, license server, signal API, and (opt-in) federated insight pool. We cannot front-run our users because we cannot see their trades. This is the inverse of every hosted bot platform.

Target user: crypto-curious to crypto-native, willing to install a desktop app and connect a wallet, but unwilling to manage API keys without help. We solve onboarding by proxying LLM calls through brainctl.org by default (BYO keys remains a power-user lane at a discount).

---

## Section A — System architecture

```
   ┌─────────────────────────────────────────────────────────┐
   │  brainctl.org   (hosted, intentionally small surface)    │
   │                                                           │
   │  • Marketing landing page                                 │
   │  • Stripe checkout (subscription)                         │
   │  • Account portal: manage sub, link wallet, downloads     │
   │  • License server: JWT-based desktop app verification     │
   │  • LLM proxy (default lane): zero-retention, forwards     │
   │    to user's allocated Anthropic + xAI calls              │
   │  • Signal API (paid feature):                             │
   │      - Grok-X firehose (we hold the xAI ingestion key)    │
   │      - KOL maps + smart-money tracker                     │
   │      - Curated trenches / narrative feeds                 │
   │  • Fleet Brain (opt-in, Pro+):                            │
   │      - aggregated, anonymized memory abstractions          │
   │      - k-anonymity + differential-privacy gates            │
   └────────────┬────────────────────────────────────────────┘
                │  authenticated HTTPS / WebSocket
                ▼
   ┌─────────────────────────────────────────────────────────┐
   │  Mantic Desktop App   (user's machine — the product)      │
   │                                                            │
   │  Built with Tauri (Rust core + web UI). Single binary      │
   │  ships GUI by default and `--headless` CLI mode.           │
   │                                                            │
   │  GUI:                                                      │
   │    • Fleet view (live tape, P&L)                           │
   │    • Agent config (template + NL overlay)                  │
   │    • Brain inspector ("what your agents learned")          │
   │    • Wallet pane (BYO connect, session-signer status)      │
   │    • Approvals queue (confirm-mode agents)                 │
   │                                                            │
   │  Core engine:                                              │
   │    • Agent loop: Claude orchestrator + Grok-as-tool        │
   │                  (Sonnet 4.6 default, Opus 4.7 Whale)      │
   │    • brain.db (sqlite at ~/.mantic/brain.db)               │
   │    • gmgn-skills CLI integration                           │
   │    • Session signer (BYO wallet authorizes once)           │
   │    • Local signal cache (subscription feeds + Grok)        │
   │                                                            │
   │  User-managed:                                             │
   │    • Wallet master key (Phantom/MetaMask/Solflare)         │
   │    • Anthropic API key (optional — BYO lane only)          │
   │    • xAI API key (optional — only for raw Grok)            │
   │    • Chain RPCs (suggested defaults, BYO supported)        │
   └─────────────────────────────────────────────────────────┘
```

### Trust boundary

What stays local (machine never leaves):
- Wallet keys + session signing key
- brain.db (full memory, all trades, all reasoning chains)
- Raw signal data after processing
- NL overlay rules
- Strategy configs

What crosses the wire (and only with explicit consent):
- License heartbeat (JWT refresh)
- LLM calls via proxy (if proxy mode enabled; zero-retention policy)
- Signal API requests (token-anonymized subscriber ID, not user identity)
- Federated brain abstractions (opt-in, locally distilled to k-anonymous patterns before transmission)
- Stripe billing events

We can demonstrate via app architecture that we cannot see specific trades, sizes, tokens, wallet addresses, or P&L. This is the marketing pitch and the legal posture.

### What the subscription buys

1. License to run the desktop app (free tier exists; paid unlocks live execution)
2. **Signal API** — Grok-X firehose, KOL maps, smart-money tracking
3. **Fleet Brain** access (read at Pro, write+read at Pro+, weighted at 1M $BRNDB+)
4. **LLM inference** (proxied; BYO lane gets a discount)
5. Bundled Solana RPC endpoints (Helius/QuickNode partnership; BYO supported)

---

## Section B — User journey: signup → first armed agent

Target: under 10 minutes from landing page to first armed agent for the default (proxy LLM) lane.

1. **Land** on brainctl.org. Hero: "Read the tape. Trade with memory." Sub-hero: "Your agents. Your wallet. Your brain. Hosted intelligence." Pricing table visible.

2. **Sign up** with email. Stripe checkout for chosen tier (Free, Starter $19, Pro $59, Whale $199). Free tier requires no card.

3. **(Optional) Link Solana wallet** to claim $BRNDB token-tier perks. Sign a one-time message proving ownership. License server reads time-weighted balance over 24h, applies appropriate tier override.

4. **Download desktop app** (macOS / Windows / Linux installers). One-click install.

5. **Pair the app**: web dashboard shows a 6-digit pairing code. User pastes into the desktop app's first-run screen. App receives signed license JWT, starts heartbeat.

6. **Connect wallet** via in-app wallet connect (Phantom, MetaMask, Solflare). User signs one session-key authorization: "Authorize device key 0xDEAD... to spend up to 5 SOL/day, expiring 2026-06-20." Master wallet never signs trades again until renewal.

7. **Choose LLM lane**: default is proxy mode (nothing to configure). Power users can paste their own Anthropic API key to switch to BYO and get the sub discount applied at next cycle.

8. **Pick a starter template**: Sniper / KOL-Copy / Trenches Scanner (the three shipping in v1). Guided config opens:
   - Risk envelope (max position, daily loss cap, blacklists)
   - Autonomy mode (autonomous / confirm-required)
   - Strategy-specific knobs (e.g., for KOL-Copy: which wallets to mirror, position scaling %)
   - NL overlay (freeform behavior notes)

9. **Fund the wallet**: dashboard shows the user's own connected address with a "fund me" QR. User sends SOL/USDC from anywhere. We never touch funds.

10. **Agent armed**. State transitions from `idle` → `armed`. First signal that matches the strategy triggers the first decision. If autonomous: trade fires. If confirm-mode: push notification with reasoning chain, user approves/rejects in app or via mobile-friendly approval link.

11. **Trade closes**. Outcome logged. Brain updates trust scores on involved entities. If loss: reflexion runs, generates a durable bias against the matched pattern.

---

## Section C — The agent fleet

### The agent unit

An agent is a long-running strategy instance bound to one wallet and one brain scope. Users typically run 2–5 agents in parallel. Each has:

- **Template** (base behavior pattern)
- **NL overlay** (freeform behavior notes, become durable memories)
- **Risk envelope** (caps, blacklists, allowed chains/tokens)
- **Autonomy mode** (autonomous / confirm-required)
- **Brain scope** (`scope="agent:<id>"`, so memories don't bleed across agents unless explicitly federated within the user's own fleet)

### Starter templates shipping in v1

1. **Sniper** — Watches new token deployments. Filters by liquidity, deployer reputation, holder count. Buys early, sells on 2x or rug signal.
2. **KOL-Copy** — Mirrors chosen wallet(s) with configurable position scaling and lag tolerance.
3. **Trenches Scanner** — Pulls gmgn's Trenches feed, evaluates each candidate via Signal API, takes positions on top N.

Reserved for v1.x or v2 (not at launch):
- Swing Trader
- Smart-Money Follower
- Custom (Pro+, blank slate, pure NL brief)

### NL overlay — examples

- *"Never trade tokens where the deployer owns more than 5%."*
- *"Skip anything that's already up >200% in the last hour."*
- *"After three losing trades in a row, pause until I review."*
- *"If $TOKEN_X is mentioned in any signal, buy 0.1 SOL immediately regardless of other rules."*

Stored as `preference`-category memories scoped to the agent. Treated as hard constraints by the decision loop, surfaced explicitly in reasoning chains.

### Fleet view (main app screen)

Single-pane layout:

```
┌───────────────────────────────────────────────────────────────┐
│ Mantic           Wallet: 7xKN...J9aH       Balance: 12.4 SOL  │
├───────────────────────────────────────────────────────────────┤
│ AGENTS                                              [+ New]    │
│ ┌─────────────────────────────────────────────────────────┐  │
│ │ 🟢 Sniper-Sol     │ KOL-Copy "@notthreadguy"   │ Swing  │  │
│ │ Auto · 0.5 SOL/tr │ Confirm · 1.0 SOL/tr       │ Auto   │  │
│ │ +2.3 SOL today    │ -0.8 SOL today              │ +0.1   │  │
│ │ 14 trades · 64%W  │ 4 trades · 50% W            │ 2 · 50%│  │
│ └─────────────────────────────────────────────────────────┘  │
│                                                                │
│ LIVE TAPE                                                      │
│ 19:43:12  Sniper  BUY 0.5 SOL → $WIFY @ 0.0024  (gmgn fills)  │
│ 19:43:08  Sniper  SIGNAL: deployer score 92, liq $340k        │
│ 19:42:55  KOL-Copy  ⏸ APPROVAL NEEDED: @notthreadguy → $PUMP  │
│ 19:41:30  Sniper  SELL 0.4 SOL ← $TURBO @ +47% (TP hit)       │
│                                                                │
│ BRAIN INSIGHTS (this week)                                     │
│ • KOL-Copy: @notthreadguy 71% W over 24 trades                │
│ • Sniper: deployer-age <2h has 3x rug rate (avoiding)         │
│ • Cross-agent: tokens with X-sentiment >85% beat 2.1x         │
└───────────────────────────────────────────────────────────────┘
```

### Brain inspector

A dedicated tab. Organized by:

- **What's working** — patterns with positive expected value, with confidence intervals
- **What's not** — patterns being deprioritized
- **Your stated preferences** — NL overlay rules, plus inferred ones the agent has learned about user style
- **Federated insights** — pulled from the fleet brain, applies to this agent's strategy

Users can directly edit memories:
- Delete a wrong inference
- Pin a memory as "never forget"
- Override a confidence score
- Mark a pattern as "ignore this for this agent only"

This is the moat surface. No other bot exposes its mind. brainctl is the product.

### Approvals queue

For confirm-mode agents only. Mobile-friendly via push notification + approval link (the only mobile surface in v1).

Each row:
- Agent name, action (buy/sell), token, proposed size
- Reasoning chain (3–5 bullets pulled from the decision event)
- Confidence score
- Approve / reject / approve-with-edit (e.g., halve size)
- Auto-expire timer (configurable per agent, 10–60s typical for memecoins)

---

## Section D — Brain layer + federated insights

The moat.

### Local brain.db — memory types specific to trading

- **`preference`** — user-stated rules from NL overlay
- **`lesson`** — patterns learned from outcomes (e.g., "deployer XYZ has 60% rug rate over 8 observations")
- **`decision`** — every non-trivial trade choice with full rationale
- **`environment`** — operational state (current balance, RPC health, open positions)

Plus brainctl-native primitives:
- **Entities**: tokens, deployers, KOLs, chains, strategies. Each accumulates observations.
- **Events**: every signal, decision turn, fill, outcome. Timestamped audit trail.
- **Decisions**: first-class records with rationale, queryable months later.
- **Reflexions**: failure-analysis records that bias future decisions.

### The decision loop

When a signal arrives:

```
1. agent_orient(query="$TOKEN OR deployer:XYZ OR KOL:@X")
     → returns past trades, lessons, user rules, entity history
2. signal_api.enrich($TOKEN)
     → gmgn safety check + Grok X-sentiment + smart-money flow
3. LLM constructs decision with brain context + signal
     → output: skip | propose | buy(size, TP, SL)
4. If buy: event_add(reasoning_chain, importance=0.7)
            execute via gmgn-skills → fill
5. When TP/SL hits: outcome_annotate(decision_id, pnl, regime)
     → updates entity trust scores
     → if loss: reflexion_failure_recurrence triggers structured analysis
```

Every step is logged. The reasoning chain in the live tape and brain inspector is literally an `event_search` over the decision ID.

### Reflexions — the loss-learning loop

When a trade loses:

1. Agent calls `reflexion_failure_recurrence` to find similar past losses
2. LLM analyzes: what signals were present? What was missed? Is this a known-bad pattern?
3. Writes a reflexion that biases the next decision against the matched pattern
4. Over time the brain accumulates **scars** — patterns it actively vetoes

This is the killer feature. Most "AI trading bots" are stateless prompt loops. Mantic agents get sharper from losses.

### Federated brain — what crosses the wire

User's brain.db never leaves the machine. What can cross (opt-in, Pro+):

**Crosses** (only when k-anonymity threshold met):
- *"Sniper template with deployer-age filter > 24h outperformed by 1.3x over 7d in high-volatility regimes"*
- *"This deployer cluster has 47% rug rate across 14 observations"*
- *"KOL @X signals had 41% W in low-vol regime, 58% W in high-vol regime"*

**Never crosses**:
- Specific trade sizes, timestamps, your wallet address
- KOLs you specifically follow (only their public hit-rates aggregated across users)
- Your NL rules
- Raw events, raw token holdings
- Anything PII

### Privacy mechanics

- **Local distillation**: the daemon converts raw memories to abstract pattern proposals before they ever touch the network.
- **k-anonymity threshold**: a pattern is only published once ≥k users have independently observed it. Default k=10. 1M $BRNDB holders see at k=3.
- **Differential privacy**: calibrated noise on aggregate stats; single-user contribution is not reverse-engineerable.

### Cold-start solution

A fresh user's brain is empty — without federated insights, the first 50 trades feel like noise. The federated pool seeds new agents with the fleet's accumulated wisdom:

- Pre-known bad deployer patterns
- Strategy-fitness by current market regime
- Signal-source reliability per template

Free tier gets read-only access at k=20 (heavily curated, high confidence only). Pro+ gets read+write at k=10. 1M $BRNDB+ gets k=3 (alpha-tier).

### Token utility tied to the brain

This is what makes the 1M and 10M $BRNDB tiers meaningful beyond "tier free":

- **Insight weighting boost** — your contributions weighted higher in the aggregator
- **Priority insight feed** — minutes-to-hours head start on new patterns
- **Lower k-anonymity threshold** — see emerging patterns earlier (this is actual alpha)
- **Copy-trade publishing rights (10M)** — publish your strategy + reasoning to other users for a revenue share

The token is not a coupon. The token is an early-access pass to the fleet's collective intelligence.

---

## Section E — Access, billing, wallet, compliance

### Account & identity

- **Primary identity**: email, registered via Stripe Customer
- **Linked wallet**: optional, for $BRNDB token-tier verification only. Sign one-time message. Stored on brainctl.org account portal.
- Wallet is *not* an auth method. Auth is email + standard session cookies + MFA.

### License & device pairing

- Subscription → license JWT issued by brainctl.org, short TTL, refreshed on heartbeat
- Desktop app pairs via one-time 6-digit pairing code shown in web dashboard
- **Starter / Pro** = 1 machine. **Whale** = up to 3 machines (read-only mirror sync, not active execution).
- Multi-machine sync is brain.db + live-tape replication. One machine holds the execution lock; others mirror.

### Wallet model (BYO only, session-signer pattern)

Onboarding:
1. User connects Phantom / MetaMask / Solflare via WalletConnect-style flow
2. Desktop app generates an ephemeral session signing key locally
3. User signs a single authorization in their wallet: spend cap, time-to-expiry
4. Session key signs all subsequent trades autonomously within those limits

Renewal: app prompts user to re-authorize when approaching expiry or spend cap.
Revocation: one-tap "kill session" instantly pauses all agents.

We never touch master keys. Session signing happens entirely in-process on user's machine.

### Subscription mechanics (Stripe)

| Tier | Proxy lane | BYO-key lane | What's included |
|---|---|---|---|
| Free | $0 | n/a | Desktop app, 1 paper-trading agent, signals delayed 60s, no fleet-brain |
| Starter | $19/mo | $14/mo | 1 live agent, basic signals, 5k decisions/mo proxy inference |
| Pro | $59/mo | $39/mo | Unlimited agents, full Grok-X firehose, 50k decisions/mo, fleet-brain read+write |
| Whale | $199/mo | $129/mo | Priority signal latency, multi-machine sync, "unlimited" inference (fair use) |

- **Upgrade**: instant proration, new tier active immediately
- **Downgrade**: effective end of cycle
- **Cancel**: agents auto-pause at cycle end. brain.db persists locally forever.
- **Refund**: 7-day prorated refund on first sub. After that, no refunds.

### Per-trade execution fee

- 0.5% of trade size, capped at $5/trade
- Skimmed before gmgn execution: `0.995 * total → gmgn`, `0.005 * total → mantic fee wallet`
- Visible in live tape on every trade. No hidden costs.
- Free tier: no fee (no live execution)
- 10M $BRNDB tier: zero fee
- Fee wallet candidate for governance-controlled $BRNDB buyback/burn (post-v1)

### $BRNDB token utility & verification

- **Chain**: Solana
- **Verification cadence**: license server reads linked-wallet balance every 6h; real-time on tier-upgrade requests
- **Snapshot**: time-weighted average balance over last 24h. Defeats flash-loan tier-claiming.
- **Stake-to-lock** (post-v1): lock $BRNDB in verified staking contract for 30d → instant tier eligibility, no TWAB delay

Tier perks (recap):
- **100k $BRNDB** → Starter free + private signals (token-holder-only feeds)
- **1M $BRNDB** → Pro free + beta access + fleet-brain weighting boost + k=3 insight access
- **10M $BRNDB** → Whale free + copy-trade publishing rights + governance voice + zero per-trade fees

### Compliance posture

Two regulated touchpoints:

1. **Software that places trades on behalf of users** — non-discretionary tool because user configures strategy and authorizes the wallet. Not a broker, fund, or custodian.
2. **Token with utility tied to access** — utility-token posture, but US law is unsettled. Mitigations:
   - Token launches outside US first
   - Utility is functional-only: no revenue share, no profit promises, no staking yield from us
   - Legal review before US-facing marketing copy

Hard rules in marketing copy from day one:
- *Mantic is software. We are not financial advisors.*
- *Past performance does not predict future returns.*
- *You can lose all your money.*
- *You hold your own keys; we do not custody funds.*

---

## Section F — Out of scope for v1 + open questions

### Deliberately not in v1 (deferred or killed)

- **Mobile native app** — desktop + web is enough; mobile is H2 2026 if demand shows up
- **Multi-machine active execution** — Whale tier mirrors only; one execution-master per fleet
- **Copy-trade marketplace** — 10M tier reserves publishing rights, marketplace UI ships v2
- **Multi-chain agents in one instance** — v1: one agent = one chain
- **Auto tax reporting** — CSV export only; users feed to Koinly/Cointracker
- **Strategy versioning / rollback** — nice-to-have, post-v1
- **In-app fiat onramp** — external (Coinbase, MoonPay); we are not a money transmitter
- **On-chain DAO governance** — Whale governance is advisory in v1 (Discord vote with $BRNDB-weighted voice)

### Open questions to settle before plan-writing

1. **Solana RPC strategy**: bundle Helius/QuickNode into Pro+ proxy lane (smooth UX), BYO RPC for BYO-key lane. Need to negotiate partnership terms.
2. **Marketing page hero emphasis**: local-first / own-your-keys / memory-as-edge — pick one. Recommend memory-as-edge because it's the unique wedge; the other two are table stakes in our positioning.
3. **Token launch timing**: soft-launch product on Stripe-only first, build to ~500 paying users, then drop $BRNDB with immediate utility/demand. Token-first carries narrative risk and gives the token nothing to do at TGE.
4. **gmgn relationship**: open-skill consumption is the minimum; a formal partnership (co-marketing, sponsored bundles) could be material. Worth a direct outreach before launch.
5. **Token symbol clash check**: confirm $BRNDB has no Solana-token conflicts before mint.

---

## Decisions log (founder calls during brainstorm)

| # | Decision | Rationale |
|---|---|---|
| 1 | Hybrid architecture: local daemon + web dashboard | Local-first keys/brain/execution; web for billing/intelligence/signals |
| 2 | Stripe subscription + $BRNDB token discount/utility | Predictable revenue floor + token gives upside and community alignment |
| 3 | Configurable per-agent autonomy (fleet model) | Users run multiple strategies with different risk profiles simultaneously |
| 4 | Templates + NL overlay for strategy definition | Best of both: predictable behavior with personalized rules |
| 5 | BYO wallet only, session-signer pattern (no Privy) | Privy too expensive; clean trust story; "own your keys" wedge |
| 6 | Local-first brain with opt-in federated insights | Solves cold-start, preserves privacy, creates token utility |
| 7 | Desktop app is the product; web is marketing+billing | Maximally local-first; web is intentionally small surface |
| 8 | Proxy LLM by default + BYO-key power-user lane | Onboarding for normies, discount for technical users |
| 9 | Three-layer billing: sub + per-trade fee + token utility | Recurring revenue + usage alignment + token has real function |
| 10 | Free tier (paper trading only) | Top-of-funnel without infra-cost freeloaders |
| 11 | 10M $BRNDB whale threshold | Top tier scarce and aspirational, not just "rich crypto guy" |
| 12 | LLM stack: Claude (Sonnet 4.6 default, Opus 4.7 Whale) + Grok-as-tool | Tool-use reliability + X data moat (see brainctl decision 227) |
| 13 | Product name: Mantic | Mythic-without-overwrought, ownable, distinctive |
| 14 | Token ticker: $BRNDB | Derived from brain.db; ties token to memory layer |

---

## Implementation plan decomposition

This spec describes the full v1 product, which is too large for a single implementation plan. The follow-on `writing-plans` skill should decompose into sub-projects, each with its own plan:

1. **Desktop app shell** (Tauri scaffolding, license pairing, account auth, packaging)
2. **brain.db integration** (brainctl client embedded in app, schema migrations, scopes)
3. **Wallet + session signer** (Phantom/MetaMask/Solflare connect, session-key generation, signing flow)
4. **Agent runtime** (loop, LLM orchestration, gmgn-skills integration, decision logging)
5. **Strategy templates** (Sniper, KOL-Copy, Trenches Scanner — ship 3 for v1)
6. **Signal API service** (Grok-X ingestion, KOL maps, smart-money tracker, subscriber gating)
7. **Fleet Brain service** (insight aggregator, k-anonymity gate, distribution endpoints)
8. **brainctl.org marketing + billing site** (landing, pricing, Stripe checkout, account portal)
9. **License server** (JWT issuance, heartbeat, device pairing)
10. **Token verification service** (Solana RPC reads, TWAB calculation, tier enforcement)
11. **GUI surfaces** (fleet view, brain inspector, approvals queue, wallet pane)
12. **CLI mode** (`--headless` operation, config-file driven, log streaming)

Suggested build order: 1 → 2 → 3 → 4 → 5 → 8 → 9 → 11 → 6 → 7 → 10 → 12. Ship a working alpha at step 5, beta at step 9, public launch at step 11.

---

*End of v1 design spec. Pending: founder review → writing-plans for sub-project plans.*
